use similar::{Algorithm, DiffTag, TextDiff};
use std::time::Duration;

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Side {
    #[serde(skip)]
    pub source: String,
    /// Original line index -> aligned display row. Gaps never enter the document.
    pub rows: Vec<usize>,
    pub changed: Vec<bool>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Alignment {
    pub left: Side,
    pub right: Side,
    pub height: usize,
}

pub fn align(left: &str, right: &str) -> Alignment {
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
    // Ignore indentation while retaining the source's original offsets and line numbers.
    let left: Vec<_> = left.split('\n').map(str::trim).collect();
    let right: Vec<_> = right.split('\n').map(str::trim).collect();
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .timeout(Duration::from_millis(100))
        .diff_slices(&left, &right);
    for op in diff.ops() {
        let a = op.old_range();
        let b = op.new_range();
        let changed = op.tag() != DiffTag::Equal;
        result
            .left
            .rows
            .extend((0..a.len()).map(|row| result.height + row));
        result
            .left
            .changed
            .extend(std::iter::repeat_n(changed, a.len()));
        result
            .right
            .rows
            .extend((0..b.len()).map(|row| result.height + row));
        result
            .right
            .changed
            .extend(std::iter::repeat_n(changed, b.len()));
        result.height += a.len().max(b.len());
    }
    result
}
