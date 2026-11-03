use super::*;

#[test]
fn comparison_footer_reports_each_inputs_own_json_status() {
    let mut app = ready_app();
    app.view = View::Compare;
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    for (left, right, left_status, right_status) in [
        ("{}", "{}", "Valid JSON", "Valid JSON"),
        ("{", "{}", "Invalid JSON", "Valid JSON"),
        ("{}", "{", "Valid JSON", "Invalid JSON"),
        ("{", "{", "Invalid JSON", "Invalid JSON"),
        ("{}", "", "Valid JSON", "Awaiting JSON"),
    ] {
        app.compare_text = right.into();
        app.right_syntax_error = None;
        app.right_json_valid = false;
        app.submit(Job::Process(Arc::new(left.into()), Format::Keep));
        finish(&mut app, receiver.recv().unwrap());
        if !right.is_empty() {
            app.submit(Job::Compare(app.full_text.clone(), Arc::new(right.into())));
            finish(&mut app, receiver.recv().unwrap());
        }
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        );
        for (expected, right_pane) in [(left_status, false), (right_status, true)] {
            assert!(
                output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.job.text == expected
                    && text.pos.y >= 774.0
                    && (text.pos.x >= 640.0) == right_pane)
                }),
                "missing {expected} in right_pane={right_pane}"
            );
        }
        if right_status == "Valid JSON" {
            app.editor = "{".into();
            app.dirty = true;
            app.syntax_error = None;
            app.clear_comparison();
            assert!(
                app.right_json_valid,
                "left edits must preserve the right validation result"
            );
        }
    }
}

#[test]
fn paths_stay_at_the_bottom_in_graph_compare_and_expanded_views() {
    let mut app = ready_app();
    app.pointer = format!("/{}", "long_path_segment/".repeat(30));
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    for height in [650.0, 800.0] {
        for (compare, wide, expected_paths) in
            [(false, false, 2), (true, false, 2), (false, true, 1)]
        {
            app.view = if compare { View::Compare } else { View::Graph };
            app.graph_wide = wide;
            app.compare_scroll = 400.0;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, height),
                    )),
                    ..Default::default()
                },
                |ctx| app.workspace(ctx),
            );
            let paths: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Path" => {
                        Some((shape.clip_rect, text))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(paths.len(), expected_paths);
            for (clip, text) in paths {
                assert!(text.pos.y >= height - 26.0);
                assert!(text.pos.y + text.galley.size().y <= height + 0.5);
                assert!(clip.top() >= height - 26.0);
            }
        }
    }
}

#[test]
fn clicking_an_object_key_updates_the_path_to_that_key() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    let frame = |app: &mut App, time, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        )
    };
    let output = frame(&mut app, 0.0, vec![]);
    let source = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == app.editor => Some(text),
            _ => None,
        })
        .expect("source editor must be painted");
    let key = app.editor.find("json_status").unwrap();
    let character = app.editor[..key + "json_statu".len()].chars().count();
    let key_start = source
        .galley
        .pos_from_cursor(egui::text::CCursor::new(character));
    let key_end = source
        .galley
        .pos_from_cursor(egui::text::CCursor::new(character + 1));
    let position = source.pos
        + egui::vec2(
            (key_start.left() + key_end.left()) * 0.5,
            key_start.center().y,
        );
    frame(
        &mut app,
        0.1,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        &mut app,
        0.2,
        vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    finish(
        &mut app,
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
    );
    assert_eq!(app.pointer, "/controls/json_status");
    let output = frame(&mut app, 0.3, vec![]);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "~ > controls > json_status")));
}

#[test]
fn clicking_a_readonly_large_preview_moves_its_caret_and_path() {
    let mut app = ready_app();
    app.editable = false;
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    let frame = |app: &mut App, time, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        )
    };
    let output = frame(&mut app, 0.0, vec![]);
    let source = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == app.editor => Some(text),
            _ => None,
        })
        .expect("source preview must be painted");
    let key = app.editor.find("json_status").unwrap();
    let character = app.editor[..key + "json_statu".len()].chars().count();
    let start = source
        .galley
        .pos_from_cursor(egui::text::CCursor::new(character));
    let end = source
        .galley
        .pos_from_cursor(egui::text::CCursor::new(character + 1));
    let position = source.pos + egui::vec2((start.left() + end.left()) * 0.5, start.center().y);
    frame(
        &mut app,
        0.1,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        &mut app,
        0.2,
        vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let offset = app.source_caret_offset.expect("preview caret must move");
    assert!(offset >= key && offset <= key + "json_status".len());
    finish(
        &mut app,
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
    );
    assert_eq!(app.pointer, "/controls/json_status");
}

