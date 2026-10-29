use super::*;

pub(super) fn error_shortcut() -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(
        egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
        egui::Key::E,
    )
}

pub(super) fn reveal_error(
    ui: &mut egui::Ui,
    output: &mut egui::text_edit::TextEditOutput,
    error: &crate::syntax::SyntaxError,
    base: usize,
) {
    let Some(offset) = error.offset.and_then(|offset| offset.checked_sub(base)) else {
        return;
    };
    let Some(before) = output.galley.job.text.get(..offset) else {
        return;
    };
    let cursor = egui::text::CCursor::new(before.chars().count());
    output
        .state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
    output.state.clone().store(ui.ctx(), output.response.id);
    output.response.request_focus();
    ui.scroll_to_rect(
        output
            .galley
            .pos_from_cursor(cursor)
            .translate(output.galley_pos.to_vec2()),
        Some(egui::Align::Center),
    );
}

impl App {
    pub(super) fn error_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| {
            input.consume_shortcut(&error_shortcut())
                || input.consume_key(
                    egui::Modifiers::CTRL.plus(egui::Modifiers::SHIFT),
                    egui::Key::E,
                )
        }) {
            self.jump_to_error(ctx, self.active_right);
        }
    }

    pub(super) fn jump_to_error(&mut self, ctx: &egui::Context, prefer_right: bool) {
        let right = self.view == View::Compare
            && self.right_syntax_error.is_some()
            && (prefer_right || self.syntax_error.is_none());
        let error = if right {
            self.right_syntax_error.as_ref()
        } else {
            self.syntax_error.as_ref()
        };
        let Some(offset) = error.and_then(|error| error.offset) else {
            return;
        };
        if !right {
            self.graph_wide = false;
            if !self.editable {
                let mut start = offset.saturating_sub(4096);
                while !self.full_text.is_char_boundary(start) {
                    start -= 1;
                }
                self.preview_start = start;
                self.preview_line = self.full_text[..start]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1;
                self.editor = preview(&self.full_text[start..], json_lens::PREVIEW_LIMIT).into();
            }
            self.search.jump = false;
        } else {
            self.search.right_jump = false;
        }
        self.active_right = right;
        self.error_jump = Some(right);
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invalid_app(ctx: &egui::Context) -> App {
        let mut app = App::create(ctx.clone(), "UI test".into());
        app.busy = false;
        app.document = None;
        app.editor = "{\n  \"中文\": }".into();
        app.full_text = Arc::new(app.editor.clone());
        app.search_source = app.full_text.clone();
        app.syntax_error = Some(crate::syntax::SyntaxError::new(
            &json_lens::parse_json(&app.editor).unwrap_err(),
            &app.editor,
        ));
        app
    }

    #[test]
    fn invalid_status_is_red_and_has_no_waiting_or_explanation_text() {
        let ctx = egui::Context::default();
        let mut app = invalid_app(&ctx);
        let output = ctx.run(egui::RawInput::default(), |ctx| app.workspace(ctx));
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text)
                } else {
                    None
                }
            })
            .collect();
        assert!(
            !texts
                .iter()
                .any(|text| text.galley.job.text.contains("Awaiting JSON"))
        );
        assert!(
            !texts
                .iter()
                .any(|text| text.galley.job.text.contains("Fix the marked"))
        );
        let badge = texts
            .iter()
            .find(|text| text.galley.job.text == "Invalid JSON")
            .unwrap();
        assert_eq!(
            badge.galley.job.sections[0].format.color,
            palette(app.dark).red
        );
        assert!(
            texts
                .iter()
                .any(|text| text.galley.job.text.contains("Go to error"))
        );
    }

    #[test]
    fn error_shortcut_focuses_and_positions_the_active_editor() {
        for modifiers in [egui::Modifiers::COMMAND, egui::Modifiers::CTRL] {
            for right in [false, true] {
                let ctx = egui::Context::default();
                let mut app = invalid_app(&ctx);
                if right {
                    app.view = View::Compare;
                    app.compare_text = "{\n  \"右边\": }".into();
                    app.right_syntax_error = Some(crate::syntax::SyntaxError::new(
                        &json_lens::parse_json(&app.compare_text).unwrap_err(),
                        &app.compare_text,
                    ));
                }
                app.active_right = right;
                let _ = ctx.run(egui::RawInput::default(), |ctx| app.workspace(ctx));
                let modifiers = modifiers.plus(egui::Modifiers::SHIFT);
                let _ = ctx.run(
                    egui::RawInput {
                        modifiers,
                        events: vec![egui::Event::Key {
                            key: egui::Key::E,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        }],
                        ..Default::default()
                    },
                    |ctx| app.workspace(ctx),
                );
                let (salt, text, error) = if right {
                    (
                        "compare_editor",
                        app.compare_text.as_str(),
                        app.right_syntax_error.as_ref().unwrap(),
                    )
                } else {
                    (
                        "json_source",
                        app.editor.as_str(),
                        app.syntax_error.as_ref().unwrap(),
                    )
                };
                let id = ctx
                    .data(|data| data.get_temp::<egui::Id>(editor_id_key(salt)))
                    .unwrap();
                let state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
                assert_eq!(
                    state.cursor.char_range().unwrap().primary.index,
                    text[..error.offset.unwrap()].chars().count()
                );
                assert!(ctx.memory(|memory| memory.has_focus(id)));
                assert_eq!(app.error_jump, None);
            }
        }
    }
}
