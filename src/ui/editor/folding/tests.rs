use super::*;

fn collapse(ctx: &egui::Context, id: egui::Id, line: usize) {
    let cache = ctx
        .data(|data| data.get_temp::<FoldLayout>(id.with("fold_layout")))
        .unwrap();
    let block = &cache.blocks[&line];
    ctx.data_mut(|data| {
        let mut state = data.get_temp::<FoldState>(cache.key).unwrap();
        state.collapsed.insert(block.start, block.end);
        data.insert_temp(cache.key, state);
    });
}

#[test]
fn nested_and_sibling_folds_keep_unicode_source_and_cursor_offsets() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("fold_test");
    let text = "{\n  \"a\": [\n    {\n      \"中文🙂\": \"escaped \\\" } [\"\n    }\n  ],\n  \"b\": {\n    \"value\": 2\n  },\n  \"last\": true\n}";
    assert_eq!(
        blocks(text).keys().copied().collect::<Vec<_>>(),
        [0, 1, 2, 6]
    );
    assert!(blocks("{\n  \"unfinished\": [\n").is_empty());
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let base = CodeCache::default().layout_width(ui, text, false, "", 140.0);
            fold_galley(ui, id, base.clone(), None, true);
            for line in [2, 1, 6] {
                collapse(ctx, id, line);
            }
            let folded = fold_galley(ui, id, base.clone(), None, true);
            assert!(folded.size().y < base.size().y);
            assert_eq!(folded.job.text, text);
            let reconstructed: String = folded
                .rows
                .iter()
                .flat_map(|row| {
                    row.glyphs
                        .iter()
                        .map(|glyph| glyph.chr)
                        .chain(row.ends_with_newline.then_some('\n'))
                })
                .collect();
            assert_eq!(reconstructed, text);
            assert_eq!(folded.end().index, text.chars().count());
            assert!(
                folded
                    .rows
                    .windows(2)
                    .all(|rows| rows[0].pos.y <= rows[1].pos.y)
            );
            let offset = text[..text.find("\"last\"").unwrap()].chars().count();
            let cursor = egui::text::CCursor::new(offset);
            let rect = folded.pos_from_cursor(cursor);
            assert_eq!(
                folded.cursor_from_pos(rect.center().to_vec2()).index,
                offset
            );
            expand_editor_folds(ctx, id);
            assert!(Arc::ptr_eq(
                &base,
                &fold_galley(ui, id, base.clone(), None, true)
            ));
            collapse(ctx, id, 1);
            let changed =
                CodeCache::default().layout_width(ui, "{\n  \"new\": true\n}", false, "", 140.0);
            assert!(Arc::ptr_eq(
                &changed,
                &fold_galley(ui, id, changed.clone(), None, true)
            ));
        });
    });
}

#[test]
fn comparison_folds_preserve_alignment_through_gaps_wrapping_and_resize() {
    let ctx = egui::Context::default();
    let left = format!(
        "{{\n  \"long_{}_key\": {{\n    \"extra\": 0,\n    \"value\": \"{}\"\n  }},\n  \"last\": true\n}}",
        "name".repeat(8),
        "中文🙂".repeat(20)
    );
    let right = "{\n  \"short_key\": {\n    \"value\": \"short\"\n  },\n  \"last\": true\n}";
    let alignment = Arc::new(forma::alignment::align(&left, right));
    let a_id = egui::Id::new("fold_left_test");
    let b_id = egui::Id::new("fold_right_test");
    let mut a_cache = CodeCache::default();
    let mut b_cache = CodeCache::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            for width in [180.0, 120.0, 320.0] {
                let a = a_cache.layout_width(ui, &left, false, "", width);
                let b = b_cache.layout_width(ui, right, false, "", width);
                let a = a_cache.align(ui, a, Some(&alignment), true, right, width);
                let b = b_cache.align(ui, b, Some(&alignment), false, &left, width);
                fold_galley(ui, a_id, a.clone(), Some(&alignment), true);
                collapse(ctx, a_id, 1);
                let a = fold_galley(ui, a_id, a, Some(&alignment), true);
                let b = fold_galley(ui, b_id, b, Some(&alignment), false);
                let y = |galley: &egui::Galley, text: &str| {
                    galley
                        .pos_from_cursor(egui::text::CCursor::new(
                            text[..text.find("\"last\"").unwrap()].chars().count(),
                        ))
                        .top()
                };
                assert_eq!(y(&a, &left), y(&b, right));
                assert_eq!(a.size().y, b.size().y);
                assert!(a.rows.iter().any(|row| row.size.y == 0.0));
                assert!(b.rows.iter().any(|row| row.size.y == 0.0));
            }
        });
    });
}

#[test]
fn gutter_click_folds_copy_preserves_hidden_text_and_typing_reveals_it() {
    let ctx = egui::Context::default();
    configure(&ctx, false);
    let id = egui::Id::new("fold_widget");
    let original = "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": true\n}";
    let mut text = original.to_owned();
    let mut cache = CodeCache::default();
    let mut output = None;
    let mut render = |ctx: &egui::Context, events| {
        ctx.run(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(400.0, 320.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    output = Some(
                        editor_scroll(
                            ui,
                            EditorViewport {
                                salt: "fold_scroll",
                                wrap: true,
                                height: 280.0,
                                offset: None,
                                palette: palette(false),
                                left: true,
                                first_line: 1,
                                base: 0,
                                alignment: None,
                                syntax_error: None,
                                font_size: EDITOR_FONT_SIZE,
                            },
                            |ui| {
                                let changed = prepare_code_input(ui, "fold_widget", &mut text);
                                let mut layouter =
                                    |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
                                        let galley = cache.layout_width(
                                            ui,
                                            buffer.as_str(),
                                            false,
                                            "",
                                            width,
                                        );
                                        fold_galley(ui, id, galley, None, true)
                                    };
                                show_code_editor(
                                    ui,
                                    "fold_widget",
                                    changed,
                                    EDITOR_LINE_HEIGHT,
                                    egui::TextEdit::multiline(&mut text)
                                        .id(id)
                                        .code_editor()
                                        .frame(false)
                                        .margin(Vec2::ZERO)
                                        .layouter(&mut layouter),
                                )
                            },
                        )
                        .inner,
                    );
                });
            },
        )
    };
    render(&ctx, vec![]);
    let pos = egui::pos2(8.0 + EDITOR_GUTTER_WIDTH - 8.0, 18.0);
    for pressed in [true, false] {
        render(
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    render(&ctx, vec![]);
    let fold = ctx
        .data(|data| data.get_temp::<FoldLayout>(id.with("fold_layout")))
        .unwrap();
    assert!(
        fold.result.rows.iter().any(|row| row.size.y == 0.0),
        "gutter arrow must fold the root"
    );
    let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(original.chars().count()),
        )));
    state.store(&ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    let copied = render(&ctx, vec![egui::Event::Copy]);
    assert!(copied.platform_output.commands.iter().any(
        |command| matches!(command, egui::OutputCommand::CopyText(value) if value == original)
    ));
    ctx.tessellate(copied.shapes, copied.pixels_per_point);
    render(&ctx, vec![egui::Event::Text("{}".into())]);
    drop(render);
    assert_eq!(text, "{}");
    assert!(
        output
            .unwrap()
            .galley
            .rows
            .iter()
            .all(|row| row.size.y > 0.0)
    );
}
