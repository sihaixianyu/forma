use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use json_lens::graph::{Graph, NODE_WIDTH};
use std::collections::{HashMap, HashSet};

/// Trim monotone curve axes before tessellation, retaining the original curve
/// within the viewport instead of flattening connections spanning a huge graph.
#[path = "canvas/edge.rs"]
mod edge;
use edge::visible_edge;

pub struct Canvas {
    zoom: f32,
    pan: Vec2,
    positions: HashMap<usize, Vec2>,
    collapsed: HashSet<usize>,
    fit: bool,
    focus: Option<String>,
    last_pointer: String,
    last_size: Vec2,
    last_rect: Rect,
    graph_id: usize,
    geometry_dirty: bool,
    hidden: Vec<bool>,
    bounds: Rect,
    viewport: Rect,
    visible: Vec<usize>,
    edges: Vec<usize>,
    pub edit_requested: Option<String>,
    pub reveal_requested: Option<String>,
    pub notice: Option<String>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            positions: HashMap::new(),
            collapsed: HashSet::new(),
            fit: true,
            focus: None,
            last_pointer: String::new(),
            last_size: Vec2::ZERO,
            last_rect: Rect::NOTHING,
            graph_id: 0,
            geometry_dirty: true,
            hidden: Vec::new(),
            bounds: Rect::NOTHING,
            viewport: Rect::NOTHING,
            visible: Vec::new(),
            edges: Vec::new(),
            edit_requested: None,
            reveal_requested: None,
            notice: None,
        }
    }
}

impl Canvas {
    pub fn reset_layout(&mut self) {
        self.positions.clear();
        self.collapsed.clear();
        self.focus = None;
        self.fit = true;
        self.geometry_dirty = true;
        self.notice = Some("Layout reset — card positions and folding restored".into());
    }

    pub fn reconcile(&mut self, old: &Graph, new: &Graph) {
        let translate = |index: usize| {
            let pointer = &old.nodes.get(index)?.pointer;
            let index = new.container_for(pointer)?;
            (new.nodes[index].pointer == *pointer).then_some(index)
        };
        self.positions = self
            .positions
            .iter()
            .filter_map(|(index, position)| translate(*index).map(|index| (index, *position)))
            .collect();
        self.collapsed = self
            .collapsed
            .iter()
            .filter_map(|index| translate(*index))
            .collect();
        self.geometry_dirty = true;
        self.graph_id = 0;
        // Rebuilt graphs can place the same selected node far from its old coordinates.
        // Recenter the selection while retaining matching card positions and folds.
        self.focus = Some(self.last_pointer.clone());
    }
    pub fn focus(&mut self, pointer: String) {
        self.focus = Some(pointer);
    }

    fn constrain_pan(&mut self, size: Vec2) {
        if !self.bounds.is_positive() {
            return;
        }
        let min = self.bounds.min.to_vec2() * self.zoom - Vec2::splat(40.0);
        let max = self.bounds.max.to_vec2() * self.zoom + Vec2::splat(40.0);
        let clamp = |pan: f32, min: f32, max: f32, size: f32| {
            if max - min <= size {
                size * 0.5 - (min + max) * 0.5
            } else {
                pan.clamp(size - max, -min)
            }
        };
        self.pan = Vec2::new(
            clamp(self.pan.x, min.x, max.x, size.x),
            clamp(self.pan.y, min.y, max.y, size.y),
        );
    }