#[test]
fn clicking_the_right_compare_key_updates_its_own_complete_path() {
    let mut app = ready_app();
    app.view = View::Compare;
    app.compare_text = "{\n  \"outer\": {\n    \"final_key\": 1\n  }\n}".into();
    app.right_json_valid = true;
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    let frame = |app: &mut App, time, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        )
    };
    let output = frame(&mut app, 0.0, vec![]);
    let source = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == app.compare_text => Some(text),
            _ => None,
        })
        .expect("right editor must be painted");
    let key = app.compare_text.find("final_key").unwrap();
    let character = app.compare_text[..key + "final_ke".len()].chars().count();
    let start = source
        .galley
        .pos_from_cursor(egui::text::CCursor::new(character));
    let end = source
        .galley
        .pos_from_cursor(egui::text::CCursor::new(character + 1));
    let position = source.pos + egui::vec2((start.left() + end.left()) * 0.5, start.center().y);
    frame(
        &mut app,
        0.1,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        &mut app,
        0.2,
        vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    finish(
        &mut app,
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
    );
    assert_eq!(app.right_pointer, "/outer/final_key");
    let output = frame(&mut app, 0.3, vec![]);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "~ > outer > final_key" && text.pos.x > 640.0)));
}

#[test]
fn mode_divider_aligns_with_editor_split_when_resized_or_dragged() {
    let mut app = ready_app();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    for width in [900.0, 1280.0] {
        for ratio in [0.35, 0.5, 0.65] {
            app.split_ratio = ratio;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 800.0),
                    )),
                    ..Default::default()
                },
                |ctx| app.workspace(ctx),
            );
            let vertical: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, .. }
                        if (points[0].x - points[1].x).abs() < 0.01 =>
                    {
                        Some(*points)
                    }
                    _ => None,
                })
                .collect();
            let splitter = vertical
                .iter()
                .find(|points| (points[1].y - points[0].y).abs() > 500.0)
                .expect("editor splitter must be rendered");
            assert!(
                vertical.iter().any(|points| {
                    points[0].y < 30.0
                        && ((points[1].y - points[0].y).abs() - 16.0).abs() < 0.1
                        && (points[0].x - splitter[0].x).abs() < 0.6
                }),
                "mode divider must align with editor at width={width}, ratio={ratio}"
            );
        }
    }
}

#[test]
fn invalid_navigation_path_does_not_hide_the_graph() {
    let mut app = ready_app();
    app.pointer = "/removed/path".into();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        },
        |ctx| app.workspace(ctx),
    );
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.ends_with(" root"))));
}

#[test]
fn blank_space_at_the_bottom_of_a_tall_editor_remains_editable() {
    let mut app = ready_app();
    app.view = View::Compare;
    app.editor = "{}".into();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    let position = egui::pos2(300.0, 1050.0);
    for (frame, events) in [
        vec![],
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::Text(" ".into())],
    ]
    .into_iter()
    .enumerate()
    {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 1200.0),
                )),
                time: Some(frame as f64 * 0.1),
                events,
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        );
    }
    assert_eq!(app.editor, "{} ");
    assert!(app.dirty);
}

#[test]
fn pending_and_equal_comparisons_start_both_editors_without_a_header_gap() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.view = View::Compare;
    app.compare_text = app.editor.clone();
    app.start(
        Job::Compare(app.full_text.clone(), Arc::new(app.compare_text.clone())),
        false,
    );
    finish(&mut app, receiver.recv().unwrap());
    assert!(app.comparison_done && app.differences.is_empty());

    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    for comparison_done in [false, true] {
        app.comparison_done = comparison_done;
        let mut output = None;
        for frame in 0..3 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 800.0),
                    )),
                    time: Some(frame as f64 * 0.1),
                    ..Default::default()
                },
                |ctx| app.workspace(ctx),
            ));
        }
        let output = output.unwrap();
        let editors: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == app.editor => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(editors.len(), 2);
        assert!(
            editors.iter().all(|editor| editor.pos.y < 45.0),
            "pending/equal editors should start below the 36px app header: {:?}",
            editors
                .iter()
                .map(|editor| editor.pos.y)
                .collect::<Vec<_>>()
        );
        assert!(!output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.job.text == "Updating comparison…"
                    || text.galley.job.text == "Both documents are equal")
        }));
    }
}

