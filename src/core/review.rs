//! One comparison model drives classification, locations, line colors and review.
use crate::{Difference, alignment, comparison::arrays, pointer_child, source_index, summary};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChangeKind {
    Added,
    Removed,
    #[default]
    Changed,
    TypeChanged,
    Moved,
}
impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Removed => "Removed",
            Self::Changed => "Changed",
            Self::TypeChanged => "Type changed",
            Self::Moved => "Moved",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ArrayMode {
    #[default]
    Auto,
    Position,
    Key,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Options {
    pub array_mode: ArrayMode,
    pub identity_key: String,
    /// Empty applies to all arrays, otherwise an exact JSON Pointer.
    pub array_path: String,
}

pub struct Report {
    pub differences: Vec<Difference>,
    pub alignment: alignment::Alignment,
    pub reordered: Option<Arc<String>>,
}

struct Builder<'a> {
    options: &'a Options,
    normalize: bool,
    limit: usize,
    differences: Vec<Difference>,
    warnings: Vec<String>,
    pairings: HashMap<String, Option<String>>,
}

impl Builder<'_> {
    fn emit(
        &mut self,
        a: Option<&Value>,
        b: Option<&Value>,
        path: &str,
        right: &str,
        original: &str,
        kind: ChangeKind,
        uncertain: bool,
    ) {
        if a.is_none() && b.is_some() {
            self.pairings.insert(right.to_owned(), None);
        }
        if self.differences.len() >= self.limit {
            return;
        }
        let label = if a.is_some() { path } else { right };
        self.differences.push(Difference {
            pointer: if label.is_empty() {
                "/ (root)".into()
            } else {
                label.into()
            },
            left: a.map(summary).unwrap_or_else(|| "(missing)".into()),
            right: b.map(summary).unwrap_or_else(|| "(missing)".into()),
            left_pointer: a.map(|_| path.into()),
            right_pointer: b.map(|_| right.into()),
            original_right_pointer: b.map(|_| original.into()),
            kind,
            left_range: None,
            right_range: None,
            uncertain,
        });
    }

    fn walk(
        &mut self,
        a: &Value,
        b: &mut Value,
        path: &str,
        right: &str,
        original: &str,
        uncertain: bool,
    ) -> bool {
        if right != path || a.is_object() || a.is_array() || b.is_object() || b.is_array() {
            self.pairings
                .insert(right.to_owned(), Some(path.to_owned()));
        }
        match (a, b) {
            (Value::Object(a), Value::Object(b)) => {
                let mut changed = false;
                for (key, value) in a {
                    let p = pointer_child(path, key);
                    let r = pointer_child(right, key);
                    let o = pointer_child(original, key);
                    if let Some(other) = b.get_mut(key) {
                        changed |= self.walk(value, other, &p, &r, &o, uncertain);
                    } else {
                        self.emit(
                            Some(value),
                            None,
                            &p,
                            &r,
                            &o,
                            ChangeKind::Removed,
                            uncertain,
                        );
                    }
                }
                for (key, value) in b.iter() {
                    if !a.contains_key(key) {
                        self.emit(
                            None,
                            Some(value),
                            &pointer_child(path, key),
                            &pointer_child(right, key),
                            &pointer_child(original, key),
                            ChangeKind::Added,
                            uncertain,
                        );
                    }
                }
                if self.normalize {
                    let order = a
                        .keys()
                        .filter(|key| b.contains_key(*key))
                        .chain(b.keys().filter(|key| !a.contains_key(*key)));
                    if !b.keys().eq(order.clone()) {
                        let keys: Vec<_> = order.cloned().collect();
                        let mut old = std::mem::take(b);
                        for key in keys {
                            let value = old.swap_remove(&key).unwrap();
                            b.insert(key, value);
                        }
                        changed = true;
                    }
                }
                changed
            }
            (Value::Array(a), Value::Array(b)) => {
                let mode = if self.options.array_path.is_empty() || self.options.array_path == path
                {
                    self.options.array_mode
                } else {
                    ArrayMode::Auto
                };
                let warning_count = self.warnings.len();
                let (pairs, fuzzy) = self.match_array(a, b, path, mode);
                let invalid_identity =
                    mode == ArrayMode::Key && self.warnings.len() > warning_count;
                let moved = moved_items(&pairs);
                let mut used = vec![false; b.len()];
                let mut uncertain_pairs = vec![invalid_identity; a.len()];
                // Bound ambiguity checks to the same budget as fuzzy matching.
                if fuzzy && a.len().saturating_mul(b.len()) <= 65_536 {
                    uncertain_pairs = arrays::uncertainties(a, b, &pairs);
                    if uncertain_pairs.iter().any(|value| *value) {
                        self.warnings.push(format!("{path}: ambiguous automatic array matches; these differences are marked as uncertain"));
                    }
                } else if fuzzy {
                    for (i, j) in pairs.iter().enumerate() {
                        uncertain_pairs[i] = j.is_some_and(|j| a[i] != b[j]);
                    }
                    if uncertain_pairs.iter().any(|value| *value) {
                        self.warnings.push(format!("{path}: large unmatched array; remaining items retain their relative order. These differences are marked as uncertain."));
                    }
                }
                let mut order: Vec<_> = pairs.iter().flatten().copied().collect();
                for j in &order {
                    used[*j] = true;
                }
                order.extend((0..b.len()).filter(|j| !used[*j]));
                let mut positions: Vec<_> = (0..b.len()).collect();
                let reordered = self.normalize && order.iter().enumerate().any(|(i, j)| i != *j);
                if reordered {
                    let mut old: Vec<_> = std::mem::take(b).into_iter().map(Some).collect();
                    for (new, old_index) in order.into_iter().enumerate() {
                        positions[old_index] = new;
                        b.push(old[old_index].take().unwrap());
                    }
                }
                let mut changed = reordered;
                for (i, old_j) in pairs.into_iter().enumerate() {
                    let p = pointer_child(path, &i.to_string());
                    if let Some(old_j) = old_j {
                        let j = positions[old_j];
                        let r = pointer_child(right, &j.to_string());
                        let o = pointer_child(original, &old_j.to_string());
                        let uncertain = uncertain || uncertain_pairs[i];
                        if mode != ArrayMode::Position && moved[i] && !uncertain {
                            self.emit(
                                Some(&a[i]),
                                Some(&b[j]),
                                &p,
                                &r,
                                &o,
                                ChangeKind::Moved,
                                false,
                            );
                        }
                        changed |= self.walk(&a[i], &mut b[j], &p, &r, &o, uncertain);
                    } else {
                        self.emit(
                            Some(&a[i]),
                            None,
                            &p,
                            "",
                            "",
                            ChangeKind::Removed,
                            uncertain,
                        );
                    }
                }
                for (old_j, used) in used.into_iter().enumerate() {
                    if !used {
                        let j = positions[old_j];
                        self.emit(
                            None,
                            Some(&b[j]),
                            "",
                            &pointer_child(right, &j.to_string()),
                            &pointer_child(original, &old_j.to_string()),
                            ChangeKind::Added,
                            uncertain,
                        );
                    }
                }
                changed
            }
            (a, b) => {
                if a != b {
                    let same_type = std::mem::discriminant(a) == std::mem::discriminant(&*b);
                    self.emit(
                        Some(a),
                        Some(b),
                        path,
                        right,
                        original,
                        if same_type {
                            ChangeKind::Changed
                        } else {
                            ChangeKind::TypeChanged
                        },
                        uncertain,
                    );
                }
                false
            }
        }
    }

    fn match_array(
        &mut self,
        a: &[Value],
        b: &[Value],
        path: &str,
        mode: ArrayMode,
    ) -> (Vec<Option<usize>>, bool) {
        let positional = || (0..a.len()).map(|i| (i < b.len()).then_some(i)).collect();
        if mode == ArrayMode::Position {
            return (positional(), false);
        }
        let fields: Vec<&str> = if mode == ArrayMode::Key {
            vec![self.options.identity_key.trim()]
        } else {
            {
                let mut fields = vec!["id", "_id", "uuid", "guid", "key", "sku", "code"];
                if let Some(object) = a.first().and_then(Value::as_object) {
                    let mut extra: Vec<_> = object
                        .keys()
                        .map(String::as_str)
                        .filter(|key| {
                            key.ends_with("_id") || key.ends_with("Id") || key.ends_with("ID")
                        })
                        .collect();
                    extra.sort_unstable();
                    for field in extra {
                        if !fields.contains(&field) {
                            fields.push(field);
                        }
                    }
                }
                fields.push("name");
                fields
            }
        };
        for field in fields {
            if let (Some(a_ids), Some(b_ids)) = (identities(a, field), identities(b, field)) {
                let lookup: HashMap<_, _> = b_ids
                    .into_iter()
                    .enumerate()
                    .map(|(i, value)| (value, i))
                    .collect();
                if mode == ArrayMode::Auto && !a_ids.iter().any(|id| lookup.contains_key(id)) {
                    continue;
                }
                return (
                    a_ids.iter().map(|id| lookup.get(id).copied()).collect(),
                    false,
                );
            }
        }
        if mode == ArrayMode::Key {
            self.warnings.push(format!(
                "{path}: identity field '{}' is missing, null or duplicated; comparing by position",
                self.options.identity_key
            ));
            return (positional(), false);
        }
        (arrays::pairs(a, b), true)
    }
}

