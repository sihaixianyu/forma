use super::*;

#[test]
fn long_paste_preserves_viewport_but_typing_still_follows_the_caret() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let id = egui::Id::new("paste_scroll_test");
    let pasted = format!("[\n{}\n]", vec!["  {\"value\": \"中文\"}"; 500].join(",\n"));
    let mut text = pasted.clone();
    for frame in 0..30 {
        let events = match frame {
            1 => {
                ctx.memory_mut(|memory| memory.request_focus(id));
                let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(text.chars().count()),
                    )));
                state.store(&ctx, id);
                vec![egui::Event::Paste(pasted.clone())]
            }
            15 => {
                let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(pasted.chars().count()),
                    )));
                state.store(&ctx, id);
                vec![egui::Event::Text(" ".into())]
            }
            _ => vec![],
        };
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(600.0, 300.0),
                )),
                time: Some(frame as f64 * 0.1),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut area = egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .animated(frame != 1);
                    if frame == 0 {
                        area = area.vertical_scroll_offset(400.0);
                    }
                    let scroll = area.show(ui, |ui| {
                        show_code_editor(
                            ui,
                            "paste_scroll_test",
                            false,
                            EDITOR_LINE_HEIGHT,
                            egui::TextEdit::multiline(&mut text)
                                .id(id)
                                .code_editor()
                                .desired_rows(1),
                        )
                    });
                    if (1..15).contains(&frame) {
                        assert_eq!(
                            scroll.state.offset.y, 0.0,
                            "paste must keep the view at the top"
                        );
                        assert_eq!(
                            scroll
                                .inner
                                .state
                                .cursor
                                .char_range()
                                .unwrap()
                                .primary
                                .index,
                            0
                        );
                    }
                    if frame == 29 {
                        assert!(
                            scroll.state.offset.y > 1000.0,
                            "typing should reveal the caret"
                        );
                    }
                });
            },
        );
    }
    assert_eq!(text, format!("{pasted} "));
}

mod history_tests {

    use super::*;

    #[test]
    fn layout_identity_keeps_caret_redo_and_external_replacements_correct() {
        let mut history = CodeHistory::default();
        let job = |text: &str| {
            Arc::new(egui::text::LayoutJob {
                text: text.into(),
                ..Default::default()
            })
        };
        let cursor = |index| egui::text::CCursorRange::one(egui::text::CCursor::new(index));
        let first = job("first");
        let second = job("second");
        history.feed_job(&first, cursor(0), 0.0, true);
        history.feed_job(&second, cursor(0), 1.0, true);
        history.feed_job(&second, cursor(3), 2.0, false);
        assert_eq!(history.current.as_ref().unwrap().cursor, cursor(3));
        assert_eq!(history.past.len(), 1);
        assert_eq!(&**history.undo(false).unwrap().text, "first");
        history.feed_job(&first, cursor(1), 3.0, false);
        assert_eq!(&**history.undo(true).unwrap().text, "second");
        history.feed("external", cursor(0), 4.0, true);
        history.feed_job(&second, cursor(0), 5.0, true);
        assert_eq!(&**history.current.as_ref().unwrap().text, "second");
    }

    #[test]
    fn history_byte_budget_remains_correct_after_trimming_undo_and_redo() {
        let mut history = CodeHistory::default();
        let cursor = egui::text::CCursorRange::one(egui::text::CCursor::new(0));
        for i in 0..120 {
            history.feed(
                &format!("{i}:{}", "a".repeat(400_000)),
                cursor,
                i as f64,
                true,
            );
        }
        assert!(history.past_bytes <= 32 * 1024 * 1024);
        assert_eq!(
            history.past_bytes,
            history.past.iter().map(|s| s.text.len()).sum::<usize>()
        );
        for redo in [false, false, true, true] {
            history.undo(redo).unwrap();
            assert_eq!(
                history.past_bytes,
                history.past.iter().map(|s| s.text.len()).sum::<usize>()
            );
        }
    }
}

mod viewport_tests {

    use super::*;

