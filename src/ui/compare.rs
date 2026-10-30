use super::*;

impl App {
    pub(super) fn next_difference(&mut self, backwards: bool) {
        let count = self.differences.len();
        if count == 0 {
            return;
        }
        self.compare_selected = if backwards {
            (self.compare_selected + count - 1) % count
        } else {
            (self.compare_selected + 1) % count
        };
        let pointer = self.differences[self.compare_selected].pointer.clone();
        let left = self.source();
        self.submit(Job::LocateDiff(
            left,
            Arc::new(self.compare_text.clone()),
            pointer,
        ));
    }
    pub(super) fn header_height(&self) -> f32 {
        if self.view == View::Compare {
            self.source_header_height.max(self.right_header_height)
        } else {
            self.source_header_height
        }
    }

    pub(super) fn align_right_content(&mut self, ui: &mut egui::Ui, top: f32) {
        let measured = ui.cursor().top() - top;
        if (measured - self.right_header_height).abs() > 0.5 {
            ui.ctx().request_repaint();
        }
        self.right_header_height = measured;
        if self.view == View::Graph {
            return;
        }
        if ui.cursor().top() > self.pane_content_top + 0.5 {
            ui.ctx().request_discard("Align pane contents");
        }
        align_content(ui, self.pane_content_top, 0.0);
    }

