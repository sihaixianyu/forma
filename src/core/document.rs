use serde_json::{Map, Value};
use std::sync::Arc;

use crate::{diagnostics, parse_json};

pub const EDIT_LIMIT: usize = 8 * 1024 * 1024;
pub const PREVIEW_LIMIT: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    let _stage = diagnostics::Stage::new("process", text.len());
    log::debug!(target: "forma", "event=format_request format={format:?} cached=false");
    let value = parse_json(&text)?;
    format_owned(
        Document {
            text,
            value: Arc::new(value),
        },
        format,
    )
}

/// Reuses the validated value when formatting and copies it only for mutations.
pub fn format_document(document: &Document, format: Format) -> Result<Document, String> {
    log::debug!(target: "forma", "event=format_request format={format:?} cached=true");
    format_owned(
        Document {
            text: document.text.clone(),
            value: document.value.clone(),
        },
        format,
    )
}

fn format_owned(document: Document, format: Format) -> Result<Document, String> {
    let _stage = diagnostics::Stage::new("format_document", document.text.len());
    let mut value = document.value;
    {
        let _transform = diagnostics::Stage::new("transform", document.text.len());
        match format {
            Format::SortAscending => sort(Arc::make_mut(&mut value), false),
            Format::SortDescending => sort(Arc::make_mut(&mut value), true),
            Format::Nested => expand_nested(Arc::make_mut(&mut value), 0),
            _ => {}
        }
    }
    let _serialize = diagnostics::Stage::new("serialize", document.text.len());
    let text = match format {
        Format::Keep => document.text,
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
            let ordered = map
                .keys()
                .is_sorted_by(|a, b| if descending { a >= b } else { a <= b });
            if ordered {
                return;
            }
            if descending {
                let old = std::mem::take(map);
                let mut entries: Vec<_> = old.into_iter().collect();
                entries.sort_unstable_by(|a, b| b.0.cmp(&a.0));
                *map = entries.into_iter().collect();
            } else {
                map.sort_keys();
            }
        }
        Value::Array(array) => array.iter_mut().for_each(|value| sort(value, descending)),
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
        Value::Object(map) => map
            .values_mut()
            .for_each(|value| expand_nested(value, depth + 1)),
        Value::Array(array) => array
            .iter_mut()
            .for_each(|value| expand_nested(value, depth + 1)),
        _ => {}
    }
}

pub fn csv_to_json(text: &str) -> Result<Value, String> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let headers = reader.headers().map_err(|error| error.to_string())?.clone();
    let mut rows = Vec::new();
    for row in reader.records() {
        let row = row.map_err(|error| error.to_string())?;
        let object: Map<String, Value> = headers
            .iter()
            .zip(row.iter())
            .map(|(key, value)| (key.to_owned(), Value::String(value.to_owned())))
            .collect();
        rows.push(Value::Object(object));
    }
    Ok(Value::Array(rows))
}
