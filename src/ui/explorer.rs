use super::*;

impl App {
    pub(super) fn mode_tabs(&mut self, ui: &mut egui::Ui, p: Palette) {
        for (view, label) in [(View::Graph, "Graph"), (View::Compare, "Compare")] {
            let selected = self.view == view;
            if ui
                .add(
                    egui::Button::new(RichText::new(label).color(if selected {
                        p.accent
                    } else {
                        p.muted
                    }))
                    .fill(if selected { p.tint } else { p.surface })
                    .min_size(Vec2::new(76.0, 34.0))
                    .corner_radius(7),
                )
                .clicked()
            {
                self.view = view;
                if self.view == View::Compare && !self.comparison_done {
                    self.clear_comparison();
                }
            }
        }
    }

    pub(super) fn explorer(&mut self, ui: &mut egui::Ui, p: Palette) {
        if self.view == View::Compare {
            self.compare_panel(ui, p);
            return;
        }
        let height = ui.available_height();
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(8)
            .show(ui, |ui| {
                let header_top = ui.cursor().top();
                ui.set_min_height((height - 18.0).max(100.0));
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    self.mode_tabs(ui, p);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.syntax_error.is_none() {
                            let reset = button(ui, "Reset", p).on_hover_text(
                                "Reset graph layout. Right-click for canvas size options.",
                            );
                            if reset.clicked() {
                                self.canvas.reset_layout();
                            }
                            reset.context_menu(|ui| {
                                if ui
                                    .button(if self.graph_wide {
                                        "Show source editor"
                                    } else {
                                        "Expand canvas"
                                    })
                                    .clicked()
                                {
                                    self.graph_wide = !self.graph_wide;
                                    ui.close();
                                }
                            });
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        let invalid_path = self
                            .document
                            .as_ref()
                            .is_some_and(|doc| doc.value.pointer(&self.pointer).is_none());
                        #[cfg(target_arch = "wasm32")]
                        let invalid_path = !self.search.pointer_valid;
                        badge(
                            ui,
                            if self.syntax_error.is_some() {
                                "Invalid JSON"
                            } else if self.dirty {
                                "Modified"
                            } else if invalid_path {
                                "Path not found"
                            } else if self.document.is_some() {
                                "Valid JSON"
                            } else {
                                "Awaiting JSON"
                            },
                            if self.syntax_error.is_some() {
                                p.red
                            } else if self.dirty || invalid_path {
                                p.amber
                            } else {
                                p.green
                            },
                            p.subtle,
                        );
                    });
                });
                if let Some(error) = &self.syntax_error {
                    ui.separator();
                    ui.label(RichText::new(error.summary()).color(p.red))
                        .on_hover_text(&error.message);
                    let label = format!(
                        "Go to error · {}",
                        ui.ctx().format_shortcut(&error_shortcut())
                    );
                    if ui.button(label).clicked() {
                        self.jump_to_error(ui.ctx(), false);
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Path").size(12.0).color(p.muted));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.pointer)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("Root · enter /items/0")
                            .desired_width(ui.available_width() - 76.0),
                    )
                    .on_hover_text(
                        "JSON Pointer: /items/0 · escape / as ~1 and ~ as ~0. Empty selects root.",
                    );
                    if ui
                        .add_enabled_ui(true, |ui| {
                            action_button(ui, "Root", p, false, Vec2::new(44.0, 28.0))
                        })
                        .inner
                        .on_hover_text("Back to root")
                        .clicked()
                    {
                        self.pointer.clear();
                        self.canvas.focus(String::new());
                    }
                });
                ui.add_space(2.0);
                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);
                if self.dirty {
                    ui.label(
                        RichText::new("Updating preview after typing…")
                            .size(11.0)
                            .color(p.muted),
                    );
                }
                self.graph_search(ui, p);
                self.align_right_content(ui, header_top);
                let Some(doc) = self.document.clone() else {
                    empty(
                        ui,
                        "Start with a document",
                        "Open a JSON file or paste one into the source editor.",
                        p,
                    );
                    return;
                };
                match self.view {
                    View::Graph => {
                        #[cfg(not(target_arch = "wasm32"))]
                        let pointer_valid = doc.value.pointer(&self.pointer).is_some();
                        #[cfg(target_arch = "wasm32")]
                        let pointer_valid =
                            self.search.pointer_valid && !self.search.pointer_pending;
                        if !self.busy
                            && !self.dirty
                            && pointer_valid
                            && !self.graph.covers_pointer(&self.pointer)
                        {
                            self.start(Job::FocusGraph(doc, self.pointer.clone()), false);
                        }
                        if !self.busy
                            && !self.dirty
                            && let Some(pointer) = self.canvas.edit_requested.take()
                            && let Some(doc) = self.document.clone()
                        {
                            self.submit(Job::Inspect(doc, pointer));
                        }
                        let old_pointer = self.pointer.clone();
                        self.canvas
                            .show(ui, &self.graph, self.dark, &mut self.pointer);
                        if let Some(message) = self.canvas.notice.take() {
                            self.notify(crate::feedback::Kind::Info, message);
                        }
                        if !self.dirty && old_pointer != self.pointer {
                            self.pending_reveal = Some(self.pointer.clone());
                        }
                        if let Some(pointer) = self.canvas.reveal_requested.take() {
                            self.pending_reveal = Some(pointer);
                        }
                    }
                    View::Compare => {}
                }
            });
    }
}
