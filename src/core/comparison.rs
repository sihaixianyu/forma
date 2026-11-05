use serde_json::Value;

use crate::{pointer_child, summary};

pub(crate) mod arrays;

/// Keep shared object keys in the reference order at every matching path.
/// Right-only keys retain their relative order; array elements never move.
pub fn match_key_order(reference: &Value, value: &mut Value) -> bool {
    match_order(reference, value, false)
}

/// Normalize comparison order, including one-to-one matches for array elements.
pub fn match_comparison_order(reference: &Value, value: &mut Value) -> bool {
    match_order(reference, value, true)
}

fn match_order(reference: &Value, value: &mut Value, match_arrays: bool) -> bool {
    match (reference, value) {
        (Value::Object(reference), Value::Object(value)) => {
            let mut changed = false;
            for (key, child) in reference {
                if let Some(other) = value.get_mut(key) {
                    changed |= match_order(child, other, match_arrays);
                }
            }
            let order = reference
                .keys()
                .filter(|key| value.contains_key(*key))
                .chain(value.keys().filter(|key| !reference.contains_key(*key)));
            if !value.keys().eq(order.clone()) {
                let order: Vec<_> = order.cloned().collect();
                let mut original = std::mem::take(value);
                for key in order {
                    let child = original.swap_remove(&key).unwrap();
                    value.insert(key, child);
                }
                changed = true;
            }
            changed
        }
        (Value::Array(reference), Value::Array(value)) => {
            if match_arrays {
                let (mut changed, pairs) = arrays::reorder(reference, value);
                for (i, j) in pairs.into_iter().enumerate() {
                    if let Some(j) = j {
                        changed |= match_order(&reference[i], &mut value[j], true);
                    }
                }
                changed
            } else {
                reference
                    .iter()
                    .zip(value)
                    .fold(false, |changed, (a, b)| match_order(a, b, false) || changed)
            }
        }
        _ => false,
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Difference {
    pub pointer: String,
    pub left: String,
    pub right: String,
    pub left_pointer: Option<String>,
    pub right_pointer: Option<String>,
    #[serde(default)]
    pub kind: crate::review::ChangeKind,
    #[serde(default)]
    pub left_range: Option<std::ops::Range<usize>>,
    #[serde(default)]
    pub right_range: Option<std::ops::Range<usize>>,
    #[serde(default)]
    pub original_right_pointer: Option<String>,
    #[serde(default)]
    pub uncertain: bool,
}

/// Bounds the report so a completely different large document cannot overwhelm the UI.
pub fn compare(left: &Value, right: &Value, limit: usize) -> Vec<Difference> {
    fn walk(
        left: Option<&Value>,
        right: Option<&Value>,
        path: &str,
        right_path: &str,
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
                        &pointer_child(right_path, key),
                        limit,
                        out,
                    );
                    if out.len() >= limit {
                        return;
                    }
                }
                for (key, value) in right {
                    if !left.contains_key(key) {
                        walk(
                            None,
                            Some(value),
                            &pointer_child(path, key),
                            &pointer_child(right_path, key),
                            limit,
                            out,
                        );
                    }
                    if out.len() >= limit {
                        return;
                    }
                }
            }
            (Some(Value::Array(left)), Some(Value::Array(right))) => {
                let pairs = arrays::pairs(left, right);
                let mut used = vec![false; right.len()];
                for (index, other) in pairs.into_iter().enumerate() {
                    if let Some(other) = other {
                        used[other] = true;
                    }
                    walk(
                        left.get(index),
                        other.and_then(|other| right.get(other)),
                        &pointer_child(path, &index.to_string()),
                        &pointer_child(right_path, &other.unwrap_or(index).to_string()),
                        limit,
                        out,
                    );
                    if out.len() >= limit {
                        return;
                    }
                }
                for (index, value) in right.iter().enumerate() {
                    if !used[index] {
                        walk(
                            None,
                            Some(value),
                            &pointer_child(path, &index.to_string()),
                            &pointer_child(right_path, &index.to_string()),
                            limit,
                            out,
                        );
                    }
                    if out.len() >= limit {
                        return;
                    }
                }
            }
            (left, right) if left != right => out.push(Difference {
                pointer: if (if left.is_some() { path } else { right_path }).is_empty() {
                    "/ (root)".into()
                } else {
                    (if left.is_some() { path } else { right_path }).into()
                },
                left: left.map(summary).unwrap_or_else(|| "(missing)".into()),
                right: right.map(summary).unwrap_or_else(|| "(missing)".into()),
                left_pointer: left.map(|_| path.into()),
                right_pointer: right.map(|_| right_path.into()),
                kind: if left.is_none() {
                    crate::review::ChangeKind::Added
                } else if right.is_none() {
                    crate::review::ChangeKind::Removed
                } else {
                    crate::review::ChangeKind::Changed
                },
                left_range: None,
                right_range: None,
                original_right_pointer: right.map(|_| right_path.into()),
                uncertain: false,
            }),
            _ => {}
        }
    }

    let mut differences = Vec::new();
    walk(Some(left), Some(right), "", "", limit, &mut differences);
    differences
}