#[test]
fn compare_rows_and_gutters_remain_aligned_when_scrolling_the_right_editor() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    let values: Vec<_> = (0..100).collect();
    app.editor = serde_json::to_string_pretty(&values).unwrap();
    app.dirty = true;
    app.parse(Format::Keep);
    finish(&mut app, receiver.recv().unwrap());
    app.view = View::Compare;
    app.compare_text = app.editor.replacen("  1,\n", "  1,\n  1000,\n  1001,\n", 1);
    app.start(
        Job::Compare(app.full_text.clone(), Arc::new(app.compare_text.clone())),
        false,
    );
    finish(&mut app, receiver.recv().unwrap());
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    for frame in 0..6 {
        let events = if frame == 1 {
            vec![
                egui::Event::PointerMoved(egui::pos2(900.0, 500.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -120.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        } else {
            vec![]
        };
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                time: Some(frame as f64 * 0.1),
                events,
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        );
        let text_shapes: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    Some(text)
                } else {
                    None
                }
            })
            .collect();
        let left = text_shapes
            .iter()
            .find(|text| text.galley.job.text == app.editor)
            .unwrap();
        let left_clip = output.shapes.iter().find(|shape| {
            matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.job.text == app.editor)
        }).unwrap().clip_rect;
        assert!(left_clip.bottom() > 720.0);
        if frame == 0 {
            assert!(
                left_clip.top() < 200.0,
                "editor header reserves too much space"
            );
        }
        let right = text_shapes
            .iter()
            .find(|text| text.galley.job.text == app.compare_text)
            .unwrap();
        // The shared row after the two inserted lines has the same screen position.
        assert!(
            (left.pos.y + left.galley.rows[3].pos.y - right.pos.y - right.galley.rows[5].pos.y)
                .abs()
                < 0.5,
            "frame {frame}: left {:?} row {}; right {:?} row {}; shared scroll {}",
            left.pos,
            left.galley.rows[3].pos.y,
            right.pos,
            right.galley.rows[5].pos.y,
            app.compare_scroll
        );
        if frame == 0 {
            let summary = format!("Difference 1 / {}", app.differences.len());
            let headers: Vec<_> = text_shapes
                .iter()
                .filter(|text| text.galley.job.text == summary)
                .collect();
            assert_eq!(headers.len(), 2, "both compare panes need a diff header");
            assert!(headers.iter().any(|text| text.pos.x < 640.0));
            assert!(headers.iter().any(|text| text.pos.x >= 640.0));
            let numbers: Vec<_> = text_shapes
                .iter()
                .filter(|text| text.galley.job.text == "1")
                .collect();
            assert_eq!(numbers.len(), 2);
            assert!((numbers[0].pos.y - numbers[1].pos.y).abs() < 0.5);
        }
    }
    assert!(app.compare_scroll > 20.0);
}

#[test]
fn pasted_json_automatically_highlights_without_moving_the_editor_cursor() {
    let mut app = ready_app();
    let (sender, receiver) = mpsc::channel();
    app.jobs = sender.into();
    app.view = View::Compare;
    app.compare_text =
        app.editor
            .replacen("\"name\": \"Forma\"", "\"name\": \"Forma (comparison)\"", 1);
    app.clear_comparison();
    let due = app.compare_due.unwrap();
    app.tick_automatic_jobs(due - Duration::from_millis(1));
    assert!(receiver.try_recv().is_err());
    app.tick_automatic_jobs(due);
    let work = receiver.recv().unwrap();
    assert!(matches!(work.job, Job::Compare(..)));
    assert!(!work.user_action);
    finish(&mut app, work);
    assert!(app.comparison_done);
    assert_eq!(app.differences.len(), 1);
    assert_eq!(
        &app.compare_text[app.compare_right_range.clone().unwrap()],
        "\"Forma (comparison)\""
    );
    assert!(app.compare_left_range.is_some());
    assert!(!app.compare_jump && !app.search.jump);
    assert!(app.source_target.is_none());
}

