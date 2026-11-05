use forma::review::{self, ArrayMode, ChangeKind, Options, Report};
use serde_json::{Value, json};
use std::sync::Arc;

fn report(a: Value, b: Value, options: Options) -> Report {
    let left = Arc::new(serde_json::to_string_pretty(&a).unwrap());
    let right = Arc::new(serde_json::to_string_pretty(&b).unwrap());
    review::build(left, right, &a, b, &options, true, 1000).unwrap()
}

#[test]
fn automatic_matching_prefers_a_unique_domain_id_over_changed_display_names() {
    let result = report(
        json!([{"order_id":1,"name":"A"},{"order_id":2,"name":"B"}]),
        json!([{"order_id":2,"name":"A"},{"order_id":1,"name":"B"}]),
        Options::default(),
    );
    let changes: Vec<_> = result
        .differences
        .iter()
        .filter(|d| d.kind == ChangeKind::Changed)
        .collect();
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().all(|d| d.pointer.ends_with("/name")));
    assert_eq!(
        result
            .differences
            .iter()
            .filter(|d| d.kind == ChangeKind::Moved)
            .count(),
        1
    );
}

#[test]
fn formatting_and_key_order_never_create_semantic_line_highlights() {
    let left = Arc::new("{\"z\":1,\"a\":2}".to_owned());
    let right = Arc::new("{\n\"a\": 2,\n\"z\": 1\n}".to_owned());
    let result = review::build(
        left.clone(),
        right.clone(),
        &serde_json::from_str(&left).unwrap(),
        serde_json::from_str(&right).unwrap(),
        &Options::default(),
        true,
        1000,
    )
    .unwrap();
    assert!(result.differences.is_empty());
    assert!(
        result
            .alignment
            .left
            .changed
            .iter()
            .chain(&result.alignment.right.changed)
            .all(|value| !value)
    );
}

#[test]
fn identity_matching_reports_a_move_and_continues_inside_the_changed_object() {
    let result = report(
        json!({"items":[{"id":1,"v":"a"},{"id":2,"v":"b"}]}),
        json!({"items":[{"id":2,"v":"changed"},{"id":1,"v":"a"}]}),
        Options::default(),
    );
    assert_eq!(
        result
            .differences
            .iter()
            .filter(|d| d.kind == ChangeKind::Moved)
            .count(),
        1
    );
    let changed = result
        .differences
        .iter()
        .find(|d| d.kind == ChangeKind::Changed)
        .unwrap();
    assert_eq!(changed.left_pointer.as_deref(), Some("/items/1/v"));
    assert_eq!(changed.right_pointer.as_deref(), Some("/items/1/v"));
    assert_eq!(
        changed.original_right_pointer.as_deref(),
        Some("/items/0/v")
    );
    assert_eq!(
        &result.alignment.right.source[changed.right_range.clone().unwrap()],
        "\"changed\""
    );
    assert!(!result.alignment.right.kinds.iter().all(Option::is_none));
}

#[test]
fn position_mode_preserves_array_order_and_exposes_the_different_values() {
    let result = report(
        json!([{"id":1},{"id":2}]),
        json!([{"id":2},{"id":1}]),
        Options {
            array_mode: ArrayMode::Position,
            ..Default::default()
        },
    );
    assert!(result.reordered.is_none());
    assert_eq!(result.alignment.left.rows, result.alignment.right.rows);
    assert_eq!(result.differences.len(), 2);
    assert!(
        result
            .differences
            .iter()
            .all(|d| d.kind == ChangeKind::Changed)
    );
}

#[test]
fn insertion_shifts_indexes_without_reporting_every_other_item_as_moved() {
    let result = report(
        json!([{"id":1},{"id":2}]),
        json!([{"id":0},{"id":1},{"id":2}]),
        Options::default(),
    );
    assert_eq!(result.differences.len(), 1);
    assert_eq!(result.differences[0].kind, ChangeKind::Added);
    assert!(result.differences[0].left_range.is_none());
    assert!(result.differences[0].right_range.is_some());
}

#[test]
fn duplicate_or_missing_identity_fields_produce_a_visible_warning() {
    let options = Options {
        array_mode: ArrayMode::Key,
        identity_key: "id".into(),
        ..Default::default()
    };
    for right in [json!([{"id":1},{"id":1}]), json!([{"v":1}])] {
        let result = report(json!([{"id":1},{"id":2}]), right, options.clone());
        assert!(!result.alignment.warnings.is_empty());
        assert!(
            result
                .differences
                .iter()
                .all(|d| d.kind != ChangeKind::Moved)
        );
    }
}

#[test]
fn fuzzy_ties_are_explicitly_marked_uncertain() {
    let result = report(
        json!([{"v":1},{"v":2}]),
        json!([{"v":3},{"v":4}]),
        Options::default(),
    );
    assert!(!result.alignment.warnings.is_empty());
    assert!(result.differences.iter().all(|d| d.uncertain));
}

#[test]
fn array_rule_is_scoped_to_its_requested_path() {
    let result = report(
        json!({"items":[{"id":1},{"id":2}],"other":[{"id":1},{"id":2}]}),
        json!({"items":[{"id":2},{"id":1}],"other":[{"id":2},{"id":1}]}),
        Options {
            array_mode: ArrayMode::Position,
            array_path: "/items".into(),
            ..Default::default()
        },
    );
    assert!(
        result
            .differences
            .iter()
            .any(|d| d.kind == ChangeKind::Moved && d.pointer.starts_with("/other"))
    );
    assert!(
        result
            .differences
            .iter()
            .filter(|d| d.pointer.starts_with("/items"))
            .all(|d| d.kind == ChangeKind::Changed)
    );
}

#[test]
fn hidden_regions_keep_three_context_rows_around_each_change() {
    let left: serde_json::Map<_, _> = (0..100).map(|i| (format!("key{i:03}"), json!(i))).collect();
    let a = Value::Object(left);
    let mut b = a.clone();
    b["key050"] = json!("changed type");
    let result = report(a, b, Options::default());
    assert_eq!(result.differences.len(), 1);
    assert_eq!(result.differences[0].kind, ChangeKind::TypeChanged);
    let line = result
        .alignment
        .left
        .changed
        .iter()
        .position(|v| *v)
        .unwrap();
    let row = result.alignment.left.rows[line];
    let regions = result.alignment.unchanged_regions(3);
    assert!(regions.len() >= 2);
    for visible in row - 3..=row + 3 {
        assert!(regions.iter().all(|r| !r.contains(&visible)));
    }
}
