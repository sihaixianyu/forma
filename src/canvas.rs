use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use json_lens::graph::{Graph, NODE_WIDTH};
use std::collections::HashSet;

#[path = "canvas/geometry.rs"]
mod geometry;
#[cfg(test)]
use geometry::visible_edge;

pub struct Canvas {
    zoom: f32,
    pan: Vec2,
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
    pub fn reconcile(&mut self, old: &Graph, new: &Graph) {
        let translate = |index: usize| {
            let pointer = &old.nodes.get(index)?.pointer;
            let index = new.container_for(pointer)?;
            (new.nodes[index].pointer == *pointer).then_some(index)
        };
        self.collapsed = self
            .collapsed
            .iter()
            .filter_map(|index| translate(*index))
            .collect();
        self.geometry_dirty = true;
        self.graph_id = 0;
        // Rebuilt graphs can place the same selected node far from its old coordinates.
        // Recenter the selection while retaining matching folds.
        self.focus = (!old.nodes.is_empty()).then(|| self.last_pointer.clone());
    }
    pub fn focus(&mut self, pointer: String) {
        self.focus = Some(pointer);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, graph: &Graph, dark: bool, pointer: &mut String) {
        let graph_id = graph as *const Graph as usize;
        if self.graph_id != graph_id {
            self.graph_id = graph_id;
            self.geometry_dirty = true;
        }
        let accent = if dark {
            Color32::from_rgb(135, 109, 234)
        } else {
            Color32::from_rgb(128, 104, 199)
        };
        let muted = if dark {
            Color32::from_rgb(145, 154, 177)
        } else {
            Color32::from_rgb(121, 132, 151)
        };
        let text = if dark {
            Color32::from_rgb(225, 229, 240)
        } else {
            Color32::from_rgb(48, 55, 73)
        };
        let border = if dark {
            Color32::from_rgb(66, 72, 92)
        } else {
            Color32::from_rgb(219, 224, 234)
        };
        let surface = if dark {
            Color32::from_rgb(30, 33, 45)
        } else {
            Color32::from_rgb(252, 253, 255)
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
            let pos = Vec2::new(node.x, node.y);
            self.pan =
                rect.size() * 0.5 - (pos + Vec2::new(NODE_WIDTH, node.height) * 0.5) * self.zoom;
            self.fit = false;
        }
        self.update_geometry(graph);
        if self.fit && self.bounds.is_positive() {
            self.zoom = ((rect.width() - 80.0) / self.bounds.width())
                .min((rect.height() - 80.0) / self.bounds.height())
                .clamp(f32::EPSILON, 1.1);
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
        let origin = rect.min
            + Vec2::new(
                self.pan.x.rem_euclid(spacing),
                self.pan.y.rem_euclid(spacing),
            );
        let dot = if dark {
            Color32::from_rgb(49, 54, 70)
        } else {
            Color32::from_rgb(220, 225, 236)
        };
        let mut x = origin.x;
        while x < rect.right() {
            let mut y = origin.y;
            while y < rect.bottom() {
                painter.circle_filled(Pos2::new(x, y), 0.8, dot);
                y += spacing;
            }
            x += spacing;
        }
        let world = Rect::from_min_size((-self.pan / self.zoom).to_pos2(), rect.size() / self.zoom)
            .expand(16.0 / self.zoom);
        self.update_viewport(graph, world);
        let selected = graph.container_for(pointer);
        let hovered_edge = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|position| rect.contains(*position))
            .and_then(|position| self.edge_at(graph, rect, position));
        self.paint_edges(graph, rect, &painter, border, accent, hovered_edge);
        if let Some(index) = hovered_edge {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Primary))
                && let Some(parent) = graph.nodes[index].parent
            {
                let destination = if selected == Some(index) {
                    parent
                } else {
                    index
                };
                *pointer = graph.nodes[destination].pointer.clone();
                self.last_pointer = pointer.clone();
                self.notice = Some(
                    if destination == parent {
                        "Previous node selected"
                    } else {
                        "Next node selected"
                    }
                    .into(),
                );
            }
        }
        let mut dragging_node = false;
        for &index in &self.visible {
            let node = &graph.nodes[index];
            if self.hidden[index] {
                continue;
            }
            let card = self.node_rect(graph, rect, index);
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
            let card_stroke = Stroke::new(
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
            );
            let at = |x, y| card.min + Vec2::new(x, y) * self.zoom;
            card_painter.text(
                at(12.0, 22.0),
                egui::Align2::LEFT_CENTER,
                format!(
                    "{}  {}",
                    if node.kind == "array" { "[ ]" } else { "{ }" },
                    node.title
                ),
                FontId::proportional(13.0 * self.zoom),
                text,
            );
            card_painter.text(
                at(NODE_WIDTH - 12.0, 22.0),
                egui::Align2::RIGHT_CENTER,
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
                let y = 52.0 + row_index as f32 * 24.0;
                let row_rect =
                    Rect::from_min_max(at(8.0, y), at(NODE_WIDTH - 8.0, y + 24.0)).intersect(rect);
                if row.pointer == *pointer {
                    card_painter.rect_filled(
                        row_rect,
                        4,
                        if dark {
                            Color32::from_rgb(61, 48, 93)
                        } else {
                            Color32::from_rgb(238, 235, 248)
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
                        Rect::from_min_max(at(10.0, y), at(98.0, y + 24.0)).intersect(rect),
                    )
                    .text(
                        at(12.0, y + 12.0),
                        egui::Align2::LEFT_CENTER,
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
                        Rect::from_min_max(at(104.0, y), at(NODE_WIDTH - 8.0, y + 24.0))
                            .intersect(rect),
                    )
                    .text(
                        at(106.0, y + 12.0),
                        egui::Align2::LEFT_CENTER,
                        &row.value,
                        FontId::monospace(11.0 * self.zoom),
                        color,
                    );
            }
            if node.count > 8 {
                card_painter.text(
                    at(12.0, node.height - 12.0),
                    egui::Align2::LEFT_CENTER,
                    format!("{} more · reveal in Source", node.count - 8),
                    FontId::proportional(10.0 * self.zoom),
                    muted,
                );
            }
            // Paint the outline after row highlights so a selected bottom row can
            // never cover the card's bottom edge.
            card_painter.rect_stroke(card, 8, card_stroke, egui::StrokeKind::Inside);
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
            "Click an edge to move between connected nodes\nCtrl/⌘ + wheel: zoom · wheel: pan · Shift + wheel: horizontal pan\nRight-click for selected node actions",
        );
    }
}

#[cfg(test)]
#[path = "canvas/tests.rs"]
mod tests;
