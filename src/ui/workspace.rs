use super::*;

impl App {
    pub fn workspace(&mut self, ctx: &egui::Context) {
        let p = palette(self.dark);
        if ctx.input(|input| input.pointer.any_pressed())
            && let Some(mouse) = ctx.input(|input| input.pointer.interact_pos())
        {
            let rect = ctx.content_rect();
            if rect.contains(mouse) && mouse.y > rect.top() + APP_HEADER_HEIGHT {
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
            .exact_height(APP_HEADER_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                let header = ui.max_rect();
                let mut left = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("header_brand")
                        .max_rect(header)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                left.spacing_mut().item_spacing.x = 0.0;
                app_logo(&mut left, p, 26.0);
                left.add_space(12.0);
                brand_title(&mut left, self.dark);
                left.add_space(12.0);
                repository_link(&mut left, p);

                let viewport = ctx.content_rect();
                let mode_center = egui::pos2(
                    if self.graph_wide && self.view == View::Graph {
                        viewport.center().x
                    } else {
                        viewport.left() + viewport.width() * self.split_ratio
                    },
                    header.center().y,
                );
                let mode_rect = egui::Rect::from_center_size(mode_center, Vec2::new(177.0, 28.0));
                let mut center = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("header_mode")
                        .max_rect(mode_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                self.mode_selector(&mut center, p);

                let mut right = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("header_actions")
                        .max_rect(header)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                );
                right.spacing_mut().item_spacing.x = 4.0;
                right.spacing_mut().button_padding = Vec2::ZERO;
                right.spacing_mut().interact_size.y = 24.0;
                right.style_mut().override_font_id = Some(egui::FontId::proportional(12.0));
                right.visuals_mut().override_text_color = Some(p.muted);
                right.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let theme = ui
                        .add(
                            egui::Button::new(
                                RichText::new("\u{f0eb}")
                                    .font(FontId::monospace(15.0))
                                    .color(if self.dark { p.muted } else { p.amber }),
                            )
                            .fill(if self.dark { p.subtle } else { p.tint })
                            .stroke(Stroke::new(1.0_f32, p.border))
                            .corner_radius(12)
                            .min_size(Vec2::splat(24.0)),
                        )
                        .on_hover_text(if self.dark {
                            "Turn the light on"
                        } else {
                            "Turn the light off"
                        });
                    if theme.clicked() {
                        self.set_theme_preference(
                            ui.ctx(),
                            if self.dark {
                                egui::ThemePreference::Light
                            } else {
                                egui::ThemePreference::Dark
                            },
                        );
                    }
                    egui::Frame::new()
                        .fill(p.subtle)
                        .stroke(Stroke::new(1.0, p.border))
                        .corner_radius(7)
                        .inner_margin(1)
                        .show(ui, |ui| self.source_actions(ui, p));
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.surface))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                let full_area = ui.available_rect_before_wrap();
                let mut area = full_area;
                area.max.y = (area.bottom() - 26.0).max(area.top());
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
                        [split.1.center_top(), split.1.center_bottom()],
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
                self.path_footer(ui, full_area, p);
            });
        if format_requested {
            if self.active_right && self.view == View::Compare {
                preserve_editor_undo(ctx, "compare_editor", &self.compare_text);
                self.submit(Job::FormatRight(Arc::new(self.compare_text.clone())));
            } else {
                self.reset_sort_state();
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

impl App {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON / CSV", &["json", "csv", "txt"])
            .pick_file()
        {
            self.filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into();
            self.submit(Job::Open(path));
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn open_file(&mut self) {
        self.files.open();
    }
}

impl App {
    pub(super) fn mode_selector(&mut self, ui: &mut egui::Ui, p: Palette) {
        let was_compare = self.view == View::Compare;
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.spacing_mut().button_padding = Vec2::ZERO;
        ui.spacing_mut().interact_size.y = 24.0;
        ui.style_mut().override_font_id = Some(FontId::proportional(12.0));
        egui::Frame::new()
            .fill(p.subtle)
            .stroke(Stroke::new(1.0_f32, p.border))
            .corner_radius(7)
            .inner_margin(1)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let highlight = ui.painter().add(egui::Shape::Noop);
                    let graph = segmented_button_with_fill(
                        ui,
                        "⎇  Graph",
                        p,
                        self.view == View::Graph,
                        Vec2::new(86.0, 24.0),
                        Color32::TRANSPARENT,
                    )
                    .on_hover_text("Graph mode");
                    if graph.clicked() {
                        self.view = View::Graph;
                    }
                    segment_divider(ui, p, 16.0);
                    let compare = segmented_button_with_fill(
                        ui,
                        "⇄  Compare",
                        p,
                        self.view == View::Compare,
                        Vec2::new(86.0, 24.0),
                        Color32::TRANSPARENT,
                    )
                    .on_hover_text("Compare mode");
                    if compare.clicked() {
                        self.view = View::Compare;
                    }
                    let position = ui.ctx().animate_bool_with_time(
                        ui.id().with("mode_selection"),
                        self.view == View::Compare,
                        0.16,
                    );
                    let seam = (graph.rect.right() + compare.rect.left()) * 0.5;
                    let mut graph_selection = graph.rect;
                    graph_selection.max.x = seam;
                    let mut compare_selection = compare.rect;
                    compare_selection.min.x = seam;
                    let selection = egui::Rect::from_min_max(
                        graph_selection.min.lerp(compare_selection.min, position),
                        graph_selection.max.lerp(compare_selection.max, position),
                    );
                    // Fill to the seam with square inner corners; round only the outer edge.
                    let left_radius = (5.0 * (1.0 - position)).round() as u8;
                    let right_radius = (5.0 * position).round() as u8;
                    let corners = egui::CornerRadius {
                        nw: left_radius,
                        sw: left_radius,
                        ne: right_radius,
                        se: right_radius,
                    };
                    ui.painter().set(
                        highlight,
                        egui::Shape::rect_filled(selection, corners, p.tint),
                    );
                });
            });
        if !was_compare && self.view == View::Compare && !self.comparison_done {
            self.clear_comparison();
        }
    }

