use serde_json::{Map, Value};
use std::sync::Arc;

pub mod alignment;
pub mod edit;
pub mod graph;
pub mod search;

pub const EDIT_LIMIT: usize = 1024 * 1024;
pub const PREVIEW_LIMIT: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Keep,
    Pretty,
    Compact,
    SortAscending,
    SortDescending,
    Nested,
}

pub struct Document {
    pub text: Arc<String>,
    pub value: Arc<Value>,
}

pub fn process(text: Arc<String>, format: Format) -> Result<Document, String> {
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    format_document(
        &Document {
            text,
            value: Arc::new(value),
        },
        format,
    )
}

// Reuse the validated value when formatting; mutations copy only when it is shared.
pub fn format_document(document: &Document, format: Format) -> Result<Document, String> {
    let mut value = document.value.clone();
    match format {
        Format::SortAscending => sort(Arc::make_mut(&mut value), false),
        Format::SortDescending => sort(Arc::make_mut(&mut value), true),
        Format::Nested => expand_nested(Arc::make_mut(&mut value), 0),
        _ => {}
    }
    let text = match format {
        Format::Keep => document.text.clone(),
        Format::Compact => Arc::new(serde_json::to_string(&*value).map_err(|e| e.to_string())?),
        _ => Arc::new(serde_json::to_string_pretty(&*value).map_err(|e| e.to_string())?),
    };
    Ok(Document { text, value })
}

fn sort(value: &mut Value, descending: bool) {
    match value {
        Value::Object(map) => {
            for child in map.values_mut() {
                sort(child, descending);
            }
            map.sort_keys();
            if descending {
                let old = std::mem::take(map);
                *map = old.into_iter().rev().collect();
            }
        }
        Value::Array(array) => array.iter_mut().for_each(|v| sort(v, descending)),
        _ => {}
    }
}

fn expand_nested(value: &mut Value, depth: usize) {
    if depth >= 64 {
        return;
    }
    if let Value::String(text) = value
        && let Ok(parsed @ (Value::Array(_) | Value::Object(_))) = serde_json::from_str(text)
    {
        *value = parsed;
    }
    match value {
        Value::Object(map) => map.values_mut().for_each(|v| expand_nested(v, depth + 1)),
        Value::Array(array) => array.iter_mut().for_each(|v| expand_nested(v, depth + 1)),
        _ => {}
    }
}

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
        Value::Array(v) => format!("Array [{}]", v.len()),
        Value::Object(v) => format!("Object {{{}}}", v.len()),
        Value::String(v) => format!(
            "{:?}{}",
            preview(v, 160),
            if v.len() > 160 { "…" } else { "" }
        ),
        _ => value.to_string(),
    }
}

pub fn csv_to_json(text: &str) -> Result<Value, String> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let headers = reader.headers().map_err(|e| e.to_string())?.clone();
    let mut rows = Vec::new();
    for row in reader.records() {
        let row = row.map_err(|e| e.to_string())?;
        let object: Map<String, Value> = headers
            .iter()
            .zip(row.iter())
            .map(|(key, value)| (key.to_owned(), Value::String(value.to_owned())))
            .collect();
        rows.push(Value::Object(object));
    }
    Ok(Value::Array(rows))
}

pub fn json_to_csv(value: &Value) -> Result<Vec<u8>, String> {
    let rows = value
        .as_array()
        .ok_or("CSV export requires an array of objects")?;
    let mut headers = Map::new();
    for row in rows {
        for key in row
            .as_object()
            .ok_or("CSV export requires an array of objects")?
            .keys()
        {
            headers.insert(key.clone(), Value::Null);
        }
    }
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record(headers.keys())
        .map_err(|e| e.to_string())?;
    for row in rows {
        writer
            .write_record(headers.keys().map(|key| match row.get(key) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(v) => v.to_string(),
            }))
            .map_err(|e| e.to_string())?;
    }
    writer.into_inner().map_err(|e| e.to_string())
}

pub struct Difference {
    pub pointer: String,
    pub left: String,
    pub right: String,
}

// Bound the report so a completely different large document cannot overwhelm the UI.
pub fn compare(left: &Value, right: &Value, limit: usize) -> Vec<Difference> {
    fn walk(
        left: Option<&Value>,
        right: Option<&Value>,
        path: &str,
        limit: usize,
        out: &mut Vec<Difference>,
    ) {
        if out.len() >= limit {
            return;
        }
        match (left, right) {
            (Some(Value::Object(a)), Some(Value::Object(b))) => {
                for (key, value) in a {
                    walk(
                        Some(value),
                        b.get(key),
                        &pointer_child(path, key),
                        limit,
                        out,
                    );
                    if out.len() >= limit {
                        return;
                    }
                }
                for (key, value) in b {
                    if !a.contains_key(key) {
                        walk(None, Some(value), &pointer_child(path, key), limit, out);
                    }
                    if out.len() >= limit {
                        return;
                    }
                }
            }
            (Some(Value::Array(a)), Some(Value::Array(b))) => {
                for i in 0..a.len().max(b.len()) {
                    walk(
                        a.get(i),
                        b.get(i),
                        &pointer_child(path, &i.to_string()),
                        limit,
                        out,
                    );
                    if out.len() >= limit {
                        return;
                    }
                }
            }
            (a, b) if a != b => out.push(Difference {
                pointer: if path.is_empty() {
                    "/ (root)".into()
                } else {
                    path.into()
                },
                left: a.map(summary).unwrap_or_else(|| "(missing)".into()),
                right: b.map(summary).unwrap_or_else(|| "(missing)".into()),
            }),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(Some(left), Some(right), "", limit, &mut out);
    out
}
