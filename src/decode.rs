use base64::{Engine, engine::general_purpose};

/// Plausible text decodings only; binary data and plain strings do not get a popup.
pub fn previews(value: &str) -> Vec<(&'static str, String)> {
    if value.len() > 64 * 1024 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let readable = |bytes: Vec<u8>| {
        String::from_utf8(bytes).ok().filter(|text| {
            !text.is_empty()
                && text
                    .chars()
                    .all(|ch| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t'))
        })
    };
    if value.len() >= 4 && !value.chars().any(char::is_whitespace) {
        for engine in [
            &general_purpose::STANDARD,
            &general_purpose::STANDARD_NO_PAD,
            &general_purpose::URL_SAFE,
            &general_purpose::URL_SAFE_NO_PAD,
        ] {
            if let Ok(bytes) = engine.decode(value)
                && let Some(text) = readable(bytes)
            {
                result.push(("Base64", text));
                break;
            }
        }
    }
    if value.contains('%')
        && let Ok(text) = percent_encoding::percent_decode_str(value).decode_utf8()
        && text != value
        && let Some(text) = readable(text.into_owned().into_bytes())
    {
        result.push(("URL / percent encoding", text));
    }
    let hex = value.strip_prefix("0x").unwrap_or(value);
    if hex.len() >= 4 && hex.len().is_multiple_of(2) && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        if let Some(text) = readable(bytes) {
            result.push(("Hex", text));
        }
    }
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(value)
        && (parsed.is_object() || parsed.is_array())
    {
        result.push(("JSON", serde_json::to_string_pretty(&parsed).unwrap()));
    }
    result
}

/// Find a JSON string token containing a source byte offset, including JSON escapes.
pub fn string_at(text: &str, offset: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if bytes[start] != b'"' {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() {
            match bytes[end] {
                b'\\' => end += 2,
                b'"' => {
                    end += 1;
                    break;
                }
                _ => end += 1,
            }
        }
        if start <= offset && offset < end.min(bytes.len()) {
            return serde_json::from_str(text.get(start..end)?).ok();
        }
        if start > offset {
            break;
        }
        start = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_text_encodings_and_rejects_binary_or_invalid_input() {
        assert_eq!(previews("5Lit5paH")[0].1, "中文");
        assert_eq!(previews("hello%20world")[0].1, "hello world");
        assert_eq!(previews("48656c6c6f").last().unwrap().1, "Hello");
        assert!(previews("////").is_empty());
        assert!(previews("plain text").is_empty());
        assert_eq!(
            string_at(r#"{"v":"\u4e2d\u6587"}"#, 10),
            Some("中文".into())
        );
    }
}