#[test]
fn system_theme_is_default_and_manual_choices_override_system_changes() {
    let mut app = ready_app();
    assert_eq!(app.theme_preference, egui::ThemePreference::System);
    let ctx = egui::Context::default();
    for preference in [
        egui::ThemePreference::System,
        egui::ThemePreference::Light,
        egui::ThemePreference::Dark,
        egui::ThemePreference::System,
    ] {
        for theme in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
            let _ = ctx.run(
                egui::RawInput {
                    system_theme: Some(theme),
                    ..Default::default()
                },
                |ctx| {
                    app.set_theme_preference(ctx, preference);
                    app.sync_theme(ctx);
                    let expected = match preference {
                        egui::ThemePreference::System => theme,
                        egui::ThemePreference::Light => egui::Theme::Light,
                        egui::ThemePreference::Dark => egui::Theme::Dark,
                    };
                    assert_eq!(app.dark, expected == egui::Theme::Dark);
                    assert_eq!(ctx.theme(), expected);
                    assert_eq!(ctx.options(|options| options.theme_preference), preference);
                },
            );
        }
    }
}

#[test]
fn find_shortcut_toggles_only_the_active_pane_and_escape_closes_it() {
    let mut app = ready_app();
    assert!(!app.search.text.visible && !app.search.tree.visible && !app.search.right.visible);
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut key = |app: &mut App, key, modifiers| {
        time += 0.1;
        let _ = ctx.run(
            egui::RawInput {
                time: Some(time),
                modifiers,
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ctx| {
                app.search_shortcuts(ctx);
            },
        );
        time += 0.1;
        let _ = ctx.run(
            egui::RawInput {
                time: Some(time),
                modifiers,
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ctx| app.search_shortcuts(ctx),
        );
    };
    key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
    assert!(app.search.text.visible && app.search.text.focus);
    assert!(!app.search.tree.visible);
    app.search.text.query = "name".into();
    key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
    assert!(!app.search.text.visible && app.search.text.query.is_empty());
    app.active_right = true;
    key(&mut app, egui::Key::F, egui::Modifiers::COMMAND);
    assert!(app.search.tree.visible && app.search.tree.focus);
    key(&mut app, egui::Key::F, egui::Modifiers::COMMAND);
    assert!(!app.search.tree.visible);
    app.view = View::Compare;
    key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
    assert!(app.search.right.visible && app.search.right.focus);
    key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
    assert!(!app.search.right.visible);
    key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
    key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!app.search.right.visible);
}

#[test]
fn holding_find_shortcut_keeps_search_open() {
    let mut app = ready_app();
    let ctx = egui::Context::default();
    for (frame, repeat) in [false, true, true].into_iter().enumerate() {
        let _ = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 * 0.1),
                modifiers: egui::Modifiers::CTRL,
                events: vec![egui::Event::Key {
                    key: egui::Key::F,
                    physical_key: None,
                    pressed: true,
                    repeat,
                    modifiers: egui::Modifiers::CTRL,
                }],
                ..Default::default()
            },
            |ctx| app.search_shortcuts(ctx),
        );
        assert!(app.search.text.visible);
        assert!(!app.search.tree.visible && !app.search.right.visible);
    }
}

#[test]
fn two_find_presses_in_one_frame_toggle_twice() {
    let mut app = ready_app();
    let ctx = egui::Context::default();
    let event = |pressed, modifiers| egui::Event::Key {
        key: egui::Key::F,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    let _ = ctx.run(
        egui::RawInput {
            events: vec![
                event(true, egui::Modifiers::CTRL),
                event(false, egui::Modifiers::CTRL),
                event(true, egui::Modifiers::COMMAND),
            ],
            ..Default::default()
        },
        |ctx| app.search_shortcuts(ctx),
    );
    assert!(!app.search.text.visible);
}

#[test]
fn paths_collapse_only_when_the_full_text_exceeds_the_footer_width() {
    let mut app = ready_app();
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    for (pointer, width, expected) in [
        ("/one/two", 1280.0, "~ > one > two"),
        (
            "/one/two/three/four/five/six",
            1280.0,
            "~ > one > two > three > four > five > six",
        ),
        (
            "/alpha/bravo/charlie/delta/echo/foxtrot",
            900.0,
            "~ > alpha > ... > echo > foxtrot",
        ),
        (
            "/alpha/bravo/charlie/delta/echo/foxtrot",
            1600.0,
            "~ > alpha > bravo > charlie > delta > echo > foxtrot",
        ),
        (
            "/a~1b/very_long_middle_segment/other_long_middle_segment/c~0d/six",
            900.0,
            "~ > a/b > ... > c~d > six",
        ),
        (
            "/a/very_long_middle_segment_that_does_not_fit/b/c",
            900.0,
            "~ > a > ... > b > c",
        ),
    ] {
        app.pointer = pointer.into();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 800.0),
                )),
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        );
        assert!(
            output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == expected)),
            "width={width}, expected={expected}"
        );
    }
}

