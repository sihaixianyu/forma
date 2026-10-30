use super::*;

#[test]
fn wrapped_comparison_keeps_following_lines_aligned_and_cursor_offsets_intact() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    configure(&ctx, false);
    let left = format!(
        "{{\n  \"value\": \"{}\",\n  \"last\": true\n}}",
        "中文 long value ".repeat(12)
    );
    let right = "{\n  \"value\": \"short\",\n  \"last\": true\n}";
    let alignment = json_lens::alignment::align(&left, right);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let a = CodeCache::default().layout_width(ui, &left, false, "", 180.0);
            let b = CodeCache::default().layout_width(ui, right, false, "", 180.0);
            assert!(a.rows.len() > left.lines().count());
            let a = aligned_wrapped_galley(ui, a, Some(&alignment), true, right, 180.0);
            let b = aligned_wrapped_galley(ui, b, Some(&alignment), false, &left, 180.0);
            let a_index = left[..left.find("\"last\"").unwrap()].chars().count();
            let b_index = right[..right.find("\"last\"").unwrap()].chars().count();
            let a_cursor = a.pos_from_cursor(egui::text::CCursor::new(a_index));
            let b_cursor = b.pos_from_cursor(egui::text::CCursor::new(b_index));
            assert_eq!(a_cursor.top(), b_cursor.top());
            assert_eq!(a.rect.height(), b.rect.height());
            assert_eq!(
                a.cursor_from_pos(a_cursor.center().to_vec2()).index,
                a_index
            );
            assert_eq!(a.job.text, left);
            assert_eq!(a.rows.iter().filter(|row| row.ends_with_newline).count(), 3);
        });
    });
}

#[test]
fn equal_split_has_mirrored_pane_dimensions() {
    for size in [Vec2::new(1240.0, 660.0), Vec2::new(760.0, 460.0)] {
        let area = egui::Rect::from_min_size(egui::pos2(20.0, 100.0), size);
        let (left, gap, right) = pane_rects(area, 0.5);
        assert_eq!(left.size(), right.size());
        assert_eq!(left.top(), right.top());
        assert_eq!(left.bottom(), right.bottom());
        assert_eq!(gap.center().x, area.center().x);
        assert_eq!(left.right(), gap.left());
        assert_eq!(right.left(), gap.right());
    }
}

fn render(ctx: &egui::Context, time: f64, events: Vec<egui::Event>) -> (egui::Rect, Color32, bool) {
    let mut rect = egui::Rect::NOTHING;
    let mut clicked = false;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(400.0, 200.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = primary(ui, "Format", palette(false));
                rect = response.rect;
                clicked = response.clicked();
            });
        },
    );
    let fill = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(shape)
                if shape.rect.center().distance(rect.center()) < 0.1
                    && shape.fill != Color32::TRANSPARENT =>
            {
                Some(shape.fill)
            }
            _ => None,
        })
        .expect("button background");
    (rect, fill, clicked)
}

#[test]
fn comparison_preserves_normal_line_spacing_at_each_pixel_scale() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        configure(&ctx, false);
        let source = "{\n  \"name\": \"中文 🚀\",\n  \"value\": 123,\n  \"ready\": true\n}";
        let alignment = json_lens::alignment::align(source, source);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.set_pixels_per_point(scale);
            egui::CentralPanel::default().show(ctx, |ui| {
                let normal = CodeCache::default().layout(ui, source, false, "");
                let aligned = aligned_galley(ui, normal.clone(), Some(&alignment), true);
                for (before, after) in normal.rows.iter().zip(&aligned.rows) {
                    assert_eq!(
                        before.pos, after.pos,
                        "line spacing changed at scale {scale}"
                    );
                    assert_eq!(before.size, after.size);
                    assert!(Arc::ptr_eq(&before.row, &after.row));
                }
                assert_eq!(normal.rect, aligned.rect);
                let right = source.replace(
                    "  \"ready\": true",
                    "  \"extra\": {\n    \"nested\": true\n  },\n  \"ready\": true",
                );
                let gaps = json_lens::alignment::align(source, &right);
                let aligned = aligned_galley(ui, normal.clone(), Some(&gaps), true);
                let line_height = normal.rows[0].size.y;
                for (index, (before, after)) in normal.rows.iter().zip(&aligned.rows).enumerate() {
                    assert_eq!(before.size, after.size);
                    assert!(Arc::ptr_eq(&before.row, &after.row));
                    assert!(
                        (after.pos.y - gaps.left.rows[index] as f32 * line_height).abs()
                            <= 0.5 / aligned.pixels_per_point
                    );
                }
            });
        });
    }
}

