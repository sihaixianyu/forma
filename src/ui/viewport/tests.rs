use super::*;

#[test]
fn gutter_marks_parser_error_line_and_click_moves_the_editor_cursor() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    configure(&ctx, false);
    let mut text = "{\n  \"中文\": }".to_owned();
    let message = json_lens::parse_json(&text).unwrap_err();
    let diagnostic = crate::syntax::SyntaxError::new(&message, &text);
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
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
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
    assert!(marker.width() >= 60.0);
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
