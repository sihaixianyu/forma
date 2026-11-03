use forma::{
    Format,
    edit::{rename_key, replace_value, source_range},
    graph::{NODE_WIDTH, build_graph},
    process,
};
use serde_json::json;
use std::sync::Arc;

#[test]
fn source_location_handles_unicode_escapes_duplicates_and_whitespace() {
    let text = "  {\"a/b~c\": {\"名字\": \"中文\\n🚀\"}, \"x\":1, \"x\":2}  ";
    assert_eq!(
        &text[source_range(text, "/a~1b~0c/名字").unwrap()],
        "\"中文\\n🚀\""
    );
    assert_eq!(&text[source_range(text, "/x").unwrap()], "2");
    assert!(source_range(text, "/a~2").is_err());
    assert_eq!(&text[source_range(text, "").unwrap()], text.trim());
    assert!(source_range("[1,2]", "/01").is_err());
}

#[test]
fn editing_one_value_preserves_surrounding_source_and_number_precision() {
    let text = "{\n\t\"n\" : 123456789012345678901234567890,\n\t\"s\" : \"\\u4e2d\"\n}";
    let doc = process(Arc::new(text.into()), Format::Keep).unwrap();
    let edited = replace_value(&doc, "/s", " \"中文🚀\" ").unwrap();
    assert_eq!(*edited.text, text.replace("\"\\u4e2d\"", "\"中文🚀\""));
    assert_eq!(&*doc.text, text);
    assert!(replace_value(&doc, "/s", "invalid").is_err());
    assert!(replace_value(&doc, "/missing", "null").is_err());
}

#[test]
fn key_rename_handles_escaped_quotes_and_rejects_collisions() {
    let text = r#"{"a\"b\\": 1, "existing":2}"#;
    let doc = process(Arc::new(text.into()), Format::Keep).unwrap();
    let renamed = rename_key(&doc, "/a\"b\\", "名字/键~").unwrap();
    assert_eq!(renamed.value.pointer("/名字~1键~0"), Some(&json!(1)));
    assert_eq!(
        renamed
            .value
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["名字/键~", "existing"]
    );
    assert!(rename_key(&doc, "/a\"b\\", "existing").is_err());
    assert!(rename_key(&doc, "", "name").is_err());
}

#[test]
fn viewport_index_agrees_with_full_scan() {
    let value = json!({"items":(0..500).map(|i| json!({"id":i, "nested":{"name":"example"}})).collect::<Vec<_>>()});
    let graph = build_graph(&value);
    for (x, y, width, height) in [
        (0.0, 0.0, 1000.0, 500.0),
        (340.0, 3000.0, 600.0, 400.0),
        (-50.0, -50.0, 400.0, 80.0),
        (0.0, 1000000.0, 1000.0, 100.0),
    ] {
        let expected: Vec<_> = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                n.x <= x + width
                    && n.x + NODE_WIDTH >= x
                    && n.y <= y + height
                    && n.y + n.height >= y
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(graph.viewport_nodes(x, y, width, height), expected);
    }
}

#[test]
fn source_click_resolves_keys_values_containers_and_array_items() {
    use forma::edit::pointer_at_offset;
    let text =
        " {\"a/b~c\": {\n   \"名字\": \"中文🚀\"},\"items\":[{\"id\":1},false],\"tail\":null} ";
    for (fragment, expected) in [
        ("a/b~c", "/a~1b~0c"),
        ("名字", "/a~1b~0c/名字"),
        ("中文", "/a~1b~0c/名字"),
        ("id", "/items/0/id"),
        ("false", "/items/1"),
        ("null", "/tail"),
    ] {
        assert_eq!(
            pointer_at_offset(text, text.find(fragment).unwrap())
                .unwrap()
                .as_deref(),
            Some(expected)
        );
    }
    let nested_key = text.find("名字").unwrap();
    let nested_indent = text[..nested_key].rfind('\n').unwrap() + 1;
    let nested_value = text.find("中文").unwrap();
    for offset in nested_indent..nested_value {
        assert_eq!(
            pointer_at_offset(text, offset).unwrap().as_deref(),
            Some("/a~1b~0c/名字"),
            "the whole property header must resolve to its final key at byte {offset}"
        );
    }
    assert_eq!(
        pointer_at_offset(text, text.find('[').unwrap())
            .unwrap()
            .as_deref(),
        Some("/items")
    );
    assert_eq!(
        pointer_at_offset(text, text.find("]").unwrap())
            .unwrap()
            .as_deref(),
        Some("/items")
    );
    assert_eq!(pointer_at_offset(text, 0).unwrap(), None);
    assert_eq!(pointer_at_offset(text, text.len()).unwrap(), None);
    assert!(pointer_at_offset("{bad}", 2).is_err());
}

#[test]
fn source_caret_assigns_comma_whitespace_by_line() {
    use forma::edit::pointer_at_offset;
    let text = "{\n  \"first\": 1,   \n    \"second\": 2\n}";
    let comma = text.find(',').unwrap();
    let newline = text[comma..].find('\n').unwrap() + comma;
    let second = text.find("second").unwrap();

    for offset in comma..newline {
        assert_eq!(
            pointer_at_offset(text, offset).unwrap().as_deref(),
            Some("/first"),
            "same-line whitespace after the comma belongs to the previous key"
        );
    }
    for offset in newline + 1..second {
        assert_eq!(
            pointer_at_offset(text, offset).unwrap().as_deref(),
            Some("/second"),
            "indentation on the next line belongs to that line's key"
        );
    }
}

#[test]
fn source_click_uses_absolute_offsets_in_a_large_document() {
    use forma::edit::pointer_at_offset;
    let text = format!(
        "{{\"padding\":\"{}\",\"target\":{{\"name\":\"中文\"}}}}",
        "x".repeat(2 * 1024 * 1024)
    );
    assert_eq!(
        pointer_at_offset(&text, text.find("中文").unwrap())
            .unwrap()
            .as_deref(),
        Some("/target/name")
    );
}

#[test]
fn source_caret_keeps_the_deep_property_at_value_boundaries() {
    use forma::edit::pointer_at_offset;
    let text = r#"{
  "transparent_param": {
    "people_hit_result": {
      "before": "中文",
      "dmp_18_user_ecom_8group": "新锐白领",
      "after": true
    }
  }
}"#;
    let pointer = "/transparent_param/people_hit_result/dmp_18_user_ecom_8group";
    let key = text.find("dmp_18_user_ecom_8group").unwrap();
    let value = text.find("新锐白领").unwrap();
    let value_end = value + "新锐白领".len() + 1;

    for offset in key..=value_end {
        assert_eq!(
            pointer_at_offset(text, offset).unwrap().as_deref(),
            Some(pointer),
            "caret boundary at byte {offset} must retain the deepest property"
        );
    }
}