#[test]
fn aligned_text_preserves_cursor_offsets_and_corresponding_row_heights() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let left = "{\n\"a\": 1,\n\"last\": \"中文\"\n}";
    let right = "{\n\"a\": 1,\n\"new\": [\n  1\n],\n\"last\": \"中文\"\n}";
    let alignment = json_lens::alignment::align(left, right);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let a = CodeCache::default().layout(ui, left, false, "");
            let b = CodeCache::default().layout(ui, right, false, "");
            let a = aligned_galley(ui, a, Some(&alignment), true);
            let b = aligned_galley(ui, b, Some(&alignment), false);
            assert_eq!(a.rows[2].pos.y, b.rows[5].pos.y);
            assert_eq!(a.rect.height(), b.rect.height());
            assert_eq!(a.job.text, left);
            assert_eq!(b.job.text, right);
            let index = left[..left.find("中文").unwrap()].chars().count();
            let cursor = egui::text::CCursor::new(index);
            let position = a.pos_from_cursor(cursor);
            assert_eq!(a.cursor_from_pos(position.center().to_vec2()).index, index);
            let edited = format!("{left}\n");
            let normal = CodeCache::default().layout(ui, &edited, false, "");
            let unchanged = aligned_galley(ui, normal.clone(), Some(&alignment), true);
            assert!(Arc::ptr_eq(&normal, &unchanged));
        });
    });
}

#[test]
fn editor_gutter_stays_outside_horizontal_scrolling() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let mut text = format!("{}\n", "long line ".repeat(100)).repeat(100);
    let mut cache = CodeCache::default();
    let mut previous = None;
    for frame in 0..2 {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(500.0, 400.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let scroll = editor_scroll(
                        ui,
                        EditorViewport {
                            salt: "gutter_test",
                            wrap: false,
                            height: 200.0,
                            offset: Some(0.0),
                            palette: palette(false),
                            left: false,
                            first_line: 1,
                            base: 0,
                            alignment: None,
                            syntax_error: None,
                        },
                        |ui| {
                            let mut layouter =
                                |ui: &egui::Ui, text: &dyn egui::TextBuffer, _: f32| {
                                    cache.layout(ui, text.as_str(), false, "")
                                };
                            egui::TextEdit::multiline(&mut text)
                                .code_editor()
                                .font(egui::TextStyle::Monospace)
                                .frame(false)
                                .margin(Vec2::ZERO)
                                .desired_width(ui.available_width())
                                .layouter(&mut layouter)
                                .show(ui)
                        },
                    );
                    if frame == 0 {
                        previous = Some((scroll.inner_rect.left(), scroll.inner.galley_pos.x));
                        let mut state = scroll.state;
                        state.offset.x = 100.0;
                        state.store(ctx, scroll.id);
                    } else {
                        let (viewport_x, text_x) = previous.unwrap();
                        assert_eq!(viewport_x, scroll.inner_rect.left());
                        assert!(scroll.inner.galley_pos.x < text_x - 50.0);
                        assert!(
                            scroll.inner_rect.left()
                                > ui.max_rect().left() + EDITOR_GUTTER_WIDTH + EDITOR_GUTTER_GAP
                                    - 1.0
                        );
                    }
                });
            },
        );
    }
}

#[test]
fn primary_button_has_distinct_hover_press_and_release_feedback() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let (rect, idle, _) = render(&ctx, 0.0, vec![]);
    let center = rect.center();
    let (_, hovered, _) = render(&ctx, 0.1, vec![egui::Event::PointerMoved(center)]);
    let event = |pressed| egui::Event::PointerButton {
        pos: center,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let (_, pressed, _) = render(&ctx, 0.2, vec![event(true)]);
    assert_ne!(idle, hovered);
    assert_ne!(hovered, pressed);
    let (_, _, clicked) = render(&ctx, 0.3, vec![event(false)]);
    assert!(clicked);
}

#[test]
fn code_editor_tab_undo_redo_and_caret_use_the_widget_input_path() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let mut text = "abc".to_owned();
    let keys = [
        None,
        None, // Register the editor's focus-lock before sending Tab.
        Some((egui::Key::Tab, egui::Modifiers::NONE)),
        Some((egui::Key::Z, egui::Modifiers::CTRL)),
        Some((egui::Key::Z, egui::Modifiers::CTRL | egui::Modifiers::SHIFT)),
        Some((egui::Key::Z, egui::Modifiers::CTRL)),
        Some((egui::Key::Y, egui::Modifiers::CTRL)),
    ];
    for (frame, key) in keys.into_iter().enumerate() {
        let output = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 * 0.1),
                events: key
                    .map(|(key, modifiers)| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let changed = prepare_code_input(ui, "tab_test", &mut text);
                    let output = show_code_editor(
                        ui,
                        "tab_test",
                        changed,
                        egui::TextEdit::multiline(&mut text)
                            .id_salt("tab_test")
                            .font(egui::TextStyle::Monospace)
                            .code_editor(),
                    );
                    if frame == 0 {
                        output.response.request_focus();
                    }
                });
            },
        );
        assert_eq!(
            text,
            if frame >= 2 && frame % 2 == 0 {
                "abc  "
            } else {
                "abc"
            },
            "frame={frame}"
        );
        if frame > 0 {
            assert!(output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::LineSegment { points, stroke }
                if stroke.color == palette(false).text
                        && (stroke.width - 2.0).abs() < 0.01
                        && (points[0].x - points[1].x).abs() < 0.01
                        && ((points[1].y - points[0].y).abs()
                            - (EDITOR_FONT_SIZE + 2.0))
                            .abs()
                            < 0.01
            )));
        }
    }
}