#[test]
fn large_comparison_scrollbar_drag_keeps_both_editors_visible_and_linked() {
    let mut app = ready_app();
    let text = format!(
        "[\n{}\n]",
        vec![r#"  {"id":123,"value":"中文 long wrapped value"}"#; 25_000].join(",\n")
    );
    app.editor = text.clone();
    app.full_text = Arc::new(text.clone());
    app.search_source = app.full_text.clone();
    app.compare_text = text.clone();
    app.compare_alignment = Some(Arc::new(forma::alignment::align(&text, &text)));
    app.comparison_done = true;
    app.editable = true;
    app.view = View::Compare;
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    let mut track = egui::Rect::ZERO;
    for frame in 0..7 {
        if frame == 1 {
            app.compare_scroll = 250_000.0;
        }
        let pos = egui::pos2(track.center().x, track.bottom() - 20.0);
        let events = match frame {
            2 => vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            3 => vec![egui::Event::PointerMoved(egui::pos2(
                pos.x,
                track.top() + 20.0,
            ))],
            4 => vec![egui::Event::PointerButton {
                pos: egui::pos2(pos.x, track.top() + 20.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            _ => vec![],
        };
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        );
        let editors: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text_shape) if text_shape.galley.job.text == text => {
                    Some((shape.clip_rect, text_shape))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            editors.len(),
            2,
            "both editors must paint after a far scrollbar jump"
        );
        for (clip, shape) in &editors {
            assert!(
                !shape.galley.rows.is_empty(),
                "the viewport must never be blank"
            );
            assert!(shape.galley.rows.len() < 40);
            assert!(
                shape
                    .galley
                    .mesh_bounds
                    .translate(shape.pos.to_vec2())
                    .intersects(*clip)
            );
        }
        assert!(
            (editors[0].1.pos.y - editors[1].1.pos.y).abs() < 1.0,
            "frame={frame} left={} right={} offset={}",
            editors[0].1.pos.y,
            editors[1].1.pos.y,
            app.compare_scroll
        );
        track = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.width() < 20.0
                        && rect.rect.height() > 100.0
                        && rect.rect.center().x < 640.0 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("left scrollbar track must be visible");
        if frame == 2 {
            assert!(
                app.compare_scroll > 400_000.0,
                "track click must move the linked offset"
            );
        }
        if frame >= 4 {
            assert!(
                app.compare_scroll < 50_000.0,
                "drag must move both panes back towards the top"
            );
        }
        let _ = ctx.tessellate(output.shapes, output.pixels_per_point);
    }
}

#[test]
fn continuous_scroll_in_either_compare_pane_uses_one_pass_and_keeps_rows_aligned() {
    let mut app = ready_app();
    let text = format!(
        "[\n{}\n]",
        vec![r#"  {"id":123,"value":"scrolling text"}"#; 20_000].join(",\n")
    );
    app.editor = text.clone();
    app.full_text = Arc::new(text.clone());
    app.search_source = app.full_text.clone();
    app.compare_text = text.clone();
    app.compare_alignment = Some(Arc::new(forma::alignment::align(&text, &text)));
    app.comparison_done = true;
    app.editable = true;
    app.view = View::Compare;
    let ctx = egui::Context::default();
    ui::install_fonts(&ctx);
    ui::configure(&ctx, false);
    for frame in 0..42 {
        let events = if frame < 2 {
            vec![]
        } else {
            vec![
                egui::Event::PointerMoved(egui::pos2(
                    if frame < 22 { 300.0 } else { 900.0 },
                    400.0,
                )),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -120.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            },
            |ctx| app.workspace(ctx),
        );
        if frame >= 2 {
            assert_eq!(
                output.platform_output.num_completed_passes, 1,
                "scrolling frame {frame} must not discard the whole UI"
            );
        }
        let editors: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(shape) if shape.galley.job.text == text => Some(shape),
                _ => None,
            })
            .collect();
        assert_eq!(editors.len(), 2);
        assert_eq!(
            editors[0].pos.y, editors[1].pos.y,
            "linked paint must align in frame {frame}"
        );
        assert_eq!(editors[0].galley.rows, editors[1].galley.rows);
        assert!(output.shapes.iter().all(|shape| !matches!(&shape.shape,
            egui::Shape::Text(shape) if shape.galley.job.text.contains("egui PERF WARNING"))));
        let _ = ctx.tessellate(output.shapes, output.pixels_per_point);
    }
    assert!(app.compare_scroll > 2000.0);
}
