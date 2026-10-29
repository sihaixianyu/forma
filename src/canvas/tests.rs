use super::*;
use json_lens::graph::build_graph;
use serde_json::json;

fn frame(
    ctx: &egui::Context,
    canvas: &mut Canvas,
    graph: &Graph,
    pointer: &mut String,
    time: f64,
    events: Vec<egui::Event>,
) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                canvas.show(ui, graph, false, pointer);
            });
        },
    );
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    }
}

#[test]
fn clicking_and_dragging_cards_updates_selection_and_position() {
    let ctx = egui::Context::default();
    let graph = build_graph(&json!({"child":{"name":"example"}}));
    let mut canvas = Canvas::default();
    let mut pointer = String::new();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.0, vec![]);
    let node = &graph.nodes[1];
    let pos = canvas.last_rect.min
        + canvas.pan
        + (Vec2::new(node.x, node.y) + Vec2::new(120.0, 20.0)) * canvas.zoom;
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        0.1,
        vec![egui::Event::PointerMoved(pos), button(pos, true)],
    );
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        0.2,
        vec![button(pos, false)],
    );
    assert_eq!(pointer, "/child");
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        1.0,
        vec![button(pos, true)],
    );
    let moved = pos + Vec2::new(40.0, 30.0);
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        1.1,
        vec![egui::Event::PointerMoved(moved)],
    );
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        1.2,
        vec![button(moved, false)],
    );
    assert!(canvas.positions[&1].x > node.x);
    assert!(canvas.positions[&1].y > node.y);
}

#[test]
fn search_focus_unfolds_ancestors_and_keeps_the_container_visible() {
    let ctx = egui::Context::default();
    let graph = build_graph(&json!({"child":{"nested":{"name":"example"}}}));
    let mut canvas = Canvas::default();
    let mut pointer = String::new();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.0, vec![]);
    canvas.collapsed.insert(0);
    canvas.collapsed.insert(1);
    pointer = "/child/nested/name".into();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.1, vec![]);
    assert!(canvas.collapsed.is_empty());
    let node = &graph.nodes[2];
    let center = canvas.last_rect.min
        + canvas.pan
        + Vec2::new(node.x + NODE_WIDTH * 0.5, node.y + node.height * 0.5) * canvas.zoom;
    assert!(canvas.last_rect.contains(center));
}

#[test]
fn clicking_a_value_selects_its_exact_pointer() {
    let ctx = egui::Context::default();
    let graph = build_graph(&json!({"child":{"name":"example"}}));
    let mut canvas = Canvas::default();
    let mut pointer = String::new();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.0, vec![]);
    let node = &graph.nodes[1];
    let position =
        canvas.last_rect.min + canvas.pan + Vec2::new(node.x + 140.0, node.y + 64.0) * canvas.zoom;
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        0.1,
        vec![egui::Event::PointerMoved(position), button(position, true)],
    );
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        0.2,
        vec![button(position, false)],
    );
    assert_eq!(pointer, "/child/name");
}

#[test]
fn document_updates_preserve_viewport_and_matching_card_state() {
    let old = build_graph(&json!({"child":{"name":"old"}}));
    let new = build_graph(&json!({"other":{}, "child":{"name":"new"}}));
    let mut canvas = Canvas::default();
    let index = old.container_for("/child").unwrap();
    canvas.positions.insert(index, Vec2::new(400.0, 80.0));
    canvas.collapsed.insert(index);
    canvas.pan = Vec2::new(100.0, 200.0);
    canvas.zoom = 0.9;
    canvas.reconcile(&old, &new);
    let index = new.container_for("/child").unwrap();
    assert_eq!(canvas.positions[&index], Vec2::new(400.0, 80.0));
    assert!(canvas.collapsed.contains(&index));
    assert_eq!(canvas.pan, Vec2::new(100.0, 200.0));
    assert_eq!(canvas.zoom, 0.9);
}