#[test]
fn selected_unicode_lines_indent_by_two_and_outdent_is_undoable() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let mut text = "  中\n  文".to_owned();
    let keys = [
        None,
        Some((egui::Key::A, egui::Modifiers::COMMAND)),
        Some((egui::Key::Tab, egui::Modifiers::SHIFT)),
        Some((egui::Key::Z, egui::Modifiers::CTRL)),
        Some((egui::Key::Y, egui::Modifiers::CTRL)),
        Some((egui::Key::Tab, egui::Modifiers::NONE)),
    ];
    for (frame, key) in keys.into_iter().enumerate() {
        let _ = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 * 0.1),
                events: key
                    .map(|(key, modifiers)| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let changed = prepare_code_input(ui, "block_test", &mut text);
                    let output = show_code_editor(
                        ui,
                        "block_test",
                        changed,
                        egui::TextEdit::multiline(&mut text)
                            .id_salt("block_test")
                            .code_editor(),
                    );
                    if frame == 0 {
                        output.response.request_focus();
                    }
                });
            },
        );
        assert_eq!(
            text,
            if frame == 2 || frame == 4 {
                "中\n文"
            } else {
                "  中\n  文"
            }
        );
    }
}

#[test]
fn code_layout_preserves_font_row_metrics_for_cursor_positions() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let mut cache = CodeCache::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            for text in ["{\n  \"name\": 1\n}", "", "name\n"] {
                let galley = cache.layout(ui, text, false, "");
                let font_height =
                    ui.fonts_mut(|fonts| fonts.row_height(&FontId::monospace(EDITOR_FONT_SIZE)));
                let mut offset = 0;
                for row in &galley.rows {
                    let cursor = egui::text_selection::text_cursor_state::cursor_rect(
                        &galley,
                        &egui::text::CCursor::new(offset),
                        font_height,
                    );
                    assert!(
                        // egui expands the font-aligned caret by 1.5 points at each end.
                        (cursor.height() - font_height - 3.0).abs() < 1.0,
                        "cursor={} font={}",
                        cursor.height(),
                        font_height
                    );
                    offset += row.glyphs.len() + usize::from(row.ends_with_newline);
                }
            }
        });
    });
}

#[test]
fn wrapped_compare_stays_aligned_after_cached_frames_width_and_theme_changes() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let left = "{\n  \"label\": \"中文 hello hello hello hello hello hello\",\n  \"value\": 42\n}";
    let right = left.replace("  \"value\"", "  \"inserted\": true,\n  \"value\"");
    let alignment = Arc::new(json_lens::alignment::align(left, &right));
    let mut left_cache = CodeCache::default();
    let mut right_cache = CodeCache::default();
    for (dark, width) in [
        (false, 200.0),
        (false, 200.0),
        (true, 90.0),
        (true, 90.0),
        (false, 160.0),
    ] {
        configure(&ctx, dark);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let a = left_cache.layout_width(ui, left, dark, "中文", width);
                let a = left_cache.align(ui, a, Some(&alignment), true, &right, width);
                let b = right_cache.layout_width(ui, &right, dark, "中文", width);
                let b = right_cache.align(ui, b, Some(&alignment), false, left, width);
                assert_eq!(a.job.text, left);
                assert_eq!(b.job.text, right);
                let line_offsets = |text: &str| {
                    let mut offset = 0;
                    text.split_inclusive('\n')
                        .map(|line| {
                            let start = offset;
                            offset += line.chars().count();
                            start
                        })
                        .collect::<Vec<_>>()
                };
                let a_offsets = line_offsets(left);
                let b_offsets = line_offsets(&right);
                for (line, display) in alignment.left.rows.iter().enumerate() {
                    if let Ok(other) = alignment.right.rows.binary_search(display) {
                        let a_top = a
                            .pos_from_cursor(egui::text::CCursor::new(a_offsets[line]))
                            .top();
                        let b_top = b
                            .pos_from_cursor(egui::text::CCursor::new(b_offsets[other]))
                            .top();
                        assert!((a_top - b_top).abs() < 0.01);
                    }
                }
            });
        });
    }
}
