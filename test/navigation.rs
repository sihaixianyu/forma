use json_lens::{
    graph::{NODE_WIDTH, build_graph},
    search::{TEXT_LIMIT, find_text, find_tree},
};
use serde_json::json;

#[test]
fn text_search_preserves_unicode_offsets_and_line_boundaries() {
    let text = "中文🚀\n名字 name\n名字";
    let result = find_text(text, "名字");
    assert_eq!(result.total, 2);
    for range in result.ranges {
        assert_eq!(&text[range], "名字");
    }
    assert_eq!(find_text(text, "\n名字").total, 2);
    assert_eq!(find_text(text, "NAME").total, 0);
    assert_eq!(find_text(text, "").total, 0);
}

#[test]
fn text_search_counts_every_match_beyond_navigation_limit() {
    let result = find_text(&"x ".repeat(TEXT_LIMIT + 17), "x");
    assert_eq!(result.total, TEXT_LIMIT + 17);
    assert_eq!(result.ranges.len(), TEXT_LIMIT);
}

#[test]
fn structured_search_uses_decoded_values_and_valid_pointers() {
    let value = json!({"a/b~c": {"名字": "中文🚀"}, "literal": "name", "items": [{"name":"first"}, {"name":"second"}]});
    let result = find_tree(&value, "名字");
    assert_eq!(result.total, 1);
    assert_eq!(result.hits[0].pointer, "/a~1b~0c/名字");
    assert_eq!(
        value.pointer(&result.hits[0].pointer),
        Some(&json!("中文🚀"))
    );
    assert_eq!(find_tree(&value, "🚀").total, 1);
    assert_eq!(find_tree(&value, "name").total, 3);
    assert_eq!(find_tree(&value, "").total, 0);
}

#[test]
fn graph_connects_containers_and_locates_scalar_search_hits() {
    let value = json!({"a/b~c": {"name":"x"}, "items":[{"id":1},{"id":2}]});
    let graph = build_graph(&value);
    assert_eq!(graph.nodes.len(), 5);
    assert!(!graph.truncated);
    let container = graph.container_for("/a~1b~0c/name").unwrap();
    assert_eq!(graph.nodes[container].pointer, "/a~1b~0c");
    for (index, node) in graph.nodes.iter().enumerate() {
        assert!(value.pointer(&node.pointer).is_some());
        assert!(node.height > 0.0 && node.x.is_finite() && node.y.is_finite());
        if let Some(parent) = node.parent {
            assert!(parent < index);
            assert!(graph.nodes[parent].x + NODE_WIDTH < node.x);
        }
    }
    let first = graph.container_for("/items/0/id").unwrap();
    let second = graph.container_for("/items/1/id").unwrap();
    assert!(graph.nodes[first].y + graph.nodes[first].height < graph.nodes[second].y);
    let items = graph.container_for("/items").unwrap();
    assert_eq!(graph.nodes[first].parent, Some(items));
    assert_eq!(graph.nodes[second].parent, Some(items));
}

#[test]
fn arrays_show_indexed_rows_and_keep_nested_source_pointers() {
    let value = json!({"tags": ["中文", "rust"], "empty": [], "a/b": [[{"name":"nested"}]], "items": [1, {"name":"one"}, [null, {"name":"two"}]]});
    let graph = build_graph(&value);
    for node in graph.nodes.iter().filter(|node| node.kind == "array") {
        assert_eq!(node.rows.len(), node.count.min(8));
        for (index, row) in node.rows.iter().enumerate() {
            assert_eq!(row.key, index.to_string());
            assert!(value.pointer(&row.pointer).is_some());
        }
        assert!(node.height >= 52.0);
        assert_eq!(
            node.count,
            value
                .pointer(&node.pointer)
                .unwrap()
                .as_array()
                .unwrap()
                .len()
        );
    }
    let tags = graph.nodes[0]
        .rows
        .iter()
        .find(|row| row.key == "tags")
        .unwrap();
    assert_eq!(tags.value, "[\"中文\", \"rust\"]");
    assert_eq!(
        graph.nodes[0]
            .rows
            .iter()
            .find(|row| row.key == "empty")
            .unwrap()
            .value,
        "[]"
    );
    for pointer in ["/a~1b/0/0", "/items/1", "/items/2/1"] {
        let index = graph.container_for(&format!("{pointer}/name")).unwrap();
        assert_eq!(graph.nodes[index].pointer, pointer);
        let parent = graph.nodes[index].parent.unwrap();
        assert_eq!(
            graph.nodes[parent].pointer,
            pointer.rsplit_once('/').unwrap().0
        );
        assert_eq!(graph.nodes[parent].kind, "array");
        assert!(value.pointer(pointer).unwrap().is_object());
    }
    assert_eq!(
        graph.container_for("/items/0"),
        graph.container_for("/items")
    );
    assert_eq!(
        graph.nodes[graph.container_for("/a~1b/0").unwrap()].pointer,
        "/a~1b/0"
    );
    let root = build_graph(&json!([1, [2, {"name":"root list"}]]));
    assert_eq!(root.nodes.len(), 3);
    assert_eq!(root.nodes[2].pointer, "/1/1");
    assert_eq!(root.nodes[2].parent, Some(1));
}

#[test]
fn scalar_document_gets_a_graph_card() {
    let graph = build_graph(&json!("中文"));
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].rows[0].value, "\"中文\"");
    assert_eq!(graph.container_for(""), Some(0));
}

#[test]
fn large_file_search_reaches_beyond_the_editor_preview() {
    let text = format!("{}\n中文目标🚀", " ".repeat(2 * 1024 * 1024));
    let result = find_text(&text, "目标🚀");
    assert_eq!(result.total, 1);
    assert!(result.ranges[0].start > 1024 * 1024);
    assert_eq!(&text[result.ranges[0].clone()], "目标🚀");
}

#[test]
fn graph_preview_cap_preserves_valid_edges_and_reports_truncation() {
    let value = serde_json::Value::Array(
        (0..json_lens::graph::NODE_LIMIT + 1)
            .map(|i| json!({"id":i}))
            .collect(),
    );
    let graph = build_graph(&value);
    assert_eq!(graph.nodes.len(), json_lens::graph::NODE_LIMIT);
    assert!(graph.truncated);
    assert!(
        graph
            .nodes
            .iter()
            .skip(1)
            .all(|node| node.parent == Some(0))
    );
}
