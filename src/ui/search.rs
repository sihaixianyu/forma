use super::*;

impl App {
    pub(crate) fn reset_search(&mut self) {
        self.source_target = None;
        self.search_source = self.full_text.clone();
        self.preview_start = 0;
        self.preview_line = 1;
        self.search.text.changed();
        self.search.tree.changed();
    }

    pub(crate) fn prepare_search_jump(&mut self) {
        let Some(range) = self.source_target.as_ref().or_else(|| {
            self.search
                .text
                .matches
                .ranges
                .get(self.search.text.selected)
        }) else {
            self.search.jump = false;
            return;
        };
        if !self.editable {
            let text = &self.full_text;
            let mut start = range.start.saturating_sub(4096);
            while !text.is_char_boundary(start) {
                start -= 1;
            }
            start = text[..start].rfind('\n').map_or(0, |offset| offset + 1);
            self.preview_start = start;
            self.preview_line = text[..start].bytes().filter(|b| *b == b'\n').count() + 1;
            self.editor = preview(&text[start..], json_lens::PREVIEW_LIMIT).into();
        }
    }
    pub(super) fn text_search(&mut self, ui: &mut egui::Ui, p: Palette) {
        if !self.search.text.visible {
            return;
        }
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search.text.query)
                    .id_salt("text_find")
                    .hint_text("Find in text…")
                    .desired_width((ui.available_width() - 155.0).max(70.0)),
            );
            if self.search.text.focus {
                response.request_focus();
                self.search.text.focus = false;
            }
            if response.changed() {
                self.source_target = None;
                self.search.text.changed();
            }
            if self.search.text.pending {
                ui.spinner();
            } else {
                ui.label(
                    RichText::new(match_count(
                        self.search.text.selected,
                        self.search.text.matches.ranges.len(),
                        self.search.text.matches.total,
                    ))
                    .size(11.0)
                    .color(p.muted),
                );
            }
            let count = self.search.text.matches.ranges.len();
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter {
                response.request_focus();
            }
            if ui
                .add_enabled(count > 0, egui::Button::new("<").frame(false))
                .on_hover_text("Previous match · Shift+Enter")
                .clicked()
                || (enter && ui.input(|i| i.modifiers.shift))
            {
                self.search.text.next(count, true);
                self.source_target = None;
                self.search.jump = true;
            }
            if ui
                .add_enabled(count > 0, egui::Button::new(">").frame(false))
                .on_hover_text("Next match · Enter")
                .clicked()
                || (enter && !ui.input(|i| i.modifiers.shift))
            {
                self.search.text.next(count, false);
                self.source_target = None;
                self.search.jump = true;
            }
            if ui
                .add(egui::Button::new("×").frame(false))
                .on_hover_text("Close search · Escape")
                .clicked()
            {
                self.search.text.close();
                self.search.jump = false;
            }
        });
        if !self.search.text.query.is_empty() {
            ui.label(
                RichText::new(if self.editable {
                    "Literal search · case sensitive"
                } else {
                    "Searching the full file · preview follows the selected match"
                })
                .size(11.0)
                .color(p.muted),
            );
        }
    }

    pub(super) fn graph_search(&mut self, ui: &mut egui::Ui, p: Palette) {
        if !self.search.tree.visible {
            return;
        }
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search.tree.query)
                    .id_salt("tree_find")
                    .hint_text("Find a key or value…")
                    .desired_width((ui.available_width() - 155.0).max(70.0)),
            );
            if self.search.tree.focus {
                response.request_focus();
                self.search.tree.focus = false;
            }
            if response.changed() {
                self.search.tree.changed();
            }
            if self.search.tree.pending {
                ui.spinner();
            } else {
                ui.label(
                    RichText::new(match_count(
                        self.search.tree.selected,
                        self.search.tree.matches.hits.len(),
                        self.search.tree.matches.total,
                    ))
                    .size(11.0)
                    .color(p.muted),
                );
            }
            let count = self.search.tree.matches.hits.len();
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter {
                response.request_focus();
            }
            let previous = ui
                .add_enabled(count > 0, egui::Button::new("<").frame(false))
                .on_hover_text("Previous result")
                .clicked();
            let next = ui
                .add_enabled(count > 0, egui::Button::new(">").frame(false))
                .on_hover_text("Next result")
                .clicked();
            if previous || next || enter {
                self.search.tree.next(
                    count,
                    previous || (enter && ui.input(|i| i.modifiers.shift)),
                );
                if let Some(hit) = self.search.tree.matches.hits.get(self.search.tree.selected) {
                    self.pointer = hit.pointer.clone();
                    self.view = View::Graph;
                    self.canvas.focus(self.pointer.clone());
                }
            }
            if ui
                .add(egui::Button::new("×").frame(false))
                .on_hover_text("Close search · Escape")
                .clicked()
            {
                self.search.tree.close();
            }
        });
        if !self.search.tree.query.is_empty() && !self.search.tree.pending {
            ui.label(
                RichText::new("Decoded keys and values · full document · case sensitive")
                    .size(11.0)
                    .color(p.muted),
            );
            if self.search.tree.matches.hits.is_empty() {
                ui.label(RichText::new("No matching keys or values").color(p.muted));
            }
            egui::ScrollArea::vertical()
                .id_salt("tree_results")
                .max_height(155.0)
                .show_rows(
                    ui,
                    26.0,
                    self.search.tree.matches.hits.len(),
                    |ui, range| {
                        for index in range {
                            let hit = &self.search.tree.matches.hits[index];
                            let path = if hit.pointer.is_empty() {
                                "(root)"
                            } else {
                                &hit.pointer
                            };
                            let text = format!("{path}   {}", hit.value);
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(&text).monospace().size(12.0).color(p.accent),
                                    )
                                    .fill(if index == self.search.tree.selected {
                                        p.tint
                                    } else {
                                        p.surface
                                    })
                                    .truncate()
                                    .min_size(Vec2::new(ui.available_width(), 26.0)),
                                )
                                .on_hover_text(&text)
                                .clicked()
                            {
                                self.search.tree.selected = index;
                                self.pointer = hit.pointer.clone();
                                self.view = View::Graph;
                                self.canvas.focus(self.pointer.clone());
                            }
                        }
                    },
                );
        }
        ui.add_space(6.0);
    }

    pub(super) fn right_search(&mut self, ui: &mut egui::Ui, p: Palette) {
        if !self.search.right.visible {
            return;
        }
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search.right.query)
                    .id_salt("right_find")
                    .hint_text("Find in right text…")
                    .desired_width((ui.available_width() - 155.0).max(70.0)),
            );
            if self.search.right.focus {
                response.request_focus();
                self.search.right.focus = false;
            }
            if response.changed() {
                self.search.right.changed();
            }
            let count = self.search.right.matches.ranges.len();
            if self.search.right.pending {
                ui.spinner();
            } else {
                ui.label(
                    RichText::new(match_count(
                        self.search.right.selected,
                        count,
                        self.search.right.matches.total,
                    ))
                    .size(11.0)
                    .color(p.muted),
                );
            }
            let enter =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if enter {
                response.request_focus();
            }
            if ui
                .add_enabled(count > 0, egui::Button::new("<").frame(false))
                .clicked()
                || (enter && ui.input(|input| input.modifiers.shift))
            {
                self.search.right.next(count, true);
                self.search.right_jump = true;
            }
            if ui
                .add_enabled(count > 0, egui::Button::new(">").frame(false))
                .clicked()
                || (enter && !ui.input(|input| input.modifiers.shift))
            {
                self.search.right.next(count, false);
                self.search.right_jump = true;
            }
            if ui
                .add(egui::Button::new("×").frame(false))
                .on_hover_text("Close search · Escape")
                .clicked()
            {
                self.search.right.close();
                self.search.right_jump = false;
            }
        });
    }

    pub(crate) fn search_shortcuts(&mut self, ctx: &egui::Context) {
        let presses = ctx.input_mut(|input| {
            let mut presses = Vec::new();
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key: egui::Key::F,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                    && (modifiers.command || modifiers.ctrl || modifiers.mac_cmd)
                    && !modifiers.alt
                {
                    if !repeat {
                        presses.push(modifiers.shift);
                    }
                    false
                } else {
                    true
                }
            });
            presses
        });
        // Process every distinct key press, including two arriving before the next frame.
        for shifted in presses {
            if shifted {
                self.active_right = true;
                self.view = View::Graph;
                self.search.tree.toggle();
            } else if !self.active_right {
                self.search.text.toggle();
                if !self.search.text.visible {
                    self.search.jump = false;
                }
            } else if self.view == View::Graph {
                self.search.tree.toggle();
            } else {
                self.search.right.toggle();
                if !self.search.right.visible {
                    self.search.right_jump = false;
                }
            }
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if !self.active_right {
                self.search.text.close();
                self.search.jump = false;
            } else if self.view == View::Graph {
                self.search.tree.close();
            } else {
                self.search.right.close();
                self.search.right_jump = false;
            }
        }
    }
}

pub(super) fn error_shortcut() -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(
        egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
        egui::Key::E,
    )
}

pub(super) fn reveal_error(
    ui: &mut egui::Ui,
    output: &mut egui::text_edit::TextEditOutput,
    error: &crate::jobs::SyntaxError,
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
mod tests;