    #[test]
    fn clicking_a_line_number_highlights_wrapped_rows_and_accents_the_number() {
        for left in [true, false] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            configure(&ctx, false);
            let mut text = format!("{{\n  \"value\": \"{}\"\n}}", "中文 long value ".repeat(20));
            let mut cache = CodeCache::default();
            let render = |ctx: &egui::Context, text: &mut String, cache: &mut CodeCache| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    editor_scroll(
                        ui,
                        EditorViewport {
                            salt: "active_line_test",
                            wrap: true,
                            height: 280.0,
                            offset: None,
                            palette: palette(false),
                            left,
                            first_line: 1,
                            base: 0,
                            alignment: None,
                            syntax_error: None,
                            font_size: EDITOR_FONT_SIZE,
                        },
                        |ui| {
                            let mut layouter =
                                |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
                                    cache.layout_width(ui, text.as_str(), false, "", width)
                                };
                            show_code_editor(
                                ui,
                                "active_line_test",
                                false,
                                EDITOR_LINE_HEIGHT,
                                egui::TextEdit::multiline(text)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .frame(false)
                                    .desired_width(ui.available_width())
                                    .layouter(&mut layouter),
                            )
                        },
                    );
                });
            };
            let input = |events| egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(400.0, 320.0),
                )),
                events,
                ..Default::default()
            };
            let initial = ctx.run(input(vec![]), |ctx| render(ctx, &mut text, &mut cache));
            let pos = initial
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "2" => {
                        Some(egui::pos2(25.0, text.pos.y + text.galley.size().y * 0.5))
                    }
                    _ => None,
                })
                .expect("second line number must be visible");
            for pressed in [true, false] {
                let _ = ctx.run(
                    input(vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]),
                    |ctx| render(ctx, &mut text, &mut cache),
                );
            }
            let selected = ctx.run(input(vec![]), |ctx| render(ctx, &mut text, &mut cache));
            let p = palette(false);
            assert!(selected.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.job.text == "2"
                    && text.galley.job.sections[0].format.color == p.accent)
            }));
            fn highlighted_rows(shape: &egui::Shape, color: Color32) -> usize {
                match shape {
                    egui::Shape::Vec(shapes) => shapes
                        .iter()
                        .map(|shape| highlighted_rows(shape, color))
                        .sum(),
                    egui::Shape::Rect(rect) if rect.fill == color && rect.rect.width() > 200.0 => 1,
                    _ => 0,
                }
            }
            assert!(
                selected
                    .shapes
                    .iter()
                    .map(|shape| highlighted_rows(&shape.shape, p.tint))
                    .sum::<usize>()
                    > 1,
                "the active logical line must highlight all its wrapped rows"
            );
        }
    }

    #[test]
    fn gutter_marks_parser_error_line_and_click_moves_the_editor_cursor() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        configure(&ctx, false);
        let mut text = "{\n  \"中文\": }".to_owned();
        let message = forma::parse_json(&text).unwrap_err();
        let diagnostic = crate::app::jobs::SyntaxError::new(&message, &text);
        let mut cache = CodeCache::default();
        let mut editor_id = egui::Id::NULL;
        let render = |ctx: &egui::Context,
                      text: &mut String,
                      cache: &mut CodeCache,
                      editor_id: &mut egui::Id| {
            egui::CentralPanel::default().show(ctx, |ui| {
                editor_scroll(
                    ui,
                    EditorViewport {
                        salt: "error_test",
                        wrap: true,
                        height: 200.0,
                        offset: None,
                        palette: palette(false),
                        left: true,
                        first_line: 1,
                        base: 0,
                        alignment: None,
                        syntax_error: Some(&diagnostic),
                        font_size: EDITOR_FONT_SIZE,
                    },
                    |ui| {
                        let mut layouter =
                            |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
                                cache.layout_width(ui, text.as_str(), false, "", width)
                            };
                        let output = show_code_editor(
                            ui,
                            "error_test",
                            false,
                            EDITOR_LINE_HEIGHT,
                            egui::TextEdit::multiline(text)
                                .code_editor()
                                .desired_rows(8)
                                .layouter(&mut layouter),
                        );
                        *editor_id = output.response.id;
                        output
                    },
                );
            });
        };
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            render(ctx, &mut text, &mut cache, &mut editor_id)
        });
        let red = palette(false).red.gamma_multiply(0.16);
        let marker = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.fill == red => Some(rect.rect),
                _ => None,
            })
            .expect("invalid line must have a red gutter marker");
        assert!(marker.height() > 0.0);
        assert!((marker.width() - (EDITOR_GUTTER_WIDTH - 16.0)).abs() < 1.0);
        let pos = marker.center();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| render(ctx, &mut text, &mut cache, &mut editor_id),
        );
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| render(ctx, &mut text, &mut cache, &mut editor_id),
        );
        let state = egui::text_edit::TextEditState::load(&ctx, editor_id).unwrap();
        assert_eq!(
            state.cursor.char_range().unwrap().primary.index,
            text[..diagnostic.offset.unwrap()].chars().count()
        );
        assert!(ctx.memory(|memory| memory.has_focus(editor_id)));
    }
}

