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
    let _stage = crate::diagnostics::Stage::new("graph_build", 0);
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
                    string: value.as_str().map(str::to_owned),
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
                    string: value.as_str().map(str::to_owned),
                })
                .collect(),
            _ => vec![Row {
                pointer: pointer.clone(),
                key: "value".into(),
                value: preview(&summary(value), 100).into(),
                kind: kind(value),
                string: value.as_str().map(str::to_owned),
            }],
        };
        let height = 52.0 + 24.0 * count.min(8) as f32 + if count > 8 { 24.0 } else { 0.0 };
        graph.nodes.push(GraphNode {
            pointer: pointer.clone(),
            title,
            kind: kind(value),
            rows,
            count,
            x: depth as f32 * 350.0,
            y: *next_y,
            height,
            parent,
        });
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
    let mut graph = Graph::default();
    visit(
        value,
        String::new(),
        "root".into(),
        0,
        None,
        &mut graph,
        &mut 0.0,
    );
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
    log::debug!(target: "json_lens", "event=graph_result nodes={} truncated={}", graph.nodes.len(), graph.truncated);
    graph
}

impl Graph {
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
