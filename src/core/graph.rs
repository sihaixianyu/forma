use crate::{pointer_child, preview, summary};
use serde_json::Value;
use std::collections::HashMap;

pub const NODE_LIMIT: usize = 10_000;
pub const NODE_WIDTH: f32 = 270.0;

#[derive(serde::Serialize)]
pub struct Row {
    pub pointer: String,
    pub key: String,
    pub value: String,
    pub kind: &'static str,
    pub string: Option<String>,
}
#[derive(serde::Serialize)]
pub struct GraphNode {
    pub pointer: String,
    pub title: String,
    pub kind: &'static str,
    pub rows: Vec<Row>,
    pub count: usize,
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub parent: Option<usize>,
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub truncated: bool,
    #[serde(default)]
    scope_pointer: String,
    pointers: HashMap<String, usize>,
    children: Vec<Vec<usize>>,
    columns: Vec<Vec<usize>>,
    max_height: f32,
}

fn known_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "object" => Some("object"),
        "array" => Some("array"),
        "string" => Some("string"),
        "number" => Some("number"),
        "boolean" => Some("boolean"),
        "null" => Some("null"),
        _ => None,
    }
}

impl<'de> serde::Deserialize<'de> for Row {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Fields {
            pointer: String,
            key: String,
            value: String,
            kind: String,
            string: Option<String>,
        }
        let fields = Fields::deserialize(de)?;
        let kind = known_kind(&fields.kind)
            .ok_or_else(|| serde::de::Error::custom("Unknown graph value kind"))?;
        Ok(Self {
            pointer: fields.pointer,
            key: fields.key,
            value: fields.value,
            kind,
            string: fields.string,
        })
    }
}

impl<'de> serde::Deserialize<'de> for GraphNode {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Fields {
            pointer: String,
            title: String,
            kind: String,
            rows: Vec<Row>,
            count: usize,
            x: f32,
            y: f32,
            height: f32,
            parent: Option<usize>,
        }
        let fields = Fields::deserialize(de)?;
        let kind = known_kind(&fields.kind)
            .ok_or_else(|| serde::de::Error::custom("Unknown graph value kind"))?;
        Ok(Self {
            pointer: fields.pointer,
            title: fields.title,
            kind,
            rows: fields.rows,
            count: fields.count,
            x: fields.x,
            y: fields.y,
            height: fields.height,
            parent: fields.parent,
        })
    }
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
    }
}

/// Objects and arrays become connected cards, with indexed element rows for arrays.
pub fn build_graph(value: &Value) -> Graph {
    build_graph_root(value, String::new(), "root".into())
}

fn hover_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| text.len() <= crate::decode::MAX_PREVIEW_BYTES)
        .map(str::to_owned)
}

fn pointer_title(pointer: &str) -> String {
    pointer.rsplit_once('/').map_or_else(
        || "root".into(),
        |(_, key)| key.replace("~1", "/").replace("~0", "~"),
    )
}

/// Build a bounded local graph around a pointer omitted from the initial preview.
pub fn build_graph_at(value: &Value, pointer: &str) -> Option<Graph> {
    let selected = value.pointer(pointer)?;
    let mut path = pointer;
    loop {
        let current = value.pointer(path)?;
        if current.is_object() || current.is_array() || path.is_empty() {
            let mut graph = build_graph_root(current, path.into(), pointer_title(path));
            if !selected.is_object() && !selected.is_array() && !graph.covers_pointer(pointer) {
                let node = &mut graph.nodes[0];
                let key = pointer.rsplit_once('/').map_or("value", |(_, key)| key);
                let row = Row {
                    pointer: pointer.into(),
                    key: preview(&key.replace("~1", "/").replace("~0", "~"), 60).into(),
                    value: preview(&summary(selected), 100).into(),
                    kind: kind(selected),
                    string: hover_string(selected),
                };
                if node.rows.len() == 8 {
                    node.rows.pop();
                }
                node.rows.push(row);
            }
            attach_ancestor_context(value, path, &mut graph);
            return Some(graph);
        }
        path = path.rsplit_once('/')?.0;
    }
}

