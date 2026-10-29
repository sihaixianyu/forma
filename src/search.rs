use crate::{append_pointer, preview, summary};
use serde_json::Value;
use std::ops::Range;

pub const TEXT_LIMIT: usize = 10_000;
pub const TREE_LIMIT: usize = 1_000;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct TextMatches {
    pub ranges: Vec<Range<usize>>,
    pub total: usize,
}

/// Case-sensitive literal search. Ranges use UTF-8 byte offsets into the original source.
pub fn find_text(text: &str, query: &str) -> TextMatches {
    let _stage = crate::diagnostics::Stage::new("search_text", text.len());
    let mut result = TextMatches::default();
    if query.is_empty() {
        return result;
    }
    for (offset, matched) in text.match_indices(query) {
        result.total += 1;
        if result.ranges.len() < TEXT_LIMIT {
            result.ranges.push(offset..offset + matched.len());
        }
    }
    log::debug!(target: "json_lens", "event=search_result matches={} stored={}", result.total, result.ranges.len());
    result
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TreeHit {
    pub pointer: String,
    pub value: String,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct TreeMatches {
    pub hits: Vec<TreeHit>,
    pub total: usize,
}

/// Search decoded property names and scalar values throughout the validated document.
pub fn find_tree(value: &Value, query: &str) -> TreeMatches {
    let _stage = crate::diagnostics::Stage::new("search_tree", 0);
    let mut result = TreeMatches::default();
    if query.is_empty() {
        return result;
    }
    fn walk(value: &Value, key: &str, path: &mut String, query: &str, result: &mut TreeMatches) {
        let matched = key.contains(query)
            || match value {
                Value::String(s) => s.contains(query),
                Value::Number(n) => n.to_string().contains(query),
                Value::Bool(b) => b.to_string().contains(query),
                Value::Null => "null".contains(query),
                _ => false,
            };
        if matched {
            result.total += 1;
            if result.hits.len() < TREE_LIMIT {
                result.hits.push(TreeHit {
                    pointer: path.clone(),
                    value: preview(&summary(value), 200).into(),
                });
            }
        }
        let length = path.len();
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    append_pointer(path, key);
                    walk(child, key, path, query, result);
                    path.truncate(length);
                }
            }
            Value::Array(array) => {
                for (i, child) in array.iter().enumerate() {
                    append_pointer(path, &i.to_string());
                    walk(child, "", path, query, result);
                    path.truncate(length);
                }
            }
            _ => {}
        }
    }
    walk(value, "", &mut String::new(), query, &mut result);

    result
}
