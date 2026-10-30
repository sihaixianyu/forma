//! Browser benchmark entry points, excluded from normal release builds.
use json_lens::{Document, Format, format_document, process};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_time::Instant;

fn reference(text: Arc<String>, format: Format) -> Result<Document, String> {
    let value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    format_document(
        &Document {
            text,
            value: Arc::new(value),
        },
        format,
    )
}

#[wasm_bindgen]
pub fn benchmark_json(input: &str, parser: &str) -> Result<String, JsValue> {
    // Logging and JS-to-WASM string transfer are outside the measured operations.
    let level = log::max_level();
    struct Restore(log::LevelFilter);
    impl Drop for Restore {
        fn drop(&mut self) {
            log::set_max_level(self.0);
        }
    }
    let _restore = Restore(level);
    log::set_max_level(log::LevelFilter::Warn);
    let run = match parser {
        "sonic" => process,
        "serde" => reference,
        _ => return Err(JsValue::from_str("Unknown parser")),
    };
    let input = Arc::new(input.to_owned());
    let mut results = serde_json::Map::new();
    for (operation, format) in [
        ("parse", Format::Keep),
        ("parse_pretty", Format::Pretty),
        ("parse_compact", Format::Compact),
    ] {
        for _ in 0..3 {
            drop(run(input.clone(), format).map_err(|error| JsValue::from_str(&error))?);
        }
        let mut samples = Vec::with_capacity(11);
        for _ in 0..11 {
            let start = Instant::now();
            let document = std::hint::black_box(
                run(input.clone(), format).map_err(|error| JsValue::from_str(&error))?,
            );
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            drop(document);
        }
        samples.sort_by(f64::total_cmp);
        results.insert(operation.into(), serde_json::json!(samples[5]));
    }
    Ok(serde_json::Value::Object(results).to_string())
}

#[wasm_bindgen]
pub fn verify_json(input: &str) -> Result<String, JsValue> {
    let actual = json_lens::parse_json(input);
    let expected =
        serde_json::from_str::<serde_json::Value>(input).map_err(|error| error.to_string());
    if actual != expected {
        return Err(JsValue::from_str("Parser behavior differs from serde_json"));
    }
    Ok(match actual {
        Ok(value) => value.to_string(),
        Err(_) => "invalid".into(),
    })
}
