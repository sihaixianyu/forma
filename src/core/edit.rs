use crate::{Document, Format, process};
use serde_json::{Value, value::RawValue};
use std::{collections::HashMap, ops::Range, sync::Arc};

pub(super) fn property_start(text: &str, value_start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut end = value_start;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == 0 || bytes[end - 1] != b':' {
        return None;
    }
    end -= 1;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == 0 || bytes[end - 1] != b'"' {
        return None;
    }
    let mut start = end - 1;
    while start > 0 {
        start -= 1;
        if bytes[start] == b'"' {
            let mut escaped = start;
            while escaped > 0 && bytes[escaped - 1] == b'\\' {
                escaped -= 1;
            }
            if (start - escaped).is_multiple_of(2) {
                while start > 0 && matches!(bytes[start - 1], b' ' | b'\t' | b'\r') {
                    start -= 1;
                }
                return Some(start);
            }
        }
    }
    None
}

/// Resolve a UTF-8 source offset to its deepest value or property, without building a second value tree.
pub fn pointer_at_offset(text: &str, offset: usize) -> Result<Option<String>, String> {
    if offset > text.len() {
        return Ok(None);
    }
    let root: &RawValue = serde_json::from_str(text).map_err(|e| e.to_string())?;
    fn range(text: &str, raw: &RawValue) -> Range<usize> {
        let start = raw.get().as_ptr() as usize - text.as_ptr() as usize;
        start..start + raw.get().len()
    }
    fn contains_caret(text: &str, range: &Range<usize>, offset: usize) -> bool {
        range.start <= offset
            && (offset < range.end
                || (offset == range.end
                    && !matches!(text.as_bytes().get(offset), Some(b']' | b'}'))))
    }
    fn descend(
        text: &str,
        raw: &RawValue,
        offset: usize,
        pointer: String,
    ) -> Result<String, String> {
        match raw.get().as_bytes().first() {
            Some(b'{') => {
                let map: HashMap<String, &RawValue> =
                    serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
                let mut properties: Vec<_> = map
                    .into_iter()
                    .map(|(key, child)| {
                        let range = range(text, child);
                        let bound = property_start(text, range.start).unwrap_or(range.start);
                        (bound, range, key, child)
                    })
                    .collect();
                properties.sort_unstable_by_key(|(bound, ..)| *bound);
                let object_end = range(text, raw).end.saturating_sub(1);
                for (index, (bound, range, key, child)) in properties.iter().enumerate() {
                    let property_end = properties
                        .get(index + 1)
                        .map_or(object_end, |(next_bound, ..)| *next_bound);
                    if *bound <= offset && offset < property_end {
                        let pointer = crate::pointer_child(&pointer, key);
                        if offset < range.start {
                            return Ok(pointer);
                        }
                        if offset <= range.end {
                            return descend(text, child, offset, pointer);
                        }
                        return Ok(pointer);
                    }
                }
            }
            Some(b'[') => {
                let array: Vec<&RawValue> =
                    serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
                for (index, child) in array.iter().enumerate() {
                    let range = range(text, child);
                    if contains_caret(text, &range, offset) {
                        return descend(
                            text,
                            child,
                            offset,
                            crate::pointer_child(&pointer, &index.to_string()),
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(pointer)
    }
    let bounds = range(text, root);
    if offset < bounds.start || offset >= bounds.end {
        return Ok(None);
    }
    descend(text, root, offset, String::new()).map(Some)
}

pub fn source_range(text: &str, pointer: &str) -> Result<Range<usize>, String> {
    let mut raw: &RawValue = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if !pointer.is_empty() {
        let path = pointer.strip_prefix('/').ok_or("Invalid JSON Pointer")?;
        for token in path.split('/') {
            let mut key = String::new();
            let mut chars = token.chars();
            while let Some(ch) = chars.next() {
                if ch == '~' {
                    key.push(match chars.next() {
                        Some('0') => '~',
                        Some('1') => '/',
                        _ => return Err("Invalid pointer escape".into()),
                    });
                } else {
                    key.push(ch);
                }
            }
            raw = if raw.get().starts_with('{') {
                let map: HashMap<String, &RawValue> =
                    serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
                *map.get(&key).ok_or("Pointer not found")?
            } else if raw.get().starts_with('[') {
                if key.len() > 1 && key.starts_with('0') {
                    return Err("Invalid array index".into());
                }
                let index: usize = key.parse().map_err(|_| "Invalid array index")?;
                let array: Vec<&RawValue> =
                    serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
                *array.get(index).ok_or("Array index out of range")?
            } else {
                return Err("Cannot descend into a scalar".into());
            };
        }
    }
    let start = raw.get().as_ptr() as usize - text.as_ptr() as usize;
    Ok(start..start + raw.get().len())
}

/// Patch only the selected value so the surrounding source retains its formatting and escapes.
pub fn replace_value(doc: &Document, pointer: &str, replacement: &str) -> Result<Document, String> {
    let _: Value = serde_json::from_str(replacement).map_err(|e| e.to_string())?;
    let range = source_range(&doc.text, pointer)?;
    let mut text = (*doc.text).clone();
    text.replace_range(range, replacement.trim());
    process(Arc::new(text), Format::Keep)
}

pub fn rename_key(doc: &Document, pointer: &str, new_key: &str) -> Result<Document, String> {
    let (parent, key) = pointer
        .rsplit_once('/')
        .ok_or("The root has no property name")?;
    let key = key.replace("~1", "/").replace("~0", "~");
    let map = doc
        .value
        .pointer(parent)
        .and_then(Value::as_object)
        .ok_or("Only object properties can be renamed")?;
    if key != new_key && map.contains_key(new_key) {
        return Err("A property with that name already exists".into());
    }
    let range = source_range(&doc.text, pointer)?;
    let bytes = doc.text.as_bytes();
    let mut end = range.start;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == 0 || bytes[end - 1] != b':' {
        return Err("Property separator not found".into());
    }
    end -= 1;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == 0 || bytes[end - 1] != b'"' {
        return Err("Property name not found".into());
    }
    let mut start = end - 1;
    loop {
        if start == 0 {
            return Err("Property name start not found".into());
        }
        start -= 1;
        if bytes[start] == b'"' {
            let mut escaped = start;
            while escaped > 0 && bytes[escaped - 1] == b'\\' {
                escaped -= 1;
            }
            if (start - escaped).is_multiple_of(2) {
                break;
            }
        }
    }
    let replacement = serde_json::to_string(new_key).map_err(|e| e.to_string())?;
    let mut text = (*doc.text).clone();
    text.replace_range(start..end, &replacement);
    process(Arc::new(text), Format::Keep)
}