#[test]
fn rebuilt_large_graph_keeps_the_selected_root_in_view() {
    let ctx = egui::Context::default();
    let old = build_graph(&json!({"child": {"name": "old"}}));
    let new = build_graph(&json!(
        (0..2000)
            .map(|i| json!({"id": i, "child": {"value": i}}))
            .collect::<Vec<_>>()
    ));
    let mut canvas = Canvas::default();
    let mut pointer = String::new();
    frame(&ctx, &mut canvas, &old, &mut pointer, 0.0, vec![]);
    canvas.reconcile(&old, &new);
    frame(&ctx, &mut canvas, &new, &mut pointer, 0.1, vec![]);
    assert!(canvas.visible.contains(&0));
}

#[test]
fn wheel_over_a_card_in_a_large_graph_pans_and_zooms() {
    let ctx = egui::Context::default();
    let graph = build_graph(&json!(
        (0..5000)
            .map(|i| json!({"id": i, "child": {"value": i}}))
            .collect::<Vec<_>>()
    ));
    let mut canvas = Canvas::default();
    let mut pointer = String::new();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.0, vec![]);
    canvas.focus(String::new());
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.1, vec![]);
    let root = &graph.nodes[0];
    let mouse = canvas.last_rect.min
        + canvas.pan
        + Vec2::new(root.x + NODE_WIDTH * 0.5, root.y + root.height * 0.5) * canvas.zoom;
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        0.2,
        vec![egui::Event::PointerMoved(mouse)],
    );
    let old = canvas.pan;
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        0.3,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 40.0),
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_ne!(
        canvas.pan.y, old.y,
        "wheel over a graph card must pan the canvas"
    );
    let zoom = canvas.zoom;
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        0.4,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 40.0),
            modifiers: egui::Modifiers::CTRL,
        }],
    );
    assert!(
        canvas.zoom > zoom,
        "modified wheel over a graph card must zoom"
    );
}

#[test]
fn pan_and_zoom_cannot_leave_a_large_graph_completely_blank() {
    let ctx = egui::Context::default();
    let graph = build_graph(&json!(
        (0..5000)
            .map(|i| json!({"id": i, "data": [1,2,3]}))
            .collect::<Vec<_>>()
    ));
    let mut canvas = Canvas::default();
    let mut pointer = String::new();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.0, vec![]);
    for (i, zoom) in [0.08, 0.75, 1.0, 2.5].into_iter().enumerate() {
        canvas.zoom = zoom;
        canvas.fit = false;
        canvas.pan = Vec2::new(35.0, -600.0);
        canvas.viewport = Rect::NOTHING;
        frame(
            &ctx,
            &mut canvas,
            &graph,
            &mut pointer,
            (i + 1) as f64 * 0.1,
            vec![],
        );
        assert!(!canvas.visible.is_empty(), "zoom={zoom}");
        canvas.pan += Vec2::new(180.0, 600.0);
        canvas.focus(String::new());
        frame(
            &ctx,
            &mut canvas,
            &graph,
            &mut pointer,
            (i + 1) as f64 * 0.1 + 0.05,
            vec![],
        );
        assert!(
            canvas.visible.contains(&0),
            "Root must recover the selected root"
        );
    }
}

#[test]
fn huge_graph_connections_are_clipped_before_tessellation() {
    let clip = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 700.0));
    let curve = egui::epaint::CubicBezierShape::from_points_stroke(
        [
            Pos2::new(100.0, -1_000_000.0),
            Pos2::new(200.0, -1_000_000.0),
            Pos2::new(300.0, 1_000_000.0),
            Pos2::new(400.0, 1_000_000.0),
        ],
        false,
        Color32::TRANSPARENT,
        Stroke::new(1.5_f32, Color32::WHITE),
    );
    let trimmed = visible_edge(curve.clone(), clip).unwrap();
    assert!(
        trimmed
            .points
            .iter()
            .all(|point| clip.expand(0.5).contains(*point))
    );
    let shapes = trimmed.to_path_shapes(Some(0.5), Some(0.001));
    assert!(shapes.iter().map(|shape| shape.points.len()).sum::<usize>() < 200);
    assert!(
        visible_edge(
            curve,
            Rect::from_min_size(Pos2::new(800.0, 0.0), Vec2::new(100.0, 100.0))
        )
        .is_none()
    );
}