fn attach_ancestor_context(value: &Value, scope_pointer: &str, graph: &mut Graph) {
    let mut ancestor_pointers = Vec::new();
    let mut pointer = scope_pointer;
    while let Some((parent, _)) = pointer.rsplit_once('/') {
        ancestor_pointers.push(parent);
        if parent.is_empty() {
            break;
        }
        pointer = parent;
    }
    ancestor_pointers.reverse();

    let Some(scope_root) = graph.nodes.first() else {
        return;
    };
    let scope_y = scope_root.y;

    // Keep the focused node even for an unusually deep pointer. If the full chain
    // cannot fit, retain the ancestors closest to the focused scope.
    let context_limit = NODE_LIMIT.saturating_sub(1);
    if ancestor_pointers.len() > context_limit {
        ancestor_pointers.drain(..ancestor_pointers.len() - context_limit);
        graph.truncated = true;
    }
    let context_count = ancestor_pointers.len();
    if context_count == 0 {
        return;
    }

    while graph.nodes.len() + context_count > NODE_LIMIT {
        graph.nodes.pop();
        graph.truncated = true;
    }

    for node in &mut graph.nodes {
        node.x += context_count as f32 * 350.0;
        node.parent = node.parent.map(|parent| parent + context_count);
    }
    graph.nodes[0].parent = Some(context_count - 1);

    let focused_nodes = std::mem::take(&mut graph.nodes);
    graph.nodes.reserve(context_count + focused_nodes.len());
    for (index, pointer) in ancestor_pointers.into_iter().enumerate() {
        let Some(ancestor) = value.pointer(pointer) else {
            return;
        };
        graph.nodes.push(graph_node(
            ancestor,
            pointer.into(),
            pointer_title(pointer),
            index,
            index.checked_sub(1),
            scope_y,
        ));
    }
    graph.nodes.extend(focused_nodes);
    finalize_graph(graph);
}

fn row_value(value: &Value) -> String {
    if let Value::Array(array) = value {
        let mut parts: Vec<_> = array
            .iter()
            .take(4)
            .map(|value| preview(&summary(value), 24).to_owned())
            .collect();
        if array.len() > 4 {
            parts.push(format!("… {} items", array.len()));
        }
        preview(&format!("[{}]", parts.join(", ")), 100).into()
    } else {
        preview(&summary(value), 100).into()
    }
}

fn graph_node(
    value: &Value,
    pointer: String,
    title: String,
    depth: usize,
    parent: Option<usize>,
    y: f32,
) -> GraphNode {
    let count = match value {
        Value::Object(map) => map.len(),
        Value::Array(array) => array.len(),
        _ => 1,
    };
    let rows = match value {
        Value::Object(map) => map
            .iter()
            .take(8)
            .map(|(key, value)| Row {
                pointer: pointer_child(&pointer, key),
                key: preview(key, 60).into(),
                value: row_value(value),
                kind: kind(value),
                string: hover_string(value),
            })
            .collect(),
        Value::Array(array) => array
            .iter()
            .take(8)
            .enumerate()
            .map(|(index, value)| Row {
                pointer: pointer_child(&pointer, &index.to_string()),
                key: index.to_string(),
                value: row_value(value),
                kind: kind(value),
                string: hover_string(value),
            })
            .collect(),
        _ => vec![Row {
            pointer: pointer.clone(),
            key: "value".into(),
            value: preview(&summary(value), 100).into(),
            kind: kind(value),
            string: hover_string(value),
        }],
    };
    let height = 52.0 + 24.0 * count.min(8) as f32 + if count > 8 { 24.0 } else { 0.0 };
    GraphNode {
        pointer,
        title,
        kind: kind(value),
        rows,
        count,
        x: depth as f32 * 350.0,
        y,
        height,
        parent,
    }
}