// Exercise real TextEdit, ScrollArea and tessellation without a window.
mod large_document_ui_tests {
    use super::*;

    #[test]
    fn far_scroll_and_scrollbar_drag_keep_text_visible_with_bounded_paint_meshes() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        configure(&ctx, false);
        let mut text = "  {\"name\":\"中文🙂 wrapped value\",\"id\":123},\n".repeat(60_000);
        let mut cache = CodeCache::default();
        let mut offset = 0.0;
        let mut extent = 0.0;
        let mut inner = egui::Rect::ZERO;
        let mut last_shape_y = 0.0;
        let mut track = egui::Rect::ZERO;
        for frame in 0..8 {
            let pos = egui::pos2(track.center().x, track.bottom() - 18.0);
            let events = match frame {
                3 => vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                4 => vec![egui::Event::PointerMoved(egui::pos2(
                    pos.x,
                    track.top() + 18.0,
                ))],
                5 => vec![egui::Event::PointerButton {
                    pos: egui::pos2(pos.x, track.top() + 18.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                _ => vec![],
            };
            if frame == 1 {
                offset = extent * 0.5;
            }
            if frame == 2 {
                offset = extent - 240.0;
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(400.0, 300.0),
                    )),
                    time: Some(frame as f64 / 60.0),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let scroll = editor_scroll(
                            ui,
                            EditorViewport {
                                salt: "large_scroll",
                                wrap: true,
                                height: 240.0,
                                offset: Some(offset),
                                palette: palette(false),
                                left: true,
                                first_line: 1,
                                base: 0,
                                alignment: None,
                                syntax_error: None,
                                font_size: EDITOR_FONT_SIZE,
                            },
                            |ui| {
                                let mut layouter =
                                    |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
                                        cache.layout_width(ui, text.as_str(), false, "", width)
                                    };
                                show_code_editor(
                                    ui,
                                    "large_scroll",
                                    false,
                                    EDITOR_LINE_HEIGHT,
                                    egui::TextEdit::multiline(&mut text)
                                        .code_editor()
                                        .frame(false)
                                        .margin(Vec2::ZERO)
                                        .desired_width(ui.available_width())
                                        .layouter(&mut layouter),
                                )
                            },
                        );
                        assert!(scroll.inner.galley.rows.len() >= 60_000);
                        assert_eq!(scroll.inner.galley.job.text, text);
                        extent = scroll.content_size.y;
                        inner = scroll.inner_rect;
                        offset = scroll.state.offset.y;
                    });
                },
            );
            let painted = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(shape) if shape.galley.job.text == text => Some(shape),
                    _ => None,
                })
                .expect("document text must be painted even after a far scroll");
            assert!(!painted.galley.rows.is_empty());
            assert!(
                painted.galley.rows.len() < 20,
                "only viewport rows may reach tessellation"
            );
            assert!(painted.galley.num_vertices < 4_000);
            assert!(
                painted
                    .galley
                    .mesh_bounds
                    .translate(painted.pos.to_vec2())
                    .intersects(inner)
            );
            last_shape_y = painted.pos.y;
            track = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.width() < 20.0 && rect.rect.height() > 100.0 =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .expect("scrollbar track must be visible");
            if frame >= 5 {
                assert!(
                    offset < extent * 0.1,
                    "drag must move the scrollbar towards the top"
                );
            }
            let meshes = ctx.tessellate(output.shapes, output.pixels_per_point);
            assert!(meshes.iter().any(|primitive| matches!(&primitive.primitive,
                egui::epaint::Primitive::Mesh(mesh) if mesh.vertices.len() > 100)));
        }
        assert!(
            last_shape_y.abs() < 300.0,
            "paint coordinates must remain near the viewport"
        );
    }
}

mod hover_ui_tests {
    use super::*;