#[test]
fn modified_wheel_zooms_at_pointer_and_plain_wheel_pans() {
    for modifier in [egui::Modifiers::CTRL, egui::Modifiers::COMMAND] {
        let ctx = egui::Context::default();
        let value = serde_json::Value::Object(
            (0..12)
                .map(|index| {
                    (
                        format!("branch{index}"),
                        json!({"child":{"nested":{"name":"example"}}}),
                    )
                })
                .collect(),
        );
        let graph = build_graph(&value);
        let mut canvas = Canvas::default();
        let mut pointer = String::new();
        frame(&ctx, &mut canvas, &graph, &mut pointer, 0.0, vec![]);
        canvas.zoom = 1.0;
        canvas.fit = false;
        canvas.pan = canvas.last_rect.size() * 0.5 - canvas.bounds.center().to_vec2();
        let mouse = canvas.last_rect.center();
        let old_zoom = canvas.zoom;
        let world = (mouse - canvas.last_rect.min - canvas.pan) / canvas.zoom;
        let wheel = |modifiers| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 40.0),
            modifiers,
        };
        frame(
            &ctx,
            &mut canvas,
            &graph,
            &mut pointer,
            0.1,
            vec![egui::Event::PointerMoved(mouse), wheel(modifier)],
        );
        assert!(canvas.zoom > old_zoom);
        let world_after = (mouse - canvas.last_rect.min - canvas.pan) / canvas.zoom;
        assert!((world_after - world).length() < 0.01);
        let pan = canvas.pan;
        let zoom = canvas.zoom;
        frame(
            &ctx,
            &mut canvas,
            &graph,
            &mut pointer,
            0.2,
            vec![wheel(egui::Modifiers::NONE)],
        );
        assert_eq!(canvas.zoom, zoom);
        assert!((canvas.pan.y - pan.y - 40.0).abs() < 0.01);
        let pan = canvas.pan;
        frame(
            &ctx,
            &mut canvas,
            &graph,
            &mut pointer,
            0.3,
            vec![wheel(egui::Modifiers::SHIFT)],
        );
        assert!((canvas.pan.x - pan.x - 40.0).abs() < 0.01);
        assert_eq!(canvas.pan.y, pan.y);
    }
}

#[test]
fn repeated_extreme_wheel_pan_cannot_leave_the_finite_graph_board() {
    let ctx = egui::Context::default();
    let graph = build_graph(&json!({"child":{"nested":{"name":"example"}}}));
    let mut canvas = Canvas::default();
    let mut pointer = String::new();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 0.0, vec![]);
    canvas.zoom = 2.5;
    canvas.fit = false;
    let mouse = canvas.last_rect.center();
    for (step, direction) in [1.0, -1.0].into_iter().enumerate() {
        let mut previous = None;
        for repeat in 0..3 {
            frame(
                &ctx,
                &mut canvas,
                &graph,
                &mut pointer,
                0.1 + (step * 3 + repeat) as f64 * 0.1,
                vec![
                    egui::Event::PointerMoved(mouse),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::splat(direction * 100_000.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let content = Rect::from_min_max(
                canvas.last_rect.min + canvas.pan + canvas.bounds.min.to_vec2() * canvas.zoom,
                canvas.last_rect.min + canvas.pan + canvas.bounds.max.to_vec2() * canvas.zoom,
            );
            assert!(content.intersects(canvas.last_rect));
            if let Some(previous) = previous {
                assert_eq!(canvas.pan, previous);
            }
            previous = Some(canvas.pan);
        }
    }
    canvas.reset_layout();
    frame(&ctx, &mut canvas, &graph, &mut pointer, 1.0, vec![]);
    let pan = canvas.pan;
    frame(
        &ctx,
        &mut canvas,
        &graph,
        &mut pointer,
        1.1,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::splat(100_000.0),
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(canvas.pan, pan);
}