fn build_graph_root(value: &Value, root_pointer: String, title: String) -> Graph {
    let _stage = crate::diagnostics::Stage::new("graph_build", 0);
    fn visit(
        value: &Value,
        pointer: String,
        title: String,
        depth: usize,
        parent: Option<usize>,
        graph: &mut Graph,
        next_y: &mut f32,
    ) -> Option<usize> {
        if graph.nodes.len() >= NODE_LIMIT {
            graph.truncated = true;
            return None;
        }
        let index = graph.nodes.len();
        graph.nodes.push(graph_node(
            value,
            pointer.clone(),
            title,
            depth,
            parent,
            *next_y,
        ));
        let height = graph.nodes[index].height;
        let mut first = None;
        let mut last = None;
        let mut child = |key: String, value: &Value, graph: &mut Graph, next_y: &mut f32| {
            if (value.is_object() || value.is_array())
                && let Some(child) = visit(
                    value,
                    pointer_child(&pointer, &key),
                    key,
                    depth + 1,
                    Some(index),
                    graph,
                    next_y,
                )
            {
                first.get_or_insert(graph.nodes[child].y);
                last = Some(graph.nodes[child].y);
            }
        };
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    child(key.clone(), value, graph, next_y);
                    if graph.truncated {
                        break;
                    }
                }
            }
            Value::Array(array) => {
                for (i, value) in array.iter().enumerate() {
                    child(i.to_string(), value, graph, next_y);
                    if graph.truncated {
                        break;
                    }
                }
            }
            _ => {}
        }
        if let (Some(first), Some(last)) = (first, last) {
            graph.nodes[index].y = (first + last) * 0.5;
        } else {
            *next_y += height + 36.0;
        }
        *next_y = next_y.max(graph.nodes[index].y + height + 36.0);
        Some(index)
    }
    let mut graph = Graph {
        scope_pointer: root_pointer.clone(),
        ..Graph::default()
    };
    visit(value, root_pointer, title, 0, None, &mut graph, &mut 0.0);
    finalize_graph(&mut graph);
    log::debug!(target: "forma", "event=graph_result nodes={} truncated={}", graph.nodes.len(), graph.truncated);
    graph
}

fn finalize_graph(graph: &mut Graph) {
    graph.pointers.clear();
    graph.children.clear();
    graph.columns.clear();
    graph.max_height = 0.0;
    graph.children.resize_with(graph.nodes.len(), Vec::new);
    for (index, node) in graph.nodes.iter().enumerate() {
        graph.pointers.insert(node.pointer.clone(), index);
        if let Some(parent) = node.parent {
            graph.children[parent].push(index);
        }
        let column = (node.x / 350.0) as usize;
        if graph.columns.len() <= column {
            graph.columns.resize_with(column + 1, Vec::new);
        }
        graph.columns[column].push(index);
        graph.max_height = graph.max_height.max(node.height);
    }
    for column in &mut graph.columns {
        column.sort_by(|a, b| graph.nodes[*a].y.total_cmp(&graph.nodes[*b].y));
    }
}

impl Graph {
    pub fn covers_pointer(&self, pointer: &str) -> bool {
        if !self.scope_pointer.is_empty()
            && pointer != self.scope_pointer
            && !pointer
                .strip_prefix(&self.scope_pointer)
                .is_some_and(|suffix| suffix.starts_with('/'))
        {
            return false;
        }
        self.container_for(pointer).is_some_and(|index| {
            let node = &self.nodes[index];
            node.pointer == pointer || node.rows.iter().any(|row| row.pointer == pointer)
        })
    }

    /// Scalar search hits belong to the nearest container card.
    pub fn container_for(&self, pointer: &str) -> Option<usize> {
        let mut pointer = pointer;
        loop {
            if let Some(index) = self.pointers.get(pointer) {
                return Some(*index);
            }
            pointer = pointer.rsplit_once('/')?.0;
        }
    }

    pub fn children(&self, index: usize) -> &[usize] {
        &self.children[index]
    }

    /// Query layout columns in O(log N + visible candidates), without walking the full graph.
    pub fn viewport_nodes(&self, x: f32, y: f32, width: f32, height: f32) -> Vec<usize> {
        let mut result = Vec::new();
        for (depth, column) in self.columns.iter().enumerate() {
            let column_x = depth as f32 * 350.0;
            if column_x > x + width || column_x + NODE_WIDTH < x {
                continue;
            }
            let start = column.partition_point(|index| self.nodes[*index].y < y - self.max_height);
            for index in &column[start..] {
                let node = &self.nodes[*index];
                if node.y > y + height {
                    break;
                }
                if node.y + node.height >= y {
                    result.push(*index);
                }
            }
        }
        result.sort_unstable();
        result
    }
}
