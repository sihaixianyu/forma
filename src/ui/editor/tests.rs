use super::*;

mod tests {

    use super::*;

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
        let message = json_lens::parse_json(&text).unwrap_err();
        let diagnostic = crate::jobs::SyntaxError::new(&message, &text);
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
        assert!(marker.width() >= EDITOR_GUTTER_WIDTH - 1.0);
        assert!(marker.width() < 50.0);
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