    pub(super) fn path_bar(&self, ui: &mut egui::Ui, p: Palette) {
        let pointer = if self.view == View::Compare {
            self.differences
                .get(self.compare_selected)
                .map_or(self.pointer.as_str(), |difference| {
                    difference.pointer.as_str()
                })
        } else {
            &self.pointer
        };
        let mut path = vec!["~".to_owned()];
        path.extend(
            pointer
                .split('/')
                .skip(1)
                .map(|part| part.replace("~1", "/").replace("~0", "~")),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Path").size(12.0).color(p.muted));
            let path = path.join(" > ");
            ui.add(
                egui::Label::new(RichText::new(&path).monospace().size(12.0).color(p.text))
                    .truncate(),
            )
            .on_hover_text(path);
        });
    }

    fn path_footer(&self, ui: &mut egui::Ui, area: egui::Rect, p: Palette) {
        let footer = egui::Rect::from_min_max(
            egui::pos2(area.left(), (area.bottom() - 26.0).max(area.top())),
            area.max,
        );
        ui.painter().rect_filled(footer, 0, p.subtle);
        ui.painter().hline(
            footer.x_range(),
            footer.top() + 0.5,
            Stroke::new(1.0_f32, p.border),
        );
        let wide = self.graph_wide && self.view == View::Graph;
        let (left, divider, right) = pane_rects(footer, self.split_ratio);
        let panes = if wide {
            vec![("wide_path_footer", footer, false)]
        } else {
            ui.painter().vline(
                divider.center().x,
                footer.y_range(),
                Stroke::new(1.0_f32, p.border),
            );
            vec![
                ("left_path_footer", left, false),
                ("right_path_footer", right, self.view == View::Compare),
            ]
        };
        for (id, rect, right_input) in panes {
            let bounds = rect.shrink2(Vec2::new(8.0, 3.0));
            let (status, color) = self.json_status(right_input, p);
            let status_width = ui
                .painter()
                .layout_no_wrap(status.into(), FontId::proportional(11.0), color)
                .size()
                .x
                + 14.0;
            let status_rect = egui::Rect::from_min_max(
                egui::pos2(bounds.right() - status_width, bounds.top()),
                bounds.max,
            );
            let path_rect = egui::Rect::from_min_max(
                bounds.min,
                egui::pos2(
                    (status_rect.left() - 10.0).max(bounds.left()),
                    bounds.bottom(),
                ),
            );
            let mut pane = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(id)
                    .max_rect(path_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            pane.set_clip_rect(rect.intersect(ui.clip_rect()));
            pane.spacing_mut().interact_size.y = 20.0;
            self.path_bar(&mut pane, p);
            let mut status_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((id, "status"))
                    .max_rect(status_rect)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            status_ui.set_clip_rect(rect.intersect(ui.clip_rect()));
            status_ui.spacing_mut().interact_size.y = 20.0;
            badge(&mut status_ui, status, color, p.surface);
        }
    }

    pub(super) fn json_status(&self, right_input: bool, p: Palette) -> (&'static str, Color32) {
        if right_input {
            if self.right_syntax_error.is_some() {
                ("Invalid JSON", p.red)
            } else if self.compare_text.trim().is_empty() {
                ("Awaiting JSON", p.muted)
            } else if self.right_json_valid {
                ("Valid JSON", p.green)
            } else {
                ("Checking JSON", p.amber)
            }
        } else if self.syntax_error.is_some() {
            ("Invalid JSON", p.red)
        } else if self.editor.trim().is_empty() && self.document.is_none() {
            ("Awaiting JSON", p.muted)
        } else if self.dirty {
            ("Modified", p.amber)
        } else if self.document.is_some() {
            ("Valid JSON", p.green)
        } else {
            ("Awaiting JSON", p.muted)
        }
    }

    pub(super) fn path_separator(&self, ui: &mut egui::Ui, p: Palette, right_pane: bool) {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
        let viewport = ui.ctx().content_rect();
        let (left, right) = if self.graph_wide && self.view == View::Graph {
            (viewport.left(), viewport.right())
        } else if right_pane {
            (rect.left() - PANE_GAP * 0.5, viewport.right())
        } else {
            (viewport.left(), rect.right() + PANE_GAP * 0.5)
        };
        // These borders span the pane padding and half the splitter gap.
        // Use the viewport clip so nested UI clipping cannot trim their ends.
        let mut painter = ui.painter().clone();
        painter.set_clip_rect(viewport);
        painter.hline(
            left..=right,
            rect.center().y,
            Stroke::new(1.0_f32, p.border),
        );
    }

    pub(super) fn explorer(&mut self, ui: &mut egui::Ui, p: Palette) {
        if self.view == View::Compare {
            self.compare_panel(ui, p);
            return;
        }
        let height = ui.available_height();
        egui::Frame::new().fill(p.surface).show(ui, |ui| {
            let header_top = ui.cursor().top();
            ui.set_min_height(height.max(100.0));
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().button_padding = Vec2::ZERO;
            if let Some(error) = &self.syntax_error {
                self.path_separator(ui, p, true);
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
            self.path_separator(ui, p, true);
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
                    let pointer_valid = self.search.pointer_valid && !self.search.pointer_pending;
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
