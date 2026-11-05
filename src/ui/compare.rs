use super::*;

impl App {
    fn right_source_changed(&mut self) {
        self.review_original_right = None;
        self.reset_sort_state();
        self.right_syntax_error = None;
        self.right_json_valid = false;
        self.right_pointer.clear();
        self.right_caret_offset = None;
        self.pending_right_source_offset = None;
        self.search.right.changed();
        self.revision += 1;
        self.clear_comparison();
        if self.dirty {
            self.auto_due = Some(web_time::Instant::now() + std::time::Duration::from_millis(500));
        }
    }

    pub(super) fn next_difference(&mut self, backwards: bool) {
        let count = self.differences.len();
        if count == 0 {
            return;
        }
        let next = if backwards {
            (self.compare_selected + count - 1) % count
        } else {
            (self.compare_selected + 1) % count
        };
        self.select_difference(next);
    }
    pub(super) fn select_difference(&mut self, index: usize) {
        if self.busy || index >= self.differences.len() {
            return;
        }
        self.compare_selected = index;
        let difference = self.differences[index].clone();
        self.pointer = difference.left_pointer.clone().unwrap_or_default();
        self.right_pointer = difference.right_pointer.clone().unwrap_or_default();
        let left = self.source();
        self.submit(Job::LocateDiff(
            left,
            Arc::new(self.compare_text.clone()),
            difference,
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
        align_content(ui, top, self.header_height());
    }

    pub(super) fn comparison_header(&mut self, ui: &mut egui::Ui, p: Palette, right_pane: bool) {
        let (removed, added) = self.compare_alignment.as_ref().map_or((0, 0), |alignment| {
            (
                alignment
                    .left
                    .changed
                    .iter()
                    .filter(|changed| **changed)
                    .count(),
                alignment
                    .right
                    .changed
                    .iter()
                    .filter(|changed| **changed)
                    .count(),
            )
        });
        let show = self.only_differences
            || !self.differences.is_empty()
            || removed + added > 0
            || self.syntax_error.is_some()
            || self.right_syntax_error.is_some();
        if !show {
            return;
        }
        let can_navigate = !self.busy && !self.differences.is_empty();
        let viewport = ui.ctx().content_rect();
        let middle = viewport.left() + viewport.width() * self.split_ratio;
        let bounds = egui::Rect::from_min_max(
            egui::pos2(
                if right_pane { middle } else { viewport.left() },
                ui.cursor().top(),
            ),
            egui::pos2(
                if right_pane { viewport.right() } else { middle },
                ui.cursor().top() + 26.0,
            ),
        );
        ui.painter()
            .with_clip_rect(viewport)
            .rect_filled(bounds, 0, p.subtle);
        ui.painter().with_clip_rect(viewport).vline(
            middle,
            bounds.y_range(),
            Stroke::new(1.0, p.border),
        );
        ui.scope_builder(egui::UiBuilder::new(), |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.set_min_height(20.0);
                    ui.style_mut().override_font_id = Some(FontId::proportional(11.0));
                    ui.spacing_mut().interact_size = Vec2::new(20.0, 20.0);
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    ui.spacing_mut().button_padding = Vec2::new(3.0, 1.0);
                    ui.horizontal(|ui| {
                        let compact = ui.available_width() < 420.0;
                        let count = self.differences.len();
                        let syntax = self
                            .right_syntax_error
                            .as_ref()
                            .map(|error| ("Right", error))
                            .or_else(|| self.syntax_error.as_ref().map(|error| ("Left", error)));
                        let both_invalid =
                            self.syntax_error.is_some() && self.right_syntax_error.is_some();
                        let has_syntax_error = syntax.is_some();
                        let message = if both_invalid {
                            "Both inputs: Invalid JSON".into()
                        } else if let Some((side, error)) = syntax {
                            format!("{side}: {}", error.summary())
                        } else if count == 0 {
                            if removed + added > 0 {
                                "Text changes".into()
                            } else {
                                "No differences".into()
                            }
                        } else if compact {
                            format!("{} / {}", self.compare_selected + 1, count)
                        } else {
                            format!(
                                "Difference {} / {}{}",
                                self.compare_selected + 1,
                                count,
                                if count == 1000 { " (limit)" } else { "" }
                            )
                        };
                        let summary = ui.add(
                            egui::Label::new(
                                RichText::new(message)
                                    .size(11.0)
                                    .color(if syntax.is_some() { p.red } else { p.text }),
                            )
                            .truncate()
                            .show_tooltip_when_elided(false),
                        );
                        if let Some((_, error)) = syntax {
                            summary.clone().on_hover_text(if both_invalid {
                                self.status.as_str()
                            } else {
                                error.message.as_str()
                            });
                            let label = ui.ctx().format_shortcut(&error_shortcut());
                            if ui.button(format!("Go to error · {label}")).clicked() {
                                self.jump_to_error(ui.ctx(), self.right_syntax_error.is_some());
                            }
                        }
                        if let Some(diff) = self.differences.get(self.compare_selected) {
                            summary.on_hover_text(format!(
                                "{}\n− {}\n+ {}\nOriginal right path: {}{}",
                                diff.pointer,
                                diff.left,
                                diff.right,
                                diff.original_right_pointer
                                    .as_deref()
                                    .unwrap_or("(missing)"),
                                if diff.uncertain {
                                    "\nUncertain automatic match"
                                } else {
                                    ""
                                }
                            ));
                        }
                        if !has_syntax_error && !compact {
                            ui.label(RichText::new(format!("−{removed}")).size(11.0).color(p.red))
                                .on_hover_text("Removed or modified lines");
                            ui.label(RichText::new(format!("+{added}")).size(11.0).color(p.green))
                                .on_hover_text("Added or modified lines");
                        }
                        if ui
                            .add_enabled(
                                can_navigate,
                                egui::Button::new("‹")
                                    .min_size(Vec2::new(20.0, 20.0))
                                    .corner_radius(5),
                            )
                            .on_hover_text("Previous difference")
                            .clicked()
                        {
                            self.next_difference(true);
                        }
                        if ui
                            .add_enabled(
                                can_navigate,
                                egui::Button::new("›")
                                    .min_size(Vec2::new(20.0, 20.0))
                                    .corner_radius(5),
                            )
                            .on_hover_text("Next difference")
                            .clicked()
                        {
                            self.next_difference(false);
                        }
                        if !compact {
                            let size = if right_pane {
                                self.compare_text.len()
                            } else {
                                self.editor.len()
                            };
                            ui.label(RichText::new(format_size(size)).color(p.muted));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .checkbox(&mut self.only_differences, "Diff Only")
                                .changed()
                            {
                                self.review_focus = Some(self.only_differences);
                                reset_review_folds(ui.ctx());
                                self.compare_scroll = 0.0;
                                ui.ctx().data_mut(|data| {
                                    data.insert_temp(
                                        egui::Id::new("forma_only_differences"),
                                        self.only_differences,
                                    )
                                });
                            }
                        });
                    });
                });
        });
        self.path_separator(ui, p, right_pane);
    }

    pub(super) fn compare_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        let height = ui.available_height();
        egui::Frame::new().fill(p.surface).show(ui, |ui| {
            let top = ui.cursor().top();
            ui.set_min_height(height.max(100.0));
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().button_padding = Vec2::ZERO;
            self.comparison_header(ui, p, true);
            self.right_search(ui, p);
            self.align_right_content(ui, top);
            let alignment = self.compare_alignment.clone();
            let height = ui.available_height().max(0.0);
            let offset = self.compare_scroll;
            let syntax_error = self.right_syntax_error.clone();
            let typography = self.editor_typography;
            if self.search.right_jump || self.error_jump == Some(true) || self.compare_jump {
                expand_editor_folds(ui.ctx(), egui::Id::new("forma_compare"));
            }
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
                    font_size: typography.font_size,
                },
                |ui| {
                    ui.style_mut().text_styles.insert(
                        egui::TextStyle::Monospace,
                        FontId::monospace(typography.font_size),
                    );
                    let wrap_width = if self.view == View::Compare {
                        (ui.ctx().content_rect().width()
                            * self.split_ratio.min(1.0 - self.split_ratio)
                            - 120.0)
                            .max(80.0)
                    } else {
                        ui.available_width().max(80.0)
                    };
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _width: f32| {
                        let galley = self.compare_code.layout_width_with_typography(
                            ui,
                            text.as_str(),
                            self.dark,
                            &self.search.right.query,
                            wrap_width,
                            typography,
                        );
                        let galley = self.compare_code.align(
                            ui,
                            galley,
                            alignment.as_ref(),
                            false,
                            &self.editor,
                            wrap_width,
                        );
                        fold_galley(
                            ui,
                            egui::Id::new("forma_compare"),
                            galley,
                            alignment.as_deref(),
                            false,
                        )
                    };
                    let indentation_changed = (!self.busy || self.automatic_job)
                        && prepare_code_input(ui, "compare_editor", &mut self.compare_text);
                    let mut output = show_code_editor(
                        ui,
                        "compare_editor",
                        indentation_changed,
                        typography.line_height,
                        egui::TextEdit::multiline(&mut self.compare_text)
                            .id(egui::Id::new("forma_compare"))
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
                    if !output.response.changed()
                        && output.response.has_focus()
                        && let Some(offset) = caret_source_offset(&output, &self.compare_text)
                        && self.right_caret_offset != Some(offset)
                    {
                        self.right_caret_offset = Some(offset);
                        self.pending_right_source_offset = Some(offset);
                    }
                    if output.response.changed() {
                        self.right_source_changed();
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
                                self.right_source_changed();
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
                ui.ctx().request_repaint();
            }
        });
    }
}
