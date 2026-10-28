use json_lens::{Format, compare, csv_to_json, json_to_csv, pointer_child, preview, process};
use serde_json::{Value, json};
use std::sync::Arc;

#[test]
fn retains_source_order_and_exact_numbers() {
    let source = Arc::new(r#" {"z":123456789012345678901234567890,"a":1.2300e+42} "#.to_owned());
    let doc = process(source.clone(), Format::Keep).unwrap();
    assert!(Arc::ptr_eq(&source, &doc.text));
    let compact = process(source, Format::Compact).unwrap();
    assert_eq!(
        &*compact.text,
        r#"{"z":123456789012345678901234567890,"a":1.2300e+42}"#
    );
}

#[test]
fn cached_format_reuses_the_value_and_sort_preserves_the_original() {
    let original = process(Arc::new(r#"{"z":1,"a":2}"#.into()), Format::Keep).unwrap();
    let pretty = json_lens::format_document(&original, Format::Pretty).unwrap();
    assert!(Arc::ptr_eq(&original.value, &pretty.value));
    let sorted = json_lens::format_document(&original, Format::SortAscending).unwrap();
    assert!(!Arc::ptr_eq(&original.value, &sorted.value));
    assert_eq!(
        original
            .value
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(
        sorted.value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["a", "z"]
    );
}

#[test]
fn recursive_sort_leaves_arrays_in_order() {
    let source = Arc::new(r#"{"z":[{"b":1,"a":2},3],"a":true}"#.to_owned());
    let doc = process(source.clone(), Format::SortAscending).unwrap();
    assert_eq!(
        doc.value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["a", "z"]
    );
    assert_eq!(
        doc.value["z"][0]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    let descending = process(source, Format::SortDescending).unwrap();
    assert_eq!(
        descending
            .value
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(
        descending.value["z"][0]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["b", "a"]
    );
    assert_eq!(descending.value["z"][1], 3);
}

#[test]
fn rejects_invalid_documents_with_location() {
    for text in [
        "{\"x\":}",
        "[1,]",
        "{} {}",
        "{ /* comment */ }",
        "\"\\x\"",
        "01",
    ] {
        let error = process(Arc::new(text.into()), Format::Keep).err().unwrap();
        assert!(error.contains("line"), "{error}");
        assert!(error.contains("column"), "{error}");
    }
}

#[test]
fn nested_strings_only_expand_valid_containers() {
    let doc = process(
        Arc::new(json!({"a":"{\"b\":\"[1,2]\"}", "b":"{bad}", "c":"123"}).to_string()),
        Format::Nested,
    )
    .unwrap();
    assert_eq!(*doc.value, json!({"a":{"b":[1,2]}, "b":"{bad}", "c":"123"}));
}

#[test]
fn unicode_preview_and_json_pointer_are_safe() {
    assert_eq!(preview("中🚀a", 5), "中");
    let value = json!({"a/b~c": {"": 42}});
    let pointer = pointer_child(&pointer_child("", "a/b~c"), "");
    assert_eq!(value.pointer(&pointer), Some(&json!(42)));
}

#[test]
fn comparison_handles_missing_null_and_ignores_object_order() {
    let a: Value = serde_json::from_str(r#"{"b":null,"a":[1,2]}"#).unwrap();
    let b: Value = serde_json::from_str(r#"{"a":[1,2],"b":null}"#).unwrap();
    assert!(compare(&a, &b, 1000).is_empty());
    let diffs = compare(&a, &json!({"a":[3]}), 1000);
    assert_eq!(diffs.len(), 3);
    assert!(
        diffs
            .iter()
            .any(|d| d.pointer == "/b" && d.right == "(missing)")
    );
    assert_eq!(compare(&json!([1, 2, 3]), &json!([4, 5, 6]), 2).len(), 2);
    assert!(compare(&a, &b, 0).is_empty());
}

#[test]
fn csv_quotes_and_sparse_columns_round_trip() {
    let value = json!([{"a":"hello, world", "b":"line\nquote\""}, {"a":"中文", "c":"extra"}]);
    let csv = String::from_utf8(json_to_csv(&value).unwrap()).unwrap();
    let imported = csv_to_json(&csv).unwrap();
    assert_eq!(imported[0]["a"], "hello, world");
    assert_eq!(imported[0]["b"], "line\nquote\"");
    assert_eq!(imported[1]["c"], "extra");
    assert!(json_to_csv(&json!([1, 2])).is_err());
}