    pub(super) fn compare_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        let height = ui.available_height();
        egui::Frame::new().fill(p.surface).show(ui, |ui| {
            let top = ui.cursor().top();
            ui.set_min_height(height.max(100.0));
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.horizontal(|ui| {
                if !self.compare_text.trim().is_empty()
                    || self.comparison_done
                    || self.syntax_error.is_some()
                    || self.right_syntax_error.is_some()
                {
                    let count = self.differences.len();
                    let syntax = self
                        .right_syntax_error
                        .as_ref()
                        .map(|error| ("Right", error))
                        .or_else(|| self.syntax_error.as_ref().map(|error| ("Left", error)));
                    let both_invalid =
                        self.syntax_error.is_some() && self.right_syntax_error.is_some();
                    let message = if both_invalid {
                        "Both inputs: Invalid JSON".into()
                    } else if let Some((side, error)) = syntax {
                        format!("{side}: {}", error.summary())
                    } else if !self.comparison_done {
                        if self.error {
                            self.status.clone()
                        } else {
                            "Updating comparison…".into()
                        }
                    } else if count == 0 {
                        "Both documents are equal".into()
                    } else {
                        format!(
                            "Difference {} / {}{}",
                            self.compare_selected + 1,
                            count,
                            if count == 1000 { " (limit)" } else { "" }
                        )
                    };
                    let summary = ui.label(
                        RichText::new(message)
                            .size(12.0)
                            .color(if syntax.is_some() { p.red } else { p.muted }),
                    );
                    if let Some((_, error)) = syntax {
                        summary.clone().on_hover_text(if both_invalid {
                            self.status.as_str()
                        } else {
                            error.message.as_str()
                        });
                    }
                    if syntax.is_some() {
                        let label = ui.ctx().format_shortcut(&error_shortcut());
                        if ui.button(format!("Go to error · {label}")).clicked() {
                            self.jump_to_error(ui.ctx(), self.active_right);
                        }
                    }
                    if let Some(diff) = self.differences.get(self.compare_selected) {
                        summary.on_hover_text(format!(
                            "{}\n− {}\n+ {}",
                            diff.pointer, diff.left, diff.right
                        ));
                    }
                    if ui
                        .add_enabled(
                            count > 0 && !self.busy,
                            egui::Button::new("<").min_size(Vec2::new(28.0, 28.0)),
                        )
                        .on_hover_text("Previous difference")
                        .clicked()
                    {
                        self.next_difference(true);
                    }
                    if ui
                        .add_enabled(
                            count > 0 && !self.busy,
                            egui::Button::new(">").min_size(Vec2::new(28.0, 28.0)),
                        )
                        .on_hover_text("Next difference")
                        .clicked()
                    {
                        self.next_difference(false);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format_size(self.compare_text.len()))
                            .size(11.0)
                            .color(p.muted),
                    );
                });
            });
            self.path_separator(ui, p, true);
            self.right_search(ui, p);
            self.align_right_content(ui, top);
            let alignment = self.compare_alignment.clone();
            let height = ui.available_height().max(0.0);
            let offset = self.compare_scroll;
            let syntax_error = self.right_syntax_error.clone();
            let scroll = editor_scroll(
                ui,
                EditorViewport {
                    salt: "compare_scroll",
                    wrap: true,
                    height,
                    offset: Some(offset),
                    palette: p,
                    left: false,
                    first_line: 1,
                    base: 0,
                    alignment: alignment.as_deref(),
                    syntax_error: syntax_error.as_ref(),
                },
                |ui| {
                    let wrap_width = if self.view == View::Compare {
                        (ui.ctx().content_rect().width()
                            * self.split_ratio.min(1.0 - self.split_ratio)
                            - 104.0)
                            .max(80.0)
                    } else {
                        ui.available_width().max(80.0)
                    };
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _width: f32| {
                        let galley = self.compare_code.layout_width(
                            ui,
                            text.as_str(),
                            self.dark,
                            &self.search.right.query,
                            wrap_width,
                        );
                        self.compare_code.align(
                            ui,
                            galley,
                            alignment.as_ref(),
                            false,
                            &self.editor,
                            wrap_width,
                        )
                    };
                    let indentation_changed = (!self.busy || self.automatic_job)
                        && prepare_code_input(ui, "compare_editor", &mut self.compare_text);
                    let mut output = show_code_editor(
                        ui,
                        "compare_editor",
                        indentation_changed,
                        egui::TextEdit::multiline(&mut self.compare_text)
                            .id(egui::Id::new("json_lens_compare"))
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .frame(false)
                            .margin(Vec2::ZERO)
                            .interactive(!self.busy || self.automatic_job)
                            .desired_width(ui.available_width())
                            .desired_rows(1)
                            .min_size(Vec2::new(0.0, height))
                            .hint_text("Paste JSON here…")
                            .layouter(&mut layouter),
                    );
                    decoded_hover(ui, &output, &self.compare_text);
                    if output.response.changed() {
                        self.right_syntax_error = None;
                        self.right_json_valid = false;
                        self.search.right.changed();
                        self.revision += 1;
                        self.clear_comparison();
                        if self.dirty {
                            self.auto_due = Some(
                                web_time::Instant::now() + std::time::Duration::from_millis(500),
                            );
                        }
                    }
                    output.response.context_menu(|ui| {
                        ui.add_enabled_ui(!self.busy || self.automatic_job, |ui| {
                            if ui.button("Format").clicked() {
                                preserve_editor_undo(
                                    ui.ctx(),
                                    "compare_editor",
                                    &self.compare_text,
                                );
                                self.submit(Job::FormatRight(Arc::new(self.compare_text.clone())));
                                ui.close();
                            }
                            if ui.button("Clear").clicked() {
                                self.compare_text.clear();
                                self.right_syntax_error = None;
                                self.right_json_valid = false;
                                self.search.right.changed();
                                self.revision += 1;
                                self.clear_comparison();
                                ui.close();
                                ui.ctx().request_discard("Right document cleared");
                            }
                        });
                    });
                    if self.search.right_jump {
                        if let Some(range) = self
                            .search
                            .right
                            .matches
                            .ranges
                            .get(self.search.right.selected)
                        {
                            let cursor = egui::text::CCursor::new(
                                self.compare_text[..range.start].chars().count(),
                            );
                            ui.scroll_to_rect(
                                output
                                    .galley
                                    .pos_from_cursor(cursor)
                                    .translate(output.galley_pos.to_vec2()),
                                Some(egui::Align::Center),
                            );
                        }
                        self.search.right_jump = false;
                    }
                    if let Some(range) = &self.compare_right_range {
                        highlight_difference(ui, &output, &self.compare_text, range, 0, p.green);
                        if self.compare_jump {
                            let cursor = egui::text::CCursor::new(
                                self.compare_text[..range.start].chars().count(),
                            );
                            ui.scroll_to_rect(
                                output
                                    .galley
                                    .pos_from_cursor(cursor)
                                    .translate(output.galley_pos.to_vec2()),
                                Some(egui::Align::Center),
                            );
                        }
                    }
                    self.compare_jump = false;
                    if self.error_jump == Some(true) {
                        self.error_jump = None;
                        if let Some(error) = &self.right_syntax_error {
                            reveal_error(ui, &mut output, error, 0);
                        }
                    }
                    self.compare_code.paint_matches(
                        ui,
                        &output,
                        &self.search.right.matches.ranges,
                        self.search.right.selected,
                        0,
                        p,
                    );
                    output
                },
            );
            if (scroll.state.offset.y - offset).abs() > 0.5 {
                self.compare_scroll = scroll.state.offset.y;
                ui.ctx().request_discard("Synchronize comparison scrolling");
            }
        });
    }
}