    fn keep_content_visible(&mut self, graph: &Graph, size: Vec2) {
        let world = Rect::from_min_size((-self.pan / self.zoom).to_pos2(), size / self.zoom);
        if self.viewport == world.expand(16.0 / self.zoom) {
            return;
        }
        let node_rect = |index: usize| {
            let node = &graph.nodes[index];
            let pos = self
                .positions
                .get(&index)
                .copied()
                .unwrap_or(Vec2::new(node.x, node.y));
            Rect::from_min_size(pos.to_pos2(), Vec2::new(NODE_WIDTH, node.height))
        };
        let candidates =
            graph.viewport_nodes(world.min.x, world.min.y, world.width(), world.height());
        let visible = candidates
            .iter()
            .chain(self.positions.keys())
            .any(|index| !self.hidden[*index] && world.intersects(node_rect(*index)));
        if !visible
            && let Some(index) = graph
                .container_for(&self.last_pointer)
                .or_else(|| (!graph.nodes.is_empty()).then_some(0))
        {
            self.pan = size * 0.5 - node_rect(index).center().to_vec2() * self.zoom;
            self.constrain_pan(size);
            log::debug!(target: "json_lens", "event=graph_view_recovered reason=empty_viewport");
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, graph: &Graph, dark: bool, pointer: &mut String) {
        let graph_id = graph as *const Graph as usize;
        if self.graph_id != graph_id {
            self.graph_id = graph_id;
            self.geometry_dirty = true;
        }
        let accent = Color32::from_rgb(135, 109, 234);
        let muted = if dark {
            Color32::from_rgb(145, 154, 177)
        } else {
            Color32::from_rgb(125, 137, 159)
        };
        let text = if dark {
            Color32::from_rgb(225, 229, 240)
        } else {
            Color32::from_rgb(52, 58, 77)
        };
        let border = if dark {
            Color32::from_rgb(66, 72, 92)
        } else {
            Color32::from_rgb(215, 220, 235)
        };
        let surface = if dark {
            Color32::from_rgb(30, 33, 45)
        } else {
            Color32::WHITE
        };
        let background = if dark {
            Color32::from_rgb(21, 24, 34)
        } else {
            Color32::from_rgb(247, 248, 252)
        };
        if &self.last_pointer != pointer {
            self.focus = Some(pointer.clone());
            self.last_pointer = pointer.clone();
        }
        let height = ui.available_height().max(0.0);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            egui::Sense::click_and_drag(),
        );
        if (self.last_size - rect.size()).length_sq() > 1.0 {
            self.fit = true;
            self.last_size = rect.size();
        }
        let painter = ui.painter().with_clip_rect(rect);
        self.last_rect = rect;
        painter.rect_filled(rect, 8, background);
        if let Some(path) = self.focus.take()
            && let Some(index) = graph.container_for(&path)
        {
            let mut parent = graph.nodes[index].parent;
            while let Some(index) = parent {
                self.collapsed.remove(&index);
                self.geometry_dirty = true;
                parent = graph.nodes[index].parent;
            }
            self.zoom = self.zoom.max(0.75);
            let node = &graph.nodes[index];
            let pos = self
                .positions
                .get(&index)
                .copied()
                .unwrap_or(Vec2::new(node.x, node.y));
            self.pan =
                rect.size() * 0.5 - (pos + Vec2::new(NODE_WIDTH, node.height) * 0.5) * self.zoom;
            self.fit = false;
        }
        if self.geometry_dirty {
            self.hidden.clear();
            self.hidden.resize(graph.nodes.len(), false);
            self.bounds = Rect::NOTHING;
            for (index, node) in graph.nodes.iter().enumerate() {
                self.hidden[index] = node
                    .parent
                    .is_some_and(|parent| self.hidden[parent] || self.collapsed.contains(&parent));
                if !self.hidden[index] {
                    let pos = self
                        .positions
                        .get(&index)
                        .copied()
                        .unwrap_or(Vec2::new(node.x, node.y));
                    self.bounds = self.bounds.union(Rect::from_min_size(
                        pos.to_pos2(),
                        Vec2::new(NODE_WIDTH, node.height),
                    ));
                }
            }
            self.geometry_dirty = false;
            self.viewport = Rect::NOTHING;
        }
        if self.fit && self.bounds.is_positive() {
            self.zoom = ((rect.width() - 80.0) / self.bounds.width())
                .min((rect.height() - 80.0) / self.bounds.height())
                .clamp(0.08, 1.1);
            self.pan = rect.size() * 0.5 - self.bounds.center().to_vec2() * self.zoom;
            self.fit = false;
        }
        if response.contains_pointer() {
            let (wheel_zoom, wheel_pan, has_wheel, pinch) = ui.input(|input| {
                let mut zoom = 0.0_f32;
                let mut pan = Vec2::ZERO;
                let mut has_wheel = false;
                let mut pinch = 1.0_f32;
                for event in &input.events {
                    match event {
                        egui::Event::MouseWheel {
                            unit,
                            delta,
                            modifiers,
                        } => {
                            has_wheel = true;
                            let delta = match unit {
                                egui::MouseWheelUnit::Point => *delta,
                                egui::MouseWheelUnit::Line => *delta * 40.0,
                                egui::MouseWheelUnit::Page => *delta * rect.height(),
                            };
                            if modifiers.ctrl || modifiers.command || modifiers.mac_cmd {
                                zoom += delta.y;
                            } else if modifiers.shift {
                                pan.x += delta.x + delta.y;
                            } else {
                                pan += delta;
                            }
                        }
                        egui::Event::Zoom(factor) => pinch *= factor,
                        _ => {}
                    }
                }
                (zoom, pan, has_wheel, pinch)
            });
            if has_wheel {
                self.pan += wheel_pan;
            }
            let factor = (wheel_zoom * 0.003).exp() * pinch;
            if factor != 1.0 {
                let old = self.zoom;
                self.zoom = (self.zoom * factor).clamp(0.08, 2.5);
                let mouse = ui
                    .input(|input| input.pointer.hover_pos())
                    .unwrap_or(rect.center())
                    - rect.min;
                self.pan = mouse - (mouse - self.pan) * (self.zoom / old);
            }
        }
        self.constrain_pan(rect.size());
        self.keep_content_visible(graph, rect.size());
        let spacing = 24.0 * self.zoom.max(0.5);
        let board = Rect::from_min_max(
            rect.min + self.pan + self.bounds.min.to_vec2() * self.zoom - Vec2::splat(40.0),
            rect.min + self.pan + self.bounds.max.to_vec2() * self.zoom + Vec2::splat(40.0),
        );
        let grid = painter.with_clip_rect(rect.intersect(board));
        let origin = rect.min
            + Vec2::new(
                self.pan.x.rem_euclid(spacing),
                self.pan.y.rem_euclid(spacing),
            );
        let dot = if dark {
            Color32::from_rgb(49, 54, 70)
        } else {
            Color32::from_rgb(218, 223, 235)
        };
        let mut x = origin.x;
        while x < rect.right() {
            let mut y = origin.y;
            while y < rect.bottom() {
                grid.circle_filled(Pos2::new(x, y), 0.8, dot);
                y += spacing;
            }
            x += spacing;
        }
        painter.rect_stroke(
            board,
            6,
            Stroke::new(1.0_f32, border),
            egui::StrokeKind::Inside,
        );
        let world = Rect::from_min_size((-self.pan / self.zoom).to_pos2(), rect.size() / self.zoom)
            .expand(16.0 / self.zoom);
        if self.viewport != world {
            self.viewport = world;
            self.visible =
                graph.viewport_nodes(world.min.x, world.min.y, world.width(), world.height());
            self.visible.extend(self.positions.keys().copied());
            self.visible.sort_unstable();
            self.visible.dedup();
            self.visible.retain(|index| {
                let node = &graph.nodes[*index];
                let pos = self
                    .positions
                    .get(index)
                    .copied()
                    .unwrap_or(Vec2::new(node.x, node.y));
                !self.hidden[*index]
                    && world.intersects(Rect::from_min_size(
                        pos.to_pos2(),
                        Vec2::new(NODE_WIDTH, node.height),
                    ))
            });
            self.edges.clear();
            for &index in &self.visible {
                if graph.nodes[index].parent.is_some() {
                    self.edges.push(index);
                }
                let mut offscreen = 0;
                for &child in graph.children(index) {
                    if self.hidden[child] {
                        continue;
                    }
                    if self.visible.binary_search(&child).is_ok() {
                        self.edges.push(child);
                    } else if offscreen < 10 {
                        self.edges.push(child);
                        offscreen += 1;
                    }
                }
            }
            self.edges.sort_unstable();
            self.edges.dedup();
            log::debug!(target: "json_lens", "event=graph_view nodes={} visible={} edges={} zoom={:.3}", graph.nodes.len(), self.visible.len(), self.edges.len(), self.zoom);
        }
        let node_rect = |index: usize, pan: Vec2, positions: &HashMap<usize, Vec2>| {
            let node = &graph.nodes[index];
            let position = positions
                .get(&index)
                .copied()
                .unwrap_or(Vec2::new(node.x, node.y));
            Rect::from_min_size(
                rect.min + pan + position * self.zoom,
                Vec2::new(NODE_WIDTH, node.height) * self.zoom,
            )
        };
        for &index in &self.edges {
            let node = &graph.nodes[index];
            if self.hidden[index] {
                continue;
            }
            if let Some(parent) = node.parent {
                let a = node_rect(parent, self.pan, &self.positions).right_center();
                let b = node_rect(index, self.pan, &self.positions).left_center();
                let edge_bounds = Rect::from_two_pos(a, b).expand(60.0 * self.zoom);
                if !rect.intersects(edge_bounds) {
                    continue;
                }
                let bend = ((b.x - a.x).abs() * 0.5).max(30.0 * self.zoom);
                let curve = egui::epaint::CubicBezierShape::from_points_stroke(
                    [a, a + Vec2::new(bend, 0.0), b - Vec2::new(bend, 0.0), b],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(1.5_f32, border),
                );
                if let Some(curve) = visible_edge(curve, rect.expand(8.0)) {
                    painter.add(curve);
                }
                if rect.expand(8.0).contains(b) {
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            b,
                            b + Vec2::new(-6.0, -3.0) * self.zoom,
                            b + Vec2::new(-6.0, 3.0) * self.zoom,
                        ],
                        border,
                        Stroke::NONE,
                    ));
                }
            }
        }
        let selected = graph.container_for(pointer);
        let mut dragging_node = false;
        for &index in &self.visible {
            let node = &graph.nodes[index];
            if self.hidden[index] {
                continue;
            }
            let card = node_rect(index, self.pan, &self.positions);
            if !rect.intersects(card) {
                continue;
            }
            let node_response = ui.interact(
                card.intersect(rect),
                ui.id().with(("graph_node", index)),
                egui::Sense::click_and_drag(),
            );
            if node_response.dragged() {
                dragging_node = true;
                let pos = self
                    .positions
                    .entry(index)
                    .or_insert(Vec2::new(node.x, node.y));
                *pos += ui.input(|input| input.pointer.delta()) / self.zoom;
                self.geometry_dirty = true;
            }
            if node_response.clicked() {
                *pointer = node.pointer.clone();
                self.last_pointer = pointer.clone();
            }
            if node_response.double_clicked() && !self.collapsed.remove(&index) {
                self.collapsed.insert(index);
            }
            if node_response.double_clicked() {
                self.geometry_dirty = true;
            }
            node_response.on_hover_text(format!(
                "{}\n{} · {} children",
                if node.pointer.is_empty() {
                    "root"
                } else {
                    &node.pointer
                },
                node.kind,
                node.count
            ));
            let card_painter = painter.with_clip_rect(card.intersect(rect));
            card_painter.rect_filled(
                card.translate(Vec2::new(0.0, 3.0)),
                8,
                if dark {
                    Color32::from_black_alpha(30)
                } else {
                    Color32::from_black_alpha(8)
                },
            );
            card_painter.rect_filled(card, 8, surface);
            card_painter.rect_stroke(
                card,
                8,
                Stroke::new(
                    if selected == Some(index) {
                        2.0_f32
                    } else {
                        1.0_f32
                    },
                    if selected == Some(index) {
                        accent
                    } else {
                        border
                    },
                ),
                egui::StrokeKind::Inside,
            );
            let at = |x, y| card.min + Vec2::new(x, y) * self.zoom;
            card_painter.text(
                at(12.0, 15.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "{}  {}",
                    if node.kind == "array" { "[ ]" } else { "{ }" },
                    node.title
                ),
                FontId::proportional(13.0 * self.zoom),
                text,
            );
            card_painter.text(
                at(NODE_WIDTH - 12.0, 17.0),
                egui::Align2::RIGHT_TOP,
                if self.collapsed.contains(&index) {
                    "folded".into()
                } else {
                    node.count.to_string()
                },
                FontId::proportional(11.0 * self.zoom),
                muted,
            );
            card_painter.line_segment(
                [at(0.0, 44.0), at(NODE_WIDTH, 44.0)],
                Stroke::new(1.0_f32, border),
            );
            for (row_index, row) in node.rows.iter().enumerate() {
                let y = 54.0 + row_index as f32 * 24.0;
                let row_rect =
                    Rect::from_min_max(at(8.0, y), at(NODE_WIDTH - 8.0, y + 23.0)).intersect(rect);
                if row.pointer == *pointer {
                    card_painter.rect_filled(
                        row_rect,
                        3,
                        if dark {
                            Color32::from_rgb(61, 48, 93)
                        } else {
                            Color32::from_rgb(238, 231, 255)
                        },
                    );
                }
                if row_rect.is_positive() {
                    let response = ui.interact(
                        row_rect,
                        ui.id().with(("graph_row", index, row_index)),
                        egui::Sense::click(),
                    );
                    crate::decode_ui::show(&response, row.string.as_deref());
                    if response.clicked() {
                        *pointer = row.pointer.clone();
                        self.last_pointer = pointer.clone();
                    }
                    if response.double_clicked() {
                        if row.kind == "array" || row.kind == "object" {
                            self.focus = Some(row.pointer.clone());
                        } else {
                            self.edit_requested = Some(row.pointer.clone());
                        }
                    }
                }
                card_painter
                    .with_clip_rect(
                        Rect::from_min_max(at(10.0, y), at(98.0, y + 23.0)).intersect(rect),
                    )
                    .text(
                        at(12.0, y),
                        egui::Align2::LEFT_TOP,
                        &row.key,
                        FontId::monospace(11.0 * self.zoom),
                        muted,
                    );
                let color = match row.kind {
                    "string" => Color32::from_rgb(56, 161, 124),
                    "number" => Color32::from_rgb(190, 138, 66),
                    "boolean" => accent,
                    _ => text,
                };
                card_painter
                    .with_clip_rect(
                        Rect::from_min_max(at(104.0, y), at(NODE_WIDTH - 8.0, y + 23.0))
                            .intersect(rect),
                    )
                    .text(
                        at(106.0, y),
                        egui::Align2::LEFT_TOP,
                        &row.value,
                        FontId::monospace(11.0 * self.zoom),
                        color,
                    );
            }
            if node.count > 8 {
                card_painter.text(
                    at(12.0, node.height - 18.0),
                    egui::Align2::LEFT_TOP,
                    format!("{} more · reveal in Source", node.count - 8),
                    FontId::proportional(10.0 * self.zoom),
                    muted,
                );
            }
            painter.circle_filled(card.left_center(), 3.0 * self.zoom, accent);
            painter.circle_filled(card.right_center(), 3.0 * self.zoom, accent);
        }
        if let Some(index) = graph.container_for(pointer) {
            response.context_menu(|ui| {
                if ui.button("Reveal in source").clicked() {
                    self.reveal_requested = Some(pointer.clone());
                    ui.close();
                }
                if ui.button("Edit node").clicked() {
                    self.edit_requested = Some(pointer.clone());
                    ui.close();
                }
                ui.separator();
                if ui
                    .button(if self.collapsed.contains(&index) {
                        "Unfold branch"
                    } else {
                        "Fold branch"
                    })
                    .clicked()
                {
                    if !self.collapsed.remove(&index) {
                        self.collapsed.insert(index);
                    }
                    self.geometry_dirty = true;
                    self.notice = Some(
                        if self.collapsed.contains(&index) {
                            "Branch folded"
                        } else {
                            "Branch unfolded"
                        }
                        .into(),
                    );
                    ui.close();
                }
                if let Some(parent) = graph.nodes[index].parent {
                    if ui.button("Go to parent").clicked() {
                        *pointer = graph.nodes[parent].pointer.clone();
                        self.notice = Some("Parent node selected".into());
                        ui.close();
                    }
                    if ui.button("Toggle sibling branches").clicked() {
                        let fold = graph
                            .children(parent)
                            .iter()
                            .any(|sibling| *sibling != index && !self.collapsed.contains(sibling));
                        for sibling in graph.children(parent) {
                            if *sibling == index {
                                continue;
                            }
                            if fold {
                                self.collapsed.insert(*sibling);
                            } else {
                                self.collapsed.remove(sibling);
                            }
                        }
                        self.geometry_dirty = true;
                        self.notice = Some(
                            if fold {
                                "Sibling branches folded"
                            } else {
                                "Sibling branches unfolded"
                            }
                            .into(),
                        );
                        ui.close();
                    }
                }
            });
        }
        if !dragging_node
            && (response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Middle))
        {
            self.pan += ui.input(|input| input.pointer.delta());
            self.constrain_pan(rect.size());
            ui.ctx().request_repaint();
        }
        if graph.truncated {
            painter.text(
                rect.left_bottom() + Vec2::new(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                format!(
                    "Graph preview limited to {} cards; search covers the complete JSON.",
                    json_lens::graph::NODE_LIMIT
                ),
                FontId::proportional(11.0),
                muted,
            );
        }
        response.on_hover_text(
            "Ctrl/⌘ + wheel: zoom · wheel: pan · Shift + wheel: horizontal pan\nRight-click for selected node actions",
        );
    }
}

#[cfg(test)]
#[path = "canvas/tests.rs"]
mod tests;