fn identities(values: &[Value], key: &str) -> Option<Vec<String>> {
    if key.is_empty() {
        return None;
    }
    let mut seen = std::collections::HashSet::new();
    values
        .iter()
        .map(|value| {
            let value = value.as_object()?.get(key)?;
            if value.is_null() || value.is_object() || value.is_array() {
                return None;
            }
            let token = value.to_string();
            seen.insert(token.clone()).then_some(token)
        })
        .collect()
}

/// Only items outside the longest increasing sequence are actual moves.
/// An insertion shifting every subsequent index is not a move.
fn moved_items(pairs: &[Option<usize>]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; pairs.len()];
    for (i, value) in pairs.iter().enumerate() {
        if let Some(value) = value {
            let slot = tails.partition_point(|j| pairs[*j].unwrap() < *value);
            previous[i] = slot.checked_sub(1).map(|slot| tails[slot]);
            if slot == tails.len() {
                tails.push(i);
            } else {
                tails[slot] = i;
            }
        }
    }
    let mut moved: Vec<_> = pairs.iter().map(Option::is_some).collect();
    let mut cursor = tails.last().copied();
    while let Some(i) = cursor {
        moved[i] = false;
        cursor = previous[i];
    }
    moved
}

pub fn build(
    left: Arc<String>,
    right: Arc<String>,
    a: &Value,
    mut b: Value,
    options: &Options,
    normalize: bool,
    limit: usize,
) -> Result<Report, String> {
    let mut builder = Builder {
        options,
        normalize,
        limit,
        differences: Vec::new(),
        warnings: Vec::new(),
        pairings: HashMap::new(),
    };
    let changed = builder.walk(a, &mut b, "", "", "", false);
    if builder.differences.len() >= limit && limit > 0 {
        builder.warnings.push(format!(
            "Showing the first {limit} changes; the comparison report is limited."
        ));
    }
    if !options.array_path.is_empty() && !options.array_path.starts_with('/') {
        builder.warnings.push("Array path must be a JSON Pointer such as /items; the selected rule did not match an array.".into());
    }
    let reordered = if changed {
        Some(Arc::new(
            serde_json::to_string_pretty(&b).map_err(|e| e.to_string())?,
        ))
    } else {
        None
    };
    let shown_right = reordered.as_ref().unwrap_or(&right);
    let left_index = source_index::cached(&left)?;
    let right_index = source_index::cached(shown_right)?;
    for difference in &mut builder.differences {
        difference.left_range = difference
            .left_pointer
            .as_deref()
            .and_then(|pointer| left_index.source_range(pointer).ok());
        difference.right_range = difference
            .right_pointer
            .as_deref()
            .and_then(|pointer| right_index.source_range(pointer).ok());
    }
    if !options.array_path.is_empty()
        && a.pointer(&options.array_path)
            .is_none_or(|value| !value.is_array())
    {
        builder.warnings.push(format!(
            "{}: array path was not found in the left input; other arrays use Auto",
            options.array_path
        ));
    }
    let mut alignment = alignment::align_matched(
        &left,
        shown_right,
        &left_index,
        &right_index,
        &builder.pairings,
    );
    alignment.apply_differences(&builder.differences);
    alignment.warnings = builder.warnings;
    Ok(Report {
        differences: builder.differences,
        alignment,
        reordered,
    })
}
