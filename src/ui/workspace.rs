use super::*;

impl App {
    pub fn workspace(&mut self, ctx: &egui::Context) {
        let p = palette(self.dark);
        if ctx.input(|input| input.pointer.any_pressed())
            && let Some(mouse) = ctx.input(|input| input.pointer.interact_pos())
        {
            let rect = ctx.content_rect();
            if mouse.y > rect.top() + 90.0 {
                self.active_right = (self.graph_wide && self.view == View::Graph)
                    || mouse.x > rect.left() + rect.width() * self.split_ratio;
            }
        }
        self.search_shortcuts(ctx);
        self.error_shortcuts(ctx);
        if (!self.busy || self.automatic_job)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O))
        {
            self.open_file();
        }
        let format_requested = (!self.busy || self.automatic_job)
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)
                    || i.consume_key(egui::Modifiers::CTRL, egui::Key::S)
            });
        if (!self.busy || self.automatic_job)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
        {
            self.parse(Format::Keep);
        }

        egui::TopBottomPanel::top("app_header")
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Reserve the native traffic-light controls inside the shared header.
                    #[cfg(target_os = "macos")]
                    if !ui.input(|input| input.viewport().fullscreen.unwrap_or(false)) {
                        ui.add_space(72.0);
                    }
                    let (logo, _) = ui.allocate_exact_size(
                        egui::vec2(32.0, 32.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(logo, 10, p.tint);
                    let point = |x: f32, y: f32| {
                        logo.min + egui::vec2(x, y) * (logo.width() / 128.0)
                    };
                    let curves = [
                        [(46.0, 28.0), (33.0, 28.0), (34.0, 38.0), (34.0, 48.0)],
                        [(34.0, 48.0), (34.0, 59.0), (27.0, 61.0), (22.0, 64.0)],
                        [(22.0, 64.0), (27.0, 67.0), (34.0, 69.0), (34.0, 80.0)],
                        [(34.0, 80.0), (34.0, 90.0), (33.0, 100.0), (46.0, 100.0)],
                    ];
                    for mirrored in [false, true] {
                        for curve in curves {
                            ui.painter().add(egui::epaint::CubicBezierShape::from_points_stroke(
                                curve.map(|(x, y)| point(if mirrored { 128.0 - x } else { x }, y)),
                                false,
                                egui::Color32::TRANSPARENT,
                                egui::Stroke::new(1.75_f32, p.accent),
                            ));
                        }
                    }
                    for x in [54.0, 64.0, 74.0] {
                        ui.painter().circle_filled(point(x, 64.0), 0.9, p.accent);
                    }
                    ui.label(RichText::new("JSON Lens")
                        .font(FontId::new(24.0, egui::FontFamily::Name("Brand".into()))))
                        .on_hover_text(format!("{}\n{}\n{}", self.filename, self.status, self.gpu_info));
                    repository_link(ui, p);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if self.dark { "Dark" } else { "Light" };
                        let theme = primary(ui, label, p)
                            .on_hover_text("Click to switch Light / Dark. Right-click to select System, Light or Dark.");
                        if theme.clicked() {
                            self.set_theme_preference(ui.ctx(), if self.dark {
                                egui::ThemePreference::Light
                            } else {
                                egui::ThemePreference::Dark
                            });
                        }
                        theme.context_menu(|ui| {
                            for (preference, label) in [
                                (egui::ThemePreference::System, "System"),
                                (egui::ThemePreference::Light, "Light"),
                                (egui::ThemePreference::Dark, "Dark"),
                            ] {
                                if ui.selectable_label(self.theme_preference == preference, label).clicked() {
                                    self.set_theme_preference(ui.ctx(), preference);
                                    ui.close();
                                }
                            }
                        });
                        ui.add_enabled_ui(!self.busy || self.automatic_job, |ui| {
                            if button(ui, "Save as…", p)
                                .on_hover_text("Save JSON")
                                .clicked()
                            {
                                self.save_file();
                            }
                            if button(ui, "Open file", p)
                                .on_hover_text("Open JSON or CSV · ⌘O / Ctrl+O")
                                .clicked()
                            {
                                self.open_file();
                            }
                        });
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let response = ui.interact(
                                ui.available_rect_before_wrap(),
                                ui.id().with("header_drag"),
                                egui::Sense::click_and_drag(),
                            );
                            header_drag(ui, &response);
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.canvas).inner_margin(8))
            .show(ctx, |ui| {
                let area = ui.available_rect_before_wrap();
                if self.graph_wide && self.view == View::Graph {
                    let mut pane = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("right_pane")
                            .max_rect(area)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    self.explorer(&mut pane, p);
                } else {
                    let split = pane_rects(area, self.split_ratio);
                    let handle = ui
                        .interact(
                            split.1,
                            ui.id().with("split_handle"),
                            egui::Sense::click_and_drag(),
                        )
                        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                    if handle.dragged() {
                        if let Some(mouse) = handle.interact_pointer_pos() {
                            self.split_ratio =
                                ((mouse.x - area.left()) / area.width()).clamp(0.3, 0.7);
                        }
                        ui.ctx().request_repaint();
                    }
                    if handle.double_clicked() {
                        self.split_ratio = 0.5;
                    }
                    let color = if handle.hovered() || handle.dragged() {
                        p.accent
                    } else {
                        p.border
                    };
                    ui.painter().line_segment(
                        [
                            split.1.center_top() + Vec2::new(0.0, 8.0),
                            split.1.center_bottom() - Vec2::new(0.0, 8.0),
                        ],
                        Stroke::new(1.0_f32, color),
                    );
                    let mut left = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("left_pane")
                            .max_rect(split.0)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    self.source_panel(&mut left, p);
                    let mut right = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("right_pane")
                            .max_rect(split.2)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    self.explorer(&mut right, p);
                }
            });
        if format_requested {
            if self.active_right && self.view == View::Compare {
                preserve_editor_undo(ctx, "compare_editor", &self.compare_text);
                self.submit(Job::FormatRight(Arc::new(self.compare_text.clone())));
            } else {
                preserve_editor_undo(ctx, "json_source", &self.editor);
                self.parse(Format::Pretty);
            }
        }
        if !self.busy
            && !self.dirty
            && let Some(offset) = self.pending_source_offset.take()
            && let Some(doc) = self.document.clone()
        {
            self.pending_reveal = None;
            if self.view == View::Graph {
                self.start(Job::SourcePointer(doc, offset), false);
            }
        }
        if !self.busy
            && !self.dirty
            && let Some(pointer) = self.pending_reveal.take()
            && let Some(doc) = self.document.clone()
        {
            self.submit(Job::Locate(doc, pointer));
        }
        self.edit_window(ctx, p);
        self.feedback_panel(ctx, p);
    }
}
