use forma::{compare, match_comparison_order};
use serde_json::{Value, json};

#[test]
fn reordered_similar_objects_report_only_the_changed_fields() {
    let left = json!({"items": [
        {"id": "a", "name": "Alice", "details": {"count": 1}},
        {"id": "b", "name": "Bob", "details": {"count": 2}}
    ]});
    let mut right = json!({"items": [
        {"name": "Bob", "details": {"count": 20}, "id": "b"},
        {"name": "Alice", "details": {"count": 10}, "id": "a"}
    ]});
    assert!(match_comparison_order(&left, &mut right));
    assert_eq!(right["items"][0]["id"], "a");
    assert_eq!(right["items"][1]["id"], "b");
    let differences = compare(&left, &right, 100);
    assert_eq!(differences.len(), 2);
    assert_eq!(differences[0].pointer, "/items/0/details/count");
    assert_eq!(differences[1].pointer, "/items/1/details/count");
    assert!(!match_comparison_order(&left, &mut right));
}

#[test]
fn changed_string_fields_match_by_content_instead_of_only_shared_key_names() {
    let left = json!({"items":[{"name":"aaa"},{"name":"bbb"}]});
    let mut right = json!({"items":[{"name":"bbc"},{"name":"aac"}]});
    assert!(match_comparison_order(&left, &mut right));
    assert_eq!(right, json!({"items":[{"name":"aac"},{"name":"bbc"}]}));
    assert_eq!(compare(&left, &right, 100).len(), 2);
    assert!(!match_comparison_order(&left, &mut right));
}

#[test]
fn exact_matches_are_reserved_before_fuzzy_matches_and_duplicates_are_preserved() {
    let left = json!([{"id":"a","v":1}, {"id":"a","v":2}, {"id":"a","v":2}]);
    let mut right = json!([{"id":"a","v":2}, {"id":"a","v":3}, {"id":"a","v":2}]);
    assert!(match_comparison_order(&left, &mut right));
    assert_eq!(
        right,
        json!([{"id":"a","v":3}, {"id":"a","v":2}, {"id":"a","v":2}])
    );
    assert_eq!(compare(&left, &right, 100).len(), 1);
}

#[test]
fn deletion_and_changed_match_keep_distinct_left_and_right_pointers() {
    let left = json!({"items":[{"id":1,"v":"a"},{"id":2,"v":"b"},{"id":3,"v":"c"}]});
    let mut right = json!({"items":[{"id":3,"v":"changed"},{"id":1,"v":"a"}]});
    match_comparison_order(&left, &mut right);
    assert_eq!(right["items"][0]["id"], 1);
    assert_eq!(right["items"][1]["id"], 3);
    let differences = compare(&left, &right, 100);
    assert_eq!(differences.len(), 2);
    assert_eq!(differences[0].left_pointer.as_deref(), Some("/items/1"));
    assert!(differences[0].right_pointer.is_none());
    assert_eq!(differences[1].left_pointer.as_deref(), Some("/items/2/v"));
    assert_eq!(differences[1].right_pointer.as_deref(), Some("/items/1/v"));
}

#[test]
fn added_values_and_nested_arrays_are_not_lost() {
    let left = json!({"items":[{"values":[1,2,1]}]});
    let mut right = json!({"items":[{"values":[1,1,2,3]}]});
    match_comparison_order(&left, &mut right);
    assert_eq!(right, json!({"items":[{"values":[1,2,1,3]}]}));
    let differences = compare(&left, &right, 100);
    assert_eq!(differences.len(), 1);
    assert!(differences[0].left_pointer.is_none());
    assert_eq!(
        differences[0].right_pointer.as_deref(),
        Some("/items/0/values/3")
    );
    assert!(compare(&json!([]), &json!([]), 100).is_empty());
}

#[test]
fn large_reversed_arrays_match_exact_elements_before_scoring_a_small_changed_remainder() {
    let left = Value::Array((0..2000).map(|id| json!({"id":id,"v":true})).collect());
    let mut right = Value::Array(
        (0..2000)
            .rev()
            .map(|id| json!({"id":id,"v":id != 700}))
            .collect(),
    );
    assert!(match_comparison_order(&left, &mut right));
    assert_eq!(right[700]["id"], 700);
    let differences = compare(&left, &right, 100);
    assert_eq!(differences.len(), 1);
    assert_eq!(differences[0].pointer, "/700/v");
}

#[test]
fn unrelated_large_arrays_retain_stable_order_when_fuzzy_budget_is_exceeded() {
    let left = Value::Array((0..300).map(|id| json!({"left":id})).collect());
    let mut right = Value::Array((0..300).map(|id| json!({"right":id})).collect());
    let original = right.clone();
    assert!(!match_comparison_order(&left, &mut right));
    assert_eq!(right, original);
    assert_eq!(compare(&left, &right, 10).len(), 10);
}
