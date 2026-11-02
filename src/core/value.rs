use serde_json::Value;

pub fn preview(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

pub fn pointer_child(parent: &str, key: &str) -> String {
    let mut pointer = String::with_capacity(parent.len() + key.len() + 1);
    pointer.push_str(parent);
    append_pointer(&mut pointer, key);
    pointer
}

pub fn append_pointer(pointer: &mut String, key: &str) {
    pointer.push('/');
    for ch in key.chars() {
        match ch {
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            _ => pointer.push(ch),
        }
    }
}

pub fn summary(value: &Value) -> String {
    match value {
        Value::Array(values) => format!("Array [{}]", values.len()),
        Value::Object(values) => format!("Object {{{}}}", values.len()),
        Value::String(value) => format!(
            "{:?}{}",
            preview(value, 160),
            if value.len() > 160 { "…" } else { "" }
        ),
        _ => value.to_string(),
    }
}
