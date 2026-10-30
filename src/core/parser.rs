use serde_json::{Map, Number, Value};
use sonic_rs::{JsonContainerTrait, JsonType, JsonValueTrait};

/// Parse with SIMD while retaining serde_json's ordered objects and exact numbers.
pub fn parse_json(text: &str) -> Result<Value, String> {
    let _stage = crate::diagnostics::Stage::new("parse_json", text.len());
    // Avoid building a temporary DOM for small interactive edits.
    if text.len() < 32 * 1024 {
        log::debug!(target: "json_lens", "event=parser_path parser=serde reason=small_input");
        return reference_parse(text);
    }
    let result = {
        let _parse = crate::diagnostics::Stage::new("parse_sonic", text.len());
        sonic_rs::from_str::<sonic_rs::Value>(text)
    };
    let result = match result {
        Ok(value) => {
            let _convert = crate::diagnostics::Stage::new("convert_dom", text.len());
            convert(&value, 0).map_err(|_| "conversion_unsupported")
        }
        Err(_) => Err("sonic_parse_failed"),
    };
    // Keep the existing validation behavior and detailed errors on unusual inputs.
    if let Ok(value) = result {
        log::debug!(target: "json_lens", "event=parser_path parser=sonic simd128={} parallel={}",
                cfg!(target_feature = "simd128"), cfg!(not(target_arch = "wasm32")));
        Ok(value)
    } else {
        log::debug!(target: "json_lens", "event=parser_path parser=serde reason={}", result.unwrap_err());
        reference_parse(text)
    }
}

fn reference_parse(text: &str) -> Result<Value, String> {
    let _stage = crate::diagnostics::Stage::new("parse_serde", text.len());
    serde_json::from_str(text).map_err(|error| error.to_string())
}

fn convert(value: &sonic_rs::Value, depth: usize) -> Result<Value, ()> {
    match value.get_type() {
        JsonType::Array => {
            if depth >= 127 {
                return Err(());
            }
            let array = value.as_array().ok_or(())?;
            #[cfg(not(target_arch = "wasm32"))]
            if array.len() >= 4096 {
                use rayon::prelude::*;
                let mut output = vec![Value::Null; array.len()];
                output
                    .par_iter_mut()
                    .zip(array.as_slice().par_iter())
                    .try_for_each(|(slot, value)| {
                        *slot = convert(value, depth + 1)?;
                        Ok(())
                    })?;
                return Ok(Value::Array(output));
            }
            let mut output = Vec::with_capacity(array.len());
            for value in array.iter() {
                output.push(convert(value, depth + 1)?);
            }
            Ok(Value::Array(output))
        }
        JsonType::Object => {
            if depth >= 127 {
                return Err(());
            }
            let object = value.as_object().ok_or(())?;
            let mut map = Map::with_capacity(object.len());
            for (key, value) in object.iter() {
                // serde_json interprets these reserved first keys during Value deserialization.
                if map.is_empty()
                    && matches!(
                        key,
                        "$serde_json::private::Number" | "$serde_json::private::RawValue"
                    )
                {
                    return Err(());
                }
                map.insert(key.to_string(), convert(value, depth + 1)?);
            }
            Ok(Value::Object(map))
        }
        JsonType::String => Ok(Value::String(value.as_str().ok_or(())?.to_owned())),
        JsonType::Number => {
            // Never pass through f64: preserve huge integers, exponents and trailing zeros.
            let raw = value.as_raw_number().ok_or(())?;
            let text = raw.as_str();
            // Common integers can use the public integer constructor without
            // creating another JSON deserializer for each scalar.
            if let Ok(integer) = text.parse::<u64>() {
                return Ok(Value::Number(Number::from(integer)));
            }
            if text != "-0"
                && let Ok(integer) = text.parse::<i64>()
            {
                return Ok(Value::Number(Number::from(integer)));
            }
            text.parse::<Number>().map(Value::Number).map_err(|_| ())
        }
        JsonType::Boolean => Ok(Value::Bool(value.as_bool().ok_or(())?)),
        JsonType::Null => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn padded(source: &str) -> String {
        format!("{}{}", " ".repeat(32 * 1024), source)
    }

    #[test]
    fn fast_path_retains_exact_numbers_order_and_duplicate_key_behavior() {
        let source = padded(
            r#"{"z":123456789012345678901234567890,"a":[-0,-0.0,1.2300e+42,1e-9999,1e9999,18446744073709551616],"z":999999999999999999999999999999}"#,
        );
        let actual = parse_json(&source).unwrap();
        let expected = reference_parse(&source).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fast_path_retains_unicode_and_escaped_strings() {
        let source = padded(
            r#"{"中文":"🚀","escaped":"\u4e2d\ud83d\ude80\n\t\\\"","empty":[],"null":null,"bool":true}"#,
        );
        let actual = parse_json(&source).unwrap();
        let expected = reference_parse(&source).unwrap();
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn parallel_conversion_keeps_array_order() {
        let source = serde_json::to_string(
            &(0..8192)
                .map(|id| serde_json::json!({"id": id, "v": [id, id + 1]}))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let actual = parse_json(&source).unwrap();
        assert_eq!(
            actual.to_string(),
            reference_parse(&source).unwrap().to_string()
        );
    }

    #[test]
    fn fast_path_keeps_validation_errors_and_recursion_limit() {
        for source in ["[1,]", "{} {}", "01", "[NaN]", r#""\ud800""#, "{\"x\":}"] {
            let source = padded(source);
            assert_eq!(
                parse_json(&source).unwrap_err(),
                reference_parse(&source).unwrap_err()
            );
        }
        // serde_json's reference parser needs more stack at its depth boundary in debug builds.
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                for depth in [126, 127, 128, 256] {
                    let source = padded(&format!("{}0{}", "[".repeat(depth), "]".repeat(depth)));
                    match reference_parse(&source) {
                        Ok(expected) => assert_eq!(parse_json(&source).unwrap(), expected),
                        Err(expected) => assert_eq!(parse_json(&source).unwrap_err(), expected),
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
