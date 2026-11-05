use similar::{Algorithm, DiffTag, capture_diff_slices_deadline};
use std::{sync::Arc, time::Duration};

use crate::source_index::SourceIndex;

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Side {
    #[serde(skip)]
    pub source: String,
    /// Original line index -> aligned display row. Gaps never enter the document.
    pub rows: Vec<usize>,
    pub changed: Vec<bool>,
    #[serde(default)]
    pub kinds: Vec<Option<crate::review::ChangeKind>>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Alignment {
    pub left: Side,
    pub right: Side,
    pub height: usize,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Alignment {
    pub fn apply_differences(&mut self, differences: &[crate::Difference]) {
        for side in [&mut self.left, &mut self.right] {
            side.changed.fill(false);
            side.kinds = vec![None; side.rows.len()];
        }
        for (left, side) in [(true, &mut self.left), (false, &mut self.right)] {
            let starts: Vec<_> = std::iter::once(0)
                .chain(side.source.match_indices('\n').map(|(i, _)| i + 1))
                .collect();
            for difference in differences {
                let range = if left {
                    &difference.left_range
                } else {
                    &difference.right_range
                };
                if let Some(range) = range {
                    let first = starts
                        .partition_point(|start| *start <= range.start)
                        .saturating_sub(1);
                    let last = starts
                        .partition_point(|start| *start < range.end)
                        .max(first + 1)
                        .min(starts.len());
                    for line in first..last {
                        side.changed[line] = true;
                        if side.kinds[line].is_none()
                            || difference.kind != crate::review::ChangeKind::Moved
                        {
                            side.kinds[line] = Some(difference.kind);
                        }
                    }
                }
            }
        }
    }

    /// Aligned rows outside the context around any semantic change.
    pub fn unchanged_regions(&self, context: usize) -> Vec<std::ops::Range<usize>> {
        let mut visible = vec![false; self.height];
        for side in [&self.left, &self.right] {
            for (line, changed) in side.changed.iter().enumerate() {
                if *changed {
                    let row = side.rows[line];
                    visible[row.saturating_sub(context)..(row + context + 1).min(self.height)]
                        .fill(true);
                }
            }
        }
        let mut regions = Vec::new();
        let mut start = 0;
        while start < visible.len() {
            if visible[start] {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < visible.len() && !visible[end] {
                end += 1;
            }
            if end - start > 3 {
                regions.push(start..end);
            }
            start = end;
        }
        regions
    }
}

type LineKey = (String, u8, Option<String>);

pub fn align(left: &str, right: &str) -> Alignment {
    let indexes = SourceIndex::new(Arc::new(left.into()))
        .ok()
        .zip(SourceIndex::new(Arc::new(right.into())).ok());
    let (left_keys, right_keys) = if let Some((a, b)) = &indexes {
        (line_keys(left, a), line_keys(right, b))
    } else {
        // Retain plain-text alignment for incomplete or non-JSON input.
        let keys = |text: &str| {
            text.split('\n')
                .map(|line| (line.trim().into(), 0, None))
                .collect::<Vec<_>>()
        };
        (keys(left), keys(right))
    };
    align_keys(left, right, left_keys, right_keys)
}

pub(crate) fn align_matched(
    left: &str,
    right: &str,
    a: &SourceIndex,
    b: &SourceIndex,
    pairs: &std::collections::HashMap<String, Option<String>>,
) -> Alignment {
    let keys = |text: &str, index: &SourceIndex, right: bool| {
        let mut offset = 0;
        text.split('\n')
            .map(|line| {
                let trimmed = line.trim_start();
                let start = offset + line.len() - trimmed.len();
                offset += line.len() + 1;
                let path = index
                    .pointer_at_offset(start)
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let key = if right {
                    let mut ancestor = path.as_str();
                    loop {
                        if let Some(target) = pairs.get(ancestor) {
                            break target.as_ref().map_or_else(
                                || format!("right:{path}"),
                                |target| format!("matched:{}{}", target, &path[ancestor.len()..]),
                            );
                        }
                        if ancestor.is_empty() {
                            break format!("right:{path}");
                        }
                        ancestor = ancestor.rsplit_once('/').map_or("", |(parent, _)| parent);
                    }
                } else {
                    format!("matched:{path}")
                };
                let kind = match trimmed.as_bytes().first() {
                    None => 2,
                    Some(b'}' | b']') => 1,
                    _ => 0,
                };
                (key, kind, None)
            })
            .collect()
    };
    align_keys(left, right, keys(left, a, false), keys(right, b, true))
}

fn align_keys(
    left: &str,
    right: &str,
    left_keys: Vec<LineKey>,
    right_keys: Vec<LineKey>,
) -> Alignment {
    let mut result = Alignment {
        left: Side {
            source: left.into(),
            ..Default::default()
        },
        right: Side {
            source: right.into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let left: Vec<_> = left.split('\n').map(normalize_line).collect();
    let right: Vec<_> = right.split('\n').map(normalize_line).collect();
    let ops = capture_diff_slices_deadline(
        Algorithm::Patience,
        &left_keys,
        &right_keys,
        Some(web_time::Instant::now() + Duration::from_millis(100)),
    );
    for op in &ops {
        let a = op.old_range();
        let b = op.new_range();
        let changed = op.tag() != DiffTag::Equal;
        result
            .left
            .rows
            .extend((0..a.len()).map(|row| result.height + row));
        result.left.changed.extend(
            a.clone()
                .enumerate()
                .map(|(row, index)| changed || left[index] != right[b.start + row]),
        );
        result
            .right
            .rows
            .extend((0..b.len()).map(|row| result.height + row));
        result.right.changed.extend(
            b.clone()
                .enumerate()
                .map(|(row, index)| changed || right[index] != left[a.start + row]),
        );
        result.height += a.len().max(b.len());
    }
    result
}

/// Match corresponding JSON properties even when their values change. Closing
/// delimiters belong to their container, not to a similarly shaped sibling.
fn line_keys(text: &str, index: &SourceIndex) -> Vec<(String, u8, Option<String>)> {
    let mut offset = 0;
    text.split('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            let start = offset + line.len() - trimmed.len();
            offset += line.len() + 1;
            let kind = match trimmed.as_bytes().first() {
                None => 2,
                Some(b'}' | b']') => 1,
                _ => 0,
            };
            let (path, in_array) = index
                .alignment_path_at_offset(start)
                .ok()
                .flatten()
                .unwrap_or_default();
            (path, kind, in_array.then(|| normalize_line(line)))
        })
        .collect()
}

fn normalize_line(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let (mut quoted, mut escaped) = (false, false);
    for ch in line.chars() {
        if quoted {
            result.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if !ch.is_whitespace() {
            result.push(ch);
            quoted = ch == '"';
        }
    }
    if result.ends_with(',') {
        result.pop();
    }
    result
}