    #[test]
    fn hovering_a_wrapped_encoded_string_far_into_unicode_source_keeps_the_preview() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        configure(&ctx, false);
        let mut text = "  {\"prefix\":\"中文🙂\",\"value\":\"5Lit5paH\"},\n".repeat(5_000);
        let character = text[..text.rfind("5Lit5paH").unwrap()].chars().count();
        let mut cache = CodeCache::default();
        let mut offset = 0.0;
        let mut pointer = egui::Pos2::ZERO;
        let mut id = egui::Id::NULL;
        for frame in 0..3 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(280.0, 300.0),
                    )),
                    events: if frame == 2 {
                        vec![egui::Event::PointerMoved(pointer)]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        editor_scroll(
                            ui,
                            EditorViewport {
                                salt: "far_hover",
                                wrap: true,
                                height: 240.0,
                                offset: Some(offset),
                                palette: palette(false),
                                left: true,
                                first_line: 1,
                                base: 0,
                                alignment: None,
                                syntax_error: None,
                                font_size: EDITOR_FONT_SIZE,
                            },
                            |ui| {
                                let mut layouter =
                                    |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
                                        cache.layout_width(ui, text.as_str(), false, "", width)
                                    };
                                let output = show_code_editor(
                                    ui,
                                    "far_hover",
                                    false,
                                    EDITOR_LINE_HEIGHT,
                                    egui::TextEdit::multiline(&mut text)
                                        .code_editor()
                                        .frame(false)
                                        .margin(Vec2::ZERO)
                                        .desired_width(ui.available_width())
                                        .layouter(&mut layouter),
                                );
                                let rect = output
                                    .galley
                                    .pos_from_cursor(egui::text::CCursor::new(character));
                                if frame == 0 {
                                    offset = rect.top() - 100.0;
                                }
                                pointer = output.galley_pos
                                    + egui::vec2(rect.min.x + 3.0, rect.center().y);
                                id = output.response.id;
                                decoded_hover(ui, &output, &text);
                                output
                            },
                        );
                    });
                },
            );
        }
        assert_eq!(
            ctx.data(|data| data.get_temp::<String>(id.with("decoded_preview")))
                .as_deref(),
            Some("5Lit5paH")
        );
    }
}

#[test]
fn indexed_caret_offsets_match_unicode_text_across_wrapped_rows() {
    let ctx = egui::Context::default();
    let text = "{\"value\":\"中文 café 👩‍💻\"}\n".repeat(200);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let galley = ui.fonts_mut(|fonts| {
                fonts.layout(text.clone(), FontId::monospace(12.0), Color32::WHITE, 90.0)
            });
            let rows = code_rows(ctx, egui::Id::new("offset_test"), &galley);
            for (index, (byte, _)) in text.char_indices().enumerate() {
                for prefer_next_row in [false, true] {
                    assert_eq!(
                        rows.byte_offset(
                            &galley,
                            &text,
                            egui::text::CCursor {
                                index,
                                prefer_next_row,
                            }
                        ),
                        byte
                    );
                }
            }
            assert_eq!(
                rows.byte_offset(
                    &galley,
                    &text,
                    egui::text::CCursor::new(text.chars().count())
                ),
                text.len()
            );
        });
    });
}

#[test]
fn code_editor_skips_duplicate_internal_undo_snapshots() {
    let ctx = egui::Context::default();
    let mut text = "{\"value\":\"work\"}".to_owned();
    let id = egui::Id::new("external_history_test");
    for frame in 0..4 {
        let _ = ctx.run(
            egui::RawInput {
                time: Some(frame as f64),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if frame > 0 {
                        ctx.memory_mut(|memory| memory.request_focus(id));
                    }
                    let output = show_code_editor(
                        ui,
                        "external_history_test",
                        false,
                        18.0,
                        egui::TextEdit::multiline(&mut text).id(id).code_editor(),
                    );
                    assert!(output.state.external_undo);
                    assert!(!output.state.undoer().has_undo(&(
                        output.state.cursor.char_range().unwrap_or_default(),
                        text.clone()
                    )));
                });
            },
        );
    }
}

#[test]
#[ignore = "manual caret lookup benchmark"]
fn large_document_caret_lookup_timings() {
    let text = "{\"name\":\"中文 café work\"}\n".repeat(35_000);
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let galley = ui.fonts_mut(|fonts| {
                fonts.layout_no_wrap(text.clone(), FontId::monospace(12.0), Color32::WHITE)
            });
            let rows = code_rows(ctx, egui::Id::new("lookup_benchmark"), &galley);
            let cursor = egui::text::CCursor::new(text.chars().count() * 4 / 5);
            let expected = text.char_indices().nth(cursor.index).unwrap().0;
            assert_eq!(rows.byte_offset(&galley, &text, cursor), expected);
            for indexed in [false, true] {
                let started = std::time::Instant::now();
                for _ in 0..100 {
                    let offset = if indexed {
                        rows.byte_offset(&galley, std::hint::black_box(&text), cursor)
                    } else {
                        std::hint::black_box(&text)
                            .char_indices()
                            .nth(cursor.index)
                            .unwrap()
                            .0
                    };
                    std::hint::black_box(offset);
                }
                eprintln!(
                    "caret lookup indexed={indexed} bytes={} ms/lookup={:.4}",
                    text.len(),
                    started.elapsed().as_secs_f64() * 10.0
                );
            }
        });
    });
}
