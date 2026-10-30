use super::*;

/// Trim monotone curve axes before tessellation, retaining the original curve
/// within the viewport instead of flattening connections spanning a huge graph.
pub(super) fn visible_edge(
    curve: egui::epaint::CubicBezierShape,
    clip: Rect,
) -> Option<egui::epaint::CubicBezierShape> {
    let mut range = 0.0_f32..1.0_f32;
    for vertical in [true, false] {
        let coordinate = |point: Pos2| if vertical { point.y } else { point.x };
        let values = curve.points.map(coordinate);
        let increasing = values.windows(2).all(|pair| pair[0] <= pair[1]);
        let decreasing = values.windows(2).all(|pair| pair[0] >= pair[1]);
        if !increasing && !decreasing {
            continue;
        }
        let (min, max) = if vertical {
            (clip.top(), clip.bottom())
        } else {
            (clip.left(), clip.right())
        };
        let low = values[0].min(values[3]);
        let high = values[0].max(values[3]);
        if high < min || low > max {
            return None;
        }
        if high - low < 0.001 {
            continue;
        }
        let at = |target: f32| {
            if increasing && target <= values[0] || decreasing && target >= values[0] {
                return 0.0;
            }
            if increasing && target >= values[3] || decreasing && target <= values[3] {
                return 1.0;
            }
            let mut a = 0.0;
            let mut b = 1.0;
            for _ in 0..24 {
                let middle = (a + b) * 0.5;
                if (coordinate(curve.sample(middle)) < target) == increasing {
                    a = middle;
                } else {
                    b = middle;
                }
            }
            (a + b) * 0.5
        };
        let a = at(min);
        let b = at(max);
        range.start = range.start.max(a.min(b));
        range.end = range.end.min(a.max(b));
        if range.start >= range.end {
            return None;
        }
    }
    Some(if range.start == 0.0 && range.end == 1.0 {
        curve
    } else {
        curve.split_range(range)
    })
}

impl Canvas {
    pub(super) fn node_rect(&self, graph: &Graph, rect: Rect, index: usize) -> Rect {
        let node = &graph.nodes[index];
        let position = Vec2::new(node.x, node.y);
        Rect::from_min_size(
            rect.min + self.pan + position * self.zoom,
            Vec2::new(NODE_WIDTH, node.height) * self.zoom,
        )
    }

    pub(super) fn paint_edges(
        &self,
        graph: &Graph,
        rect: Rect,
        painter: &egui::Painter,
        border: Color32,
        accent: Color32,
        hovered: Option<usize>,
    ) {
        for &index in &self.edges {
            let node = &graph.nodes[index];
            if self.hidden[index] {
                continue;
            }
            if node.parent.is_some() {
                let Some(mut curve) = self.edge_curve(graph, rect, index) else {
                    continue;
                };
                let b = curve.points[3];
                let color = if hovered == Some(index) {
                    accent
                } else {
                    border
                };
                curve.stroke =
                    Stroke::new(if hovered == Some(index) { 2.5 } else { 1.5 }, color).into();
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
                        color,
                        Stroke::NONE,
                    ));
                }
            }
        }
    }

    pub(super) fn edge_curve(
        &self,
        graph: &Graph,
        rect: Rect,
        index: usize,
    ) -> Option<egui::epaint::CubicBezierShape> {
        let node = graph.nodes.get(index)?;
        let parent = node.parent?;
        let a = self.node_rect(graph, rect, parent).right_center();
        let b = self.node_rect(graph, rect, index).left_center();
        if !rect.intersects(Rect::from_two_pos(a, b).expand(60.0 * self.zoom)) {
            return None;
        }
        let bend = ((b.x - a.x).abs() * 0.5).max(30.0 * self.zoom);
        Some(egui::epaint::CubicBezierShape::from_points_stroke(
            [a, a + Vec2::new(bend, 0.0), b - Vec2::new(bend, 0.0), b],
            false,
            Color32::TRANSPARENT,
            Stroke::NONE,
        ))
    }

    pub(super) fn edge_at(&self, graph: &Graph, rect: Rect, position: Pos2) -> Option<usize> {
        if self
            .visible
            .iter()
            .any(|index| self.node_rect(graph, rect, *index).contains(position))
        {
            return None;
        }
        let threshold = (6.0_f32.max(5.0 * self.zoom)).powi(2);
        self.edges
            .iter()
            .filter_map(|index| {
                let curve = self.edge_curve(graph, rect, *index)?;
                let mut distance = f32::INFINITY;
                let mut previous = curve.points[0];
                for step in 1..=24 {
                    let current = curve.sample(step as f32 / 24.0);
                    distance = distance.min(segment_distance_squared(position, previous, current));
                    previous = current;
                }
                (distance <= threshold).then_some((*index, distance))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(index, _)| index)
    }

    pub(super) fn constrain_pan(&mut self, size: Vec2) {
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

    pub(super) fn keep_content_visible(&mut self, graph: &Graph, size: Vec2) {
        let world = Rect::from_min_size((-self.pan / self.zoom).to_pos2(), size / self.zoom);
        if self.viewport == world.expand(16.0 / self.zoom) {
            return;
        }
        let node_rect = |index: usize| {
            let node = &graph.nodes[index];
            let pos = Vec2::new(node.x, node.y);
            Rect::from_min_size(pos.to_pos2(), Vec2::new(NODE_WIDTH, node.height))
        };
        let candidates =
            graph.viewport_nodes(world.min.x, world.min.y, world.width(), world.height());
        let visible = candidates
            .iter()
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

    pub(super) fn update_geometry(&mut self, graph: &Graph) {
        if self.geometry_dirty {
            self.hidden.clear();
            self.hidden.resize(graph.nodes.len(), false);
            self.bounds = Rect::NOTHING;
            for (index, node) in graph.nodes.iter().enumerate() {
                self.hidden[index] = node
                    .parent
                    .is_some_and(|parent| self.hidden[parent] || self.collapsed.contains(&parent));
                if !self.hidden[index] {
                    let pos = Vec2::new(node.x, node.y);
                    self.bounds = self.bounds.union(Rect::from_min_size(
                        pos.to_pos2(),
                        Vec2::new(NODE_WIDTH, node.height),
                    ));
                }
            }
            self.geometry_dirty = false;
            self.viewport = Rect::NOTHING;
        }
    }

    pub(super) fn update_viewport(&mut self, graph: &Graph, world: Rect) {
        if self.viewport != world {
            self.viewport = world;
            self.visible =
                graph.viewport_nodes(world.min.x, world.min.y, world.width(), world.height());
            self.visible.sort_unstable();
            self.visible.dedup();
            self.visible.retain(|index| {
                let node = &graph.nodes[*index];
                let pos = Vec2::new(node.x, node.y);
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
    }
}

fn segment_distance_squared(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let segment = end - start;
    let length = segment.length_sq();
    if length <= f32::EPSILON {
        return point.distance_sq(start);
    }
    let t = ((point - start).dot(segment) / length).clamp(0.0, 1.0);
    point.distance_sq(start + segment * t)
}
