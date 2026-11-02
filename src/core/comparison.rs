use serde_json::Value;

use crate::{pointer_child, summary};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Difference {
    pub pointer: String,
    pub left: String,
    pub right: String,
}

/// Bounds the report so a completely different large document cannot overwhelm the UI.
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
            (Some(Value::Object(left)), Some(Value::Object(right))) => {
                for (key, value) in left {
                    walk(
                        Some(value),
                        right.get(key),
                        &pointer_child(path, key),
                        limit,
                        out,
                    );
                    if out.len() >= limit {
                        return;
                    }
                }
                for (key, value) in right {
                    if !left.contains_key(key) {
                        walk(None, Some(value), &pointer_child(path, key), limit, out);
                    }
                    if out.len() >= limit {
                        return;
                    }
                }
            }
            (Some(Value::Array(left)), Some(Value::Array(right))) => {
                for index in 0..left.len().max(right.len()) {
                    walk(
                        left.get(index),
                        right.get(index),
                        &pointer_child(path, &index.to_string()),
                        limit,
                        out,
                    );
                    if out.len() >= limit {
                        return;
                    }
                }
            }
            (left, right) if left != right => out.push(Difference {
                pointer: if path.is_empty() {
                    "/ (root)".into()
                } else {
                    path.into()
                },
                left: left.map(summary).unwrap_or_else(|| "(missing)".into()),
                right: right.map(summary).unwrap_or_else(|| "(missing)".into()),
            }),
            _ => {}
        }
    }

    let mut differences = Vec::new();
    walk(Some(left), Some(right), "", limit, &mut differences);
    differences
}
