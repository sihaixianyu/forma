use super::*;

impl App {
    pub(super) fn source_actions(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.add_enabled_ui(!self.busy || self.automatic_job, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().interact_size = Vec2::new(0.0, 24.0);
            ui.spacing_mut().button_padding = Vec2::new(8.0, 0.0);
            let sort = sort_button(ui, p, self.sort_state)
                .on_hover_text("Click to cycle: original → ascending → descending → original");
            if sort.clicked() {
                let state = self.sort_state.after_single_click();
                if state == SortState::Original {
                    preserve_editor_undo(ui.ctx(), "json_source", &self.editor);
                }
                self.apply_sort_state(state);
            }
            segment_divider(ui, p, 16.0);
            if segmented_button(ui, "\u{f065}  Expand", p, false, Vec2::new(0.0, 24.0))
                .on_hover_text("Expand JSON encoded in strings")
                .clicked()
            {
                self.reset_sort_state();
                preserve_editor_undo(ui.ctx(), "json_source", &self.editor);
                self.parse(Format::Nested);
            }
            segment_divider(ui, p, 16.0);
            if segmented_button(ui, "\u{f066}  Minify", p, false, Vec2::new(0.0, 24.0)).clicked() {
                self.reset_sort_state();
                preserve_editor_undo(ui.ctx(), "json_source", &self.editor);
                self.parse(Format::Compact);
            }
            segment_divider(ui, p, 16.0);
            if segmented_button(ui, "\u{f036}  Format", p, false, Vec2::new(0.0, 24.0))
                .on_hover_text("Format JSON · ⌘S / Ctrl+S")
                .clicked()
            {
                self.reset_sort_state();
                preserve_editor_undo(ui.ctx(), "json_source", &self.editor);
                self.parse(Format::Pretty);
            }
        });
    }

    pub(super) fn source_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        if !self.editable && !self.dirty {
            if self.view == View::Compare && self.editor.len() != self.full_text.len() {
                self.editor = (*self.full_text).clone();
                self.preview_start = 0;
                self.preview_line = 1;
            } else if self.view == View::Graph && self.editor.len() > json_lens::PREVIEW_LIMIT {
                self.editor = preview(&self.full_text, json_lens::PREVIEW_LIMIT).into();
                self.preview_start = 0;
                self.preview_line = 1;
            }
        }
        let height = ui.available_height();
        egui::Frame::new()
            .fill(p.surface)
            .show(ui, |ui| {
                let header_top = ui.cursor().top();
                ui.set_min_height(height.max(100.0));
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                self.text_search(ui, p);
                if self.search.jump {
                    self.prepare_search_jump();
                }
                if !self.editable {
                    egui::Frame::new()
                        .fill(p.tint)
                        .corner_radius(6)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(if self.view == View::Compare {
                                    "Large document · full source shown for comparison".into()
                                } else if self.preview_start > 0 {
                                    format!("Large document · preview near line {}", self.preview_line)
                                } else {
                                    "Large document · previewing the first 16 KiB".into()
                                })
                                .size(12.0)
                                .color(p.accent),
                            );
                            ui.label(
                                RichText::new("Every command uses the complete file.")
                                    .size(11.0)
                                    .color(p.muted),
                            );
                            if ui
                                .add_enabled_ui(!self.busy, |ui| {
                                    button(ui, "Enable full editing", p)
                                })
                                .inner
                                .clicked()
                            {
                                self.editor = (*self.full_text).clone();
                                self.editable = true;
                                self.preview_start = 0;
                                self.preview_line = 1;
                            }
                        });
                }
                self.source_header_height = ui.cursor().top() - header_top;
                align_content(ui, header_top, self.header_height());
                self.pane_content_top = ui.cursor().top();
                let editor_height = ui.available_height().max(0.0);
                let editable = self.editable && (!self.busy || self.automatic_job);
                let alignment = if self.view == View::Compare && self.preview_start == 0 {
                    self.compare_alignment.clone()
                } else {
                    None
                };
                let offset = (self.view == View::Compare).then_some(self.compare_scroll);
                let syntax_error = self.syntax_error.clone();
                let scroll = editor_scroll(
                    ui,
                    EditorViewport {
                        salt: "source_scroll",
                        wrap: true,
                        height: editor_height,
                        offset,
                        palette: p,
                        left: true,
                        first_line: self.preview_line,
                        base: self.preview_start,
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
                        let mut layouter =
                            |ui: &egui::Ui, text: &dyn egui::TextBuffer, _width: f32| {
                                let galley = self.code.layout_width(
                                    ui,
                                    text.as_str(),
                                    self.dark,
                                    &self.search.text.query,
                                    wrap_width,
                                );
                                self.code.align(
                                    ui,
                                    galley,
                                    alignment.as_ref(),
                                    true,
                                    &self.compare_text,
                                    wrap_width,
                                )
                            };
                        let indentation_changed =
                            editable && prepare_code_input(ui, "json_source", &mut self.editor);
                        let mut output = show_code_editor(
                            ui,
                            "json_source",
                            indentation_changed,
                            egui::TextEdit::multiline(&mut self.editor)
                                .id(egui::Id::new("json_lens_source"))
                                .font(egui::TextStyle::Monospace)
                                .code_editor()
                                .frame(false)
                                .margin(Vec2::ZERO)
                                .interactive(editable)
                                .desired_width(ui.available_width())
                                .desired_rows(1)
                                .min_size(Vec2::new(0.0, editor_height))
                                .layouter(&mut layouter),
                        );
                        decoded_hover(ui, &output, &self.editor);
                        #[cfg(not(target_arch = "wasm32"))]
                        if self.smoke_test && std::env::var_os("JSON_LENS_CURSOR").is_some() {
                            output.response.request_focus();
                            let cursor = egui::text::CCursor::new(12);
                            output
                                .state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                            output.state.clone().store(ui.ctx(), output.response.id);
                        }
                        if output.response.changed() {
                            self.reset_sort_state();
                            self.syntax_error = None;
                            self.revision += 1;
                            self.clear_comparison();
                            self.auto_due = Some(
                                web_time::Instant::now() + std::time::Duration::from_millis(500),
                            );
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_millis(500));
                            self.source_target = None;
                            self.pending_reveal = None;
                            self.pending_source_offset = None;
                            self.canvas.edit_requested = None;
                            self.dirty = true;
                            self.search_source = Arc::new(self.editor.clone());
                            self.search.text.changed();
                            self.search.tree.changed();
                            self.search.jump = false;
                        }
                        let source_clicked = ui.input(|input| {
                            input.pointer.button_clicked(egui::PointerButton::Primary)
                                && input.pointer.interact_pos().is_some_and(|mouse| {
                                    output.text_clip_rect.contains(mouse)
                                        && output.response.rect.contains(mouse)
                                })
                        }) && ui.rect_contains_pointer(output.text_clip_rect);
                        if self.view == View::Graph
                            && (output.response.clicked()
                                || source_clicked
                                || (!editable
                                    && ui
                                        .interact(
                                            output.response.rect.intersect(output.text_clip_rect),
                                            output.response.id.with("preview_click"),
                                            egui::Sense::click(),
                                        )
                                        .clicked()))
                            && !output.response.changed()
                            && let Some(mouse) = ui.input(|input| input.pointer.interact_pos())
                        {
                            let cursor = output.galley.cursor_from_pos(mouse - output.galley_pos);
                            let offset = self
                                .editor
                                .char_indices()
                                .nth(cursor.index)
                                .map_or(self.editor.len(), |(offset, _)| offset);
                            self.pending_source_offset = Some(self.preview_start + offset);
                            log::debug!(target: "json_lens", "event=source_graph_click bytes={} offset={}", self.full_text.len(), self.preview_start + offset);
                            self.source_target = None;
                            self.pending_reveal = None;
                        }
                        if self.search.jump {
                            if let Some(range) = self.source_target.as_ref().or_else(|| {
                                self.search
                                    .text
                                    .matches
                                    .ranges
                                    .get(self.search.text.selected)
                            }) && range.start >= self.preview_start
                                && range.end <= self.preview_start + self.editor.len()
                            {
                                let start = self.editor[..range.start - self.preview_start]
                                    .chars()
                                    .count();
                                let end = start
                                    + self.editor[range.start - self.preview_start
                                        ..range.end - self.preview_start]
                                        .chars()
                                        .count();
                                let cursor = egui::text::CCursor::new(start);
                                output.state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::two(
                                        cursor,
                                        egui::text::CCursor::new(end),
                                    ),
                                ));
                                output.state.clone().store(ui.ctx(), output.response.id);
                                if self.source_target.is_some() {
                                    output.response.request_focus();
                                }
                                let target = output
                                    .galley
                                    .pos_from_cursor(cursor)
                                    .translate(output.galley_pos.to_vec2());
                                ui.scroll_to_rect(target, Some(egui::Align::Center));
                            }
                            self.search.jump = false;
                        }
                        if self.view == View::Compare
                            && let Some(range) = &self.compare_left_range
                        {
                            highlight_difference(
                                ui,
                                &output,
                                &self.editor,
                                range,
                                self.preview_start,
                                p.red,
                            );
                        }
                        if self.error_jump == Some(false) {
                            self.error_jump = None;
                            if let Some(error) = &self.syntax_error {
                                reveal_error(ui, &mut output, error, self.preview_start);
                            }
                        }
                        self.code.paint_matches(ui, &output, &self.search.text.matches.ranges, self.search.text.selected, self.preview_start, p);
                        output
                    },
                );
                if self.view == View::Compare {
                    self.compare_scroll = scroll.state.offset.y;
                }
            });
    }
}
