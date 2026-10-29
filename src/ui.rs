use super::{App, Job, View};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use json_lens::{Format, preview};
use std::sync::Arc;

pub struct EditDialog {
    pub pointer: String,
    pub json: String,
    pub key: Option<String>,
    pub rename: bool,
    pub error: String,
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Maple Mono CN".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/MapleMono-NF-CN-Regular.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "Maple Mono CN".into());
    fonts.font_data.insert(
        "Inter".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Inter.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".into());
    // Inter has no CJK glyphs; keep Maple as a fallback for Chinese text.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("Maple Mono CN".into());
    fonts.font_data.insert(
        "Bungee".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Bungee-Regular.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name("Brand".into()),
        vec!["Bungee".into(), "Inter".into()],
    );
    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy)]
struct Palette {
    canvas: Color32,
    surface: Color32,
    subtle: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    tint: Color32,
    green: Color32,
    amber: Color32,
    red: Color32,
}

fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::from_rgb(19, 21, 29),
            surface: Color32::from_rgb(26, 29, 39),
            subtle: Color32::from_rgb(33, 37, 49),
            border: Color32::from_rgb(47, 52, 68),
            text: Color32::from_rgb(222, 226, 237),
            muted: Color32::from_rgb(143, 153, 174),
            accent: Color32::from_rgb(167, 151, 255),
            tint: Color32::from_rgb(48, 40, 77),
            green: Color32::from_rgb(118, 209, 172),
            amber: Color32::from_rgb(231, 181, 113),
            red: Color32::from_rgb(246, 143, 157),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(244, 245, 249),
            surface: Color32::WHITE,
            subtle: Color32::from_rgb(248, 249, 252),
            border: Color32::from_rgb(228, 231, 239),
            text: Color32::from_rgb(42, 48, 65),
            muted: Color32::from_rgb(126, 136, 155),
            accent: Color32::from_rgb(110, 83, 218),
            tint: Color32::from_rgb(239, 235, 253),
            green: Color32::from_rgb(38, 133, 103),
            amber: Color32::from_rgb(167, 112, 41),
            red: Color32::from_rgb(195, 66, 92),
        }
    }
}

pub fn configure(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    let p = palette(dark);
    let mut style = egui::Style {
        visuals: if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        },
        ..Default::default()
    };
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(23.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(13.0));
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
    style.spacing.interact_size = Vec2::new(32.0, 32.0);
    style.spacing.indent = 22.0;
    style.visuals.override_text_color = Some(p.text);
    style.visuals.panel_fill = p.canvas;
    style.visuals.window_fill = p.surface;
    style.visuals.extreme_bg_color = p.subtle;
    style.visuals.faint_bg_color = p.subtle;
    style.visuals.selection.bg_fill = p.tint;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.muted);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 7.into();
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0_f32, p.text);
    }
    style.visuals.widgets.inactive.weak_bg_fill = p.surface;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
    style.visuals.widgets.inactive.bg_fill = p.subtle;
    style.visuals.widgets.hovered.weak_bg_fill = p.tint;
    style.visuals.widgets.hovered.bg_fill = p.tint;
    style.visuals.widgets.active.weak_bg_fill = p.tint.lerp_to_gamma(p.accent, 0.25);
    style.visuals.widgets.active.bg_fill = p.tint.lerp_to_gamma(p.accent, 0.25);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.accent);
    ctx.set_style(style);
}

fn action_button(
    ui: &mut egui::Ui,
    text: &str,
    p: Palette,
    primary: bool,
    size: Vec2,
) -> egui::Response {
    action_button_with_radius(ui, text, p, primary, size, 7.into())
}

fn action_button_with_radius(
    ui: &mut egui::Ui,
    text: &str,
    p: Palette,
    primary: bool,
    size: Vec2,
    radius: egui::CornerRadius,
) -> egui::Response {
    let response = ui
        .push_id(text, |ui| {
            ui.scope(|ui| {
                let base = if primary {
                    if ui.visuals().dark_mode {
                        Color32::from_rgb(123, 98, 222)
                    } else {
                        p.accent
                    }
                } else {
                    p.surface
                };
                let hover = if primary {
                    base.lerp_to_gamma(Color32::WHITE, 0.14)
                } else {
                    p.tint
                };
                let pressed = if primary {
                    base.gamma_multiply(0.76)
                } else {
                    p.tint.lerp_to_gamma(p.accent, 0.22)
                };
                let style = ui.style_mut();
                style.visuals.widgets.inactive.weak_bg_fill = base;
                style.visuals.widgets.inactive.bg_stroke = if primary {
                    Stroke::NONE
                } else {
                    Stroke::new(1.0_f32, p.border)
                };
                style.visuals.widgets.hovered.weak_bg_fill = hover;
                style.visuals.widgets.hovered.bg_stroke =
                    Stroke::new(1.5_f32, if primary { hover } else { p.accent });
                style.visuals.widgets.active.weak_bg_fill = pressed;
                style.visuals.widgets.active.bg_stroke =
                    Stroke::new(2.0_f32, if primary { pressed } else { p.accent });
                style.visuals.widgets.active.expansion = -1.0;
                ui.add(
                    egui::Button::new(RichText::new(text).color(if primary {
                        Color32::WHITE
                    } else {
                        p.text
                    }))
                    .corner_radius(radius)
                    .min_size(size),
                )
            })
            .inner
        })
        .inner
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let id = response.id.with("click_flash");
    let now = ui.input(|input| input.time);
    if response.clicked() {
        ui.ctx().data_mut(|data| data.insert_temp(id, now));
        ui.ctx().request_repaint();
    }
    let elapsed = ui
        .ctx()
        .data(|data| data.get_temp::<f64>(id))
        .map(|time| now - time);
    if let Some(elapsed) = elapsed
        && elapsed < 0.4
    {
        let color = if primary { Color32::WHITE } else { p.accent };
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            radius,
            Stroke::new(
                2.0_f32,
                color.gamma_multiply((1.0 - elapsed as f32 / 0.4).max(0.0)),
            ),
            egui::StrokeKind::Inside,
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
    response
}

fn button(ui: &mut egui::Ui, text: &str, p: Palette) -> egui::Response {
    action_button(ui, text, p, false, Vec2::new(76.0, 34.0))
}
fn primary(ui: &mut egui::Ui, text: &str, p: Palette) -> egui::Response {
    action_button(ui, text, p, true, Vec2::new(80.0, 34.0))
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32, fill: Color32) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(color));
        });
}

impl App {
    fn feedback_panel(&mut self, ctx: &egui::Context, p: Palette) {
        if self
            .feedback
            .as_ref()
            .is_some_and(|feedback| feedback.expired())
        {
            self.feedback = None;
        }
        let Some(feedback) = &self.feedback else {
            return;
        };
        let color = match feedback.kind {
            crate::feedback::Kind::Working => p.accent,
            crate::feedback::Kind::Success => p.green,
            crate::feedback::Kind::Info => p.accent,
            crate::feedback::Kind::Error => p.red,
        };
        let mut dismiss = false;
        egui::Area::new(egui::Id::new("operation_feedback"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-24.0, 98.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(p.surface)
                    .stroke(Stroke::new(1.5_f32, color))
                    .corner_radius(10)
                    .inner_margin(14)
                    .show(ui, |ui| {
                        ui.set_max_width(380.0);
                        ui.horizontal_top(|ui| {
                            if feedback.kind == crate::feedback::Kind::Working {
                                ui.spinner();
                            } else {
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                                ui.painter().circle_filled(rect.center(), 4.0, color);
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&feedback.text).color(p.text).size(13.0),
                                )
                                .wrap(),
                            );
                            dismiss = ui.button("×").on_hover_text("Dismiss message").clicked();
                        });
                    });
            });
        if dismiss {
            self.feedback = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
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

    fn text_search(&mut self, ui: &mut egui::Ui, p: Palette) {
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

    fn graph_search(&mut self, ui: &mut egui::Ui, p: Palette) {
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

    fn right_search(&mut self, ui: &mut egui::Ui, p: Palette) {
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
        let new_press = ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::F,
                        pressed: true,
                        repeat: false,
                        ..
                    }
                )
            })
        });
        let shifted = ctx.input_mut(|input| {
            input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::F,
            ) || input.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::F)
        });
        let find = ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::COMMAND, egui::Key::F)
                || input.consume_key(egui::Modifiers::CTRL, egui::Key::F)
        });
        if shifted && new_press {
            self.active_right = true;
            self.view = View::Graph;
            self.search.tree.toggle();
        } else if find && new_press {
            if !self.active_right {
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
        if (!self.busy || self.automatic_job)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O))
        {
            self.open_file();
        }
        if (!self.busy || self.automatic_job)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
        {
            self.save_file();
        }
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
                                egui::Stroke::new(1.75, p.accent),
                            ));
                        }
                    }
                    for x in [54.0, 64.0, 74.0] {
                        ui.painter().circle_filled(point(x, 64.0), 0.9, p.accent);
                    }
                    ui.label(RichText::new("JSON Lens")
                        .font(FontId::new(24.0, egui::FontFamily::Name("Brand".into()))))
                        .on_hover_text(format!("{}\n{}\n{}", self.filename, self.status, self.gpu_info));
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
                                .on_hover_text("Save JSON · ⌘S / Ctrl+S")
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

    fn edit_window(&mut self, ctx: &egui::Context, p: Palette) {
        let Some(dialog) = &mut self.edit_dialog else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut copied = false;
        egui::Window::new("Edit JSON node")
            .id(egui::Id::new("edit_node"))
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(if dialog.pointer.is_empty() {
                        "root"
                    } else {
                        &dialog.pointer
                    })
                    .monospace()
                    .color(p.accent),
                );
                if dialog.key.is_some() {
                    ui.checkbox(&mut dialog.rename, "Rename property");
                }
                ui.add_enabled_ui(!self.busy || self.automatic_job, |ui| {
                    if dialog.rename {
                        if let Some(key) = &mut dialog.key {
                            ui.text_edit_singleline(key);
                        }
                    } else {
                        ui.label("Enter a JSON value. String values need double quotes.");
                        egui::ScrollArea::both().max_height(320.0).show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut dialog.json)
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(12),
                            );
                        });
                    }
                    if !dialog.error.is_empty() {
                        ui.colored_label(p.red, &dialog.error);
                    }
                    ui.horizontal(|ui| {
                        save = primary(ui, "Apply", p).clicked();
                        if button(ui, "Copy JSON", p).clicked() {
                            ui.ctx().copy_text(dialog.json.clone());
                            copied = true;
                            dialog.error.clear();
                        }
                    });
                });
            });
        if copied {
            self.notify(crate::feedback::Kind::Success, "JSON copied to clipboard");
        }
        if save {
            let dialog = self.edit_dialog.as_ref().unwrap();
            if let Some(doc) = self.document.clone() {
                let text = if dialog.rename {
                    dialog.key.clone().unwrap_or_default()
                } else {
                    dialog.json.clone()
                };
                self.submit(Job::Edit(doc, dialog.pointer.clone(), text, dialog.rename));
            }
        }
        if !open {
            self.edit_dialog = None;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_file(&mut self) {
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

    #[cfg(not(target_arch = "wasm32"))]
    fn save_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&self.filename)
            .save_file()
        {
            let text = self.source();
            self.submit(Job::Save(text, path));
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn open_file(&mut self) {
        self.files.open();
    }

    #[cfg(target_arch = "wasm32")]
    fn save_file(&mut self) {
        let text = self.source();
        self.files
            .save(Arc::new(text.as_bytes().to_vec()), self.filename.clone());
    }

    fn next_difference(&mut self, backwards: bool) {
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
    fn header_height(&self) -> f32 {
        if self.view == View::Compare {
            self.source_header_height.max(self.right_header_height)
        } else {
            self.source_header_height
        }
    }

    fn align_right_content(&mut self, ui: &mut egui::Ui, top: f32) {
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

    fn source_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
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
            .stroke(Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(8)
            .show(ui, |ui| {
                let header_top = ui.cursor().top();
                ui.set_min_height((height - 18.0).max(100.0));
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(!self.busy || self.automatic_job, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if primary(ui, "Format", p).clicked() {
                                self.parse(Format::Pretty);
                            }
                            if button(ui, "Minify", p).clicked() {
                                self.parse(Format::Compact);
                            }
                            if button(ui, "Expand", p)
                                .on_hover_text("Expand JSON encoded in strings")
                                .clicked()
                            {
                                self.parse(Format::Nested);
                            }
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                if action_button_with_radius(
                                    ui,
                                    "Sort",
                                    p,
                                    false,
                                    Vec2::new(76.0, 34.0),
                                    egui::CornerRadius {
                                        nw: 7,
                                        ne: 0,
                                        sw: 7,
                                        se: 0,
                                    },
                                )
                                .on_hover_text(if self.sort_descending {
                                    "Sort keys descending"
                                } else {
                                    "Sort keys ascending"
                                })
                                .clicked()
                                {
                                    self.parse(if self.sort_descending {
                                        Format::SortDescending
                                    } else {
                                        Format::SortAscending
                                    });
                                }
                                let direction = if self.sort_descending { "↓" } else { "↑" };
                                if action_button_with_radius(
                                    ui,
                                    direction,
                                    p,
                                    false,
                                    Vec2::new(30.0, 34.0),
                                    egui::CornerRadius {
                                        nw: 0,
                                        ne: 7,
                                        sw: 0,
                                        se: 7,
                                    },
                                )
                                .on_hover_text("Toggle ascending / descending order")
                                .clicked()
                                {
                                    self.sort_descending = !self.sort_descending;
                                    self.notify(
                                        crate::feedback::Kind::Info,
                                        if self.sort_descending {
                                            "Sort order: descending"
                                        } else {
                                            "Sort order: ascending"
                                        },
                                    );
                                }
                            });
                        });
                    });
                });
                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);
                self.text_search(ui, p);
                if self.search.jump {
                    self.prepare_search_jump();
                }
                ui.add_space(2.0);
                if !self.editable {
                    egui::Frame::new()
                        .fill(p.tint)
                        .corner_radius(6)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(if self.view == View::Compare {
                                    "Large document · full source shown for comparison"
                                } else {
                                    "Large document · previewing the first 16 KiB"
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
                        alignment: alignment.as_deref(),
                    },
                    |ui| {
                        let wrap_width = if self.view == View::Compare {
                            (ui.ctx().content_rect().width()
                                * self.split_ratio.min(1.0 - self.split_ratio)
                                - 128.0)
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
                                aligned_wrapped_galley(
                                    ui,
                                    galley,
                                    alignment.as_deref(),
                                    true,
                                    &self.compare_text,
                                    wrap_width,
                                )
                            };
                        let mut output = egui::TextEdit::multiline(&mut self.editor)
                            .id_salt("json_source")
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .frame(false)
                            .margin(Vec2::ZERO)
                            .interactive(editable)
                            .desired_width(ui.available_width())
                            .desired_rows(1)
                            .min_size(Vec2::new(0.0, editor_height))
                            .layouter(&mut layouter)
                            .show(ui);
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
                        if self.view == View::Graph
                            && (output.response.clicked()
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
                        output
                    },
                );
                if self.view == View::Compare {
                    self.compare_scroll = scroll.state.offset.y;
                }
            });
    }

    fn mode_tabs(&mut self, ui: &mut egui::Ui, p: Palette) {
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

    fn explorer(&mut self, ui: &mut egui::Ui, p: Palette) {
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
                        badge(
                            ui,
                            if self.dirty {
                                "Modified"
                            } else if self.document.is_some() {
                                "Valid JSON"
                            } else {
                                "Awaiting JSON"
                            },
                            if self.dirty { p.amber } else { p.green },
                            p.subtle,
                        );
                    });
                });
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
                if doc.value.pointer(&self.pointer).is_none() {
                    empty(
                        ui,
                        "Nothing at this path",
                        "Check the JSON Pointer or return to the root.",
                        p,
                    );
                    return;
                };
                match self.view {
                    View::Graph => {
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

    fn compare_panel(&mut self, ui: &mut egui::Ui, p: Palette) {
        let height = ui.available_height();
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(8)
            .show(ui, |ui| {
                let top = ui.cursor().top();
                ui.set_min_height((height - 18.0).max(100.0));
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    self.mode_tabs(ui, p);
                    if !self.compare_text.trim().is_empty() || self.comparison_done {
                        let count = self.differences.len();
                        let message = if !self.comparison_done {
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
                        let summary = ui.label(RichText::new(message).size(12.0).color(p.muted));
                        if let Some(diff) = self.differences.get(self.compare_selected) {
                            summary.on_hover_text(format!(
                                "{}\n− {}\n+ {}",
                                diff.pointer, diff.left, diff.right
                            ));
                        }
                        if ui
                            .add_enabled(count > 0 && !self.busy, egui::Button::new("<"))
                            .on_hover_text("Previous difference")
                            .clicked()
                        {
                            self.next_difference(true);
                        }
                        if ui
                            .add_enabled(count > 0 && !self.busy, egui::Button::new(">"))
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
                ui.add_space(2.0);
                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);
                self.right_search(ui, p);
                self.align_right_content(ui, top);
                let alignment = self.compare_alignment.clone();
                let height = ui.available_height().max(0.0);
                let offset = self.compare_scroll;
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
                        alignment: alignment.as_deref(),
                    },
                    |ui| {
                        let wrap_width = if self.view == View::Compare {
                            (ui.ctx().content_rect().width()
                                * self.split_ratio.min(1.0 - self.split_ratio)
                                - 128.0)
                                .max(80.0)
                        } else {
                            ui.available_width().max(80.0)
                        };
                        let mut layouter =
                            |ui: &egui::Ui, text: &dyn egui::TextBuffer, _width: f32| {
                                let galley = self.compare_code.layout_width(
                                    ui,
                                    text.as_str(),
                                    self.dark,
                                    &self.search.right.query,
                                    wrap_width,
                                );
                                aligned_wrapped_galley(
                                    ui,
                                    galley,
                                    alignment.as_deref(),
                                    false,
                                    &self.editor,
                                    wrap_width,
                                )
                            };
                        let output = egui::TextEdit::multiline(&mut self.compare_text)
                            .id_salt("compare_editor")
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .frame(false)
                            .margin(Vec2::ZERO)
                            .interactive(!self.busy || self.automatic_job)
                            .desired_width(ui.available_width())
                            .desired_rows(1)
                            .min_size(Vec2::new(0.0, height))
                            .hint_text("Paste JSON here…")
                            .layouter(&mut layouter)
                            .show(ui);
                        decoded_hover(ui, &output, &self.compare_text);
                        if output.response.changed() {
                            self.search.right.changed();
                            self.revision += 1;
                            self.clear_comparison();
                            if self.dirty {
                                self.auto_due = Some(
                                    web_time::Instant::now()
                                        + std::time::Duration::from_millis(500),
                                );
                            }
                        }
                        output.response.context_menu(|ui| {
                            ui.add_enabled_ui(!self.busy || self.automatic_job, |ui| {
                                if ui.button("Format").clicked() {
                                    self.submit(Job::FormatRight(Arc::new(
                                        self.compare_text.clone(),
                                    )));
                                    ui.close();
                                }
                                if ui.button("Clear").clicked() {
                                    self.compare_text.clear();
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
                            highlight_difference(
                                ui,
                                &output,
                                &self.compare_text,
                                range,
                                0,
                                p.green,
                            );
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

fn decoded_hover(ui: &egui::Ui, output: &egui::text_edit::TextEditOutput, text: &str) {
    let value = if output.response.hovered() {
        ui.input(|input| input.pointer.hover_pos())
            .and_then(|mouse| {
                let cursor = output.galley.cursor_from_pos(mouse - output.galley_pos);
                let rect = output
                    .galley
                    .pos_from_cursor(cursor)
                    .translate(output.galley_pos.to_vec2());
                if (rect.min.x - mouse.x).abs() > 12.0 || (rect.center().y - mouse.y).abs() > 15.0 {
                    return None;
                }
                let offset = text
                    .char_indices()
                    .nth(cursor.index)
                    .map_or(text.len(), |(offset, _)| offset);
                json_lens::decode::string_at(text, offset)
            })
    } else {
        None
    };
    crate::decode_ui::show(&output.response, value.as_deref());
}

fn aligned_wrapped_galley(
    ui: &egui::Ui,
    galley: Arc<egui::Galley>,
    alignment: Option<&json_lens::alignment::Alignment>,
    left: bool,
    other: &str,
    width: f32,
) -> Arc<egui::Galley> {
    if !width.is_finite() {
        return aligned_galley(ui, galley, alignment, left);
    }
    let Some(alignment) = alignment else {
        return galley;
    };
    let (side, other_side) = if left {
        (&alignment.left, &alignment.right)
    } else {
        (&alignment.right, &alignment.left)
    };
    if side.source != galley.job.text || other_side.source != other {
        return galley;
    }
    let mut job =
        egui::text::LayoutJob::simple(other.into(), FontId::monospace(13.0), Color32::WHITE, width);
    job.wrap.break_anywhere = galley.job.wrap.break_anywhere;
    let opposite = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let mut heights = vec![0.0_f32; alignment.height];
    for (layout, side) in [(&*galley, side), (&*opposite, other_side)] {
        let mut line = 0;
        let mut height = 0.0;
        for row in &layout.rows {
            height += row.size.y;
            if row.ends_with_newline {
                if let Some(display) = side.rows.get(line) {
                    heights[*display] = heights[*display].max(height);
                }
                line += 1;
                height = 0.0;
            }
        }
        if let Some(display) = side.rows.get(line) {
            heights[*display] = heights[*display].max(height);
        }
    }
    let mut tops = vec![0.0; heights.len()];
    let mut total = 0.0;
    for (top, height) in tops.iter_mut().zip(heights) {
        *top = total;
        total += height;
    }
    let mut result = (*galley).clone();
    result.mesh_bounds = egui::Rect::NOTHING;
    let mut line = 0;
    let mut within = 0.0;
    for row in &mut result.rows {
        let Some(display) = side.rows.get(line) else {
            return galley;
        };
        row.pos.y = tops[*display] + within;
        result.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
        within += row.size.y;
        if row.ends_with_newline {
            line += 1;
            within = 0.0;
        }
    }
    result.rect.max.y = total;
    Arc::new(result)
}

fn aligned_galley(
    _ui: &egui::Ui,
    galley: Arc<egui::Galley>,
    alignment: Option<&json_lens::alignment::Alignment>,
    left: bool,
) -> Arc<egui::Galley> {
    let Some(alignment) = alignment else {
        return galley;
    };
    let side = if left {
        &alignment.left
    } else {
        &alignment.right
    };
    if side.source != galley.job.text || side.rows.len() != galley.rows.len() {
        return galley;
    }
    if alignment.height == galley.rows.len()
        && side
            .rows
            .iter()
            .enumerate()
            .all(|(index, row)| index == *row)
    {
        return galley;
    }
    // Galley rows already have pixel-rounded heights. Reusing raw font metrics here
    // changes the spacing cumulatively, especially at fractional display scales.
    let line_height = galley.rows.first().map_or(0.0, |row| row.size.y);
    let mut aligned = (*galley).clone();
    aligned.mesh_bounds = egui::Rect::NOTHING;
    for (row, display) in aligned.rows.iter_mut().zip(&side.rows) {
        row.pos.y = (*display as f32 * line_height * aligned.pixels_per_point).round()
            / aligned.pixels_per_point;
        aligned.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
    }
    aligned.rect.max.y = alignment.height as f32 * line_height;
    Arc::new(aligned)
}

struct EditorViewport<'a> {
    salt: &'static str,
    wrap: bool,
    height: f32,
    offset: Option<f32>,
    palette: Palette,
    left: bool,
    first_line: usize,
    alignment: Option<&'a json_lens::alignment::Alignment>,
}

fn editor_scroll(
    ui: &mut egui::Ui,
    view: EditorViewport<'_>,
    contents: impl FnOnce(&mut egui::Ui) -> egui::text_edit::TextEditOutput,
) -> egui::scroll_area::ScrollAreaOutput<egui::text_edit::TextEditOutput> {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let gutter = ui
            .allocate_exact_size(Vec2::new(64.0, view.height), egui::Sense::hover())
            .0;
        let mut area = (if view.wrap {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        })
        .id_salt(view.salt)
        .auto_shrink([false, false])
        .max_height(view.height);
        if let Some(offset) = view.offset {
            area = area
                .vertical_scroll_offset(offset)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);
        }
        let scroll = area.show(ui, contents);
        let output = &scroll.inner;
        let gutter = egui::Rect::from_min_max(
            egui::pos2(gutter.left(), scroll.inner_rect.top()),
            egui::pos2(gutter.right(), scroll.inner_rect.bottom()),
        );
        let painter = ui.painter().with_clip_rect(gutter);
        let p = view.palette;
        painter.rect_filled(gutter, 8, p.subtle);
        let side = view
            .alignment
            .map(|alignment| {
                if view.left {
                    &alignment.left
                } else {
                    &alignment.right
                }
            })
            .filter(|side| side.source == output.galley.job.text);
        let color = if view.left { p.red } else { p.green };
        let text_painter = ui.painter().with_clip_rect(scroll.inner_rect);
        let mut line = 0;
        let mut first_visual_row = true;
        for row in &output.galley.rows {
            let index = line;
            let number = first_visual_row;
            first_visual_row = row.ends_with_newline;
            line += usize::from(row.ends_with_newline);
            let y = output.galley_pos.y + row.pos.y;
            if y + row.size.y < gutter.top() || y > gutter.bottom() {
                continue;
            }
            let changed = side.is_some_and(|side| side.changed.get(index) == Some(&true));
            if changed {
                let marker = egui::Rect::from_min_size(
                    egui::pos2(gutter.left() + 4.0, y),
                    Vec2::new(3.0, row.size.y),
                )
                .intersect(gutter.shrink2(Vec2::new(0.0, 8.0)));
                if marker.is_positive() {
                    painter.rect_filled(marker, 2, color);
                }
                text_painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(scroll.inner_rect.left(), y),
                        Vec2::new(scroll.inner_rect.width(), row.size.y),
                    ),
                    0,
                    color.gamma_multiply(0.06),
                );
            }
            if number {
                painter.text(
                    egui::pos2(gutter.right() - 8.0, y),
                    egui::Align2::RIGHT_TOP,
                    (index + view.first_line).to_string(),
                    FontId::monospace(13.0),
                    if changed { color } else { p.muted },
                );
            }
        }
        scroll
    })
    .inner
}

fn pane_rects(area: egui::Rect, ratio: f32) -> (egui::Rect, egui::Rect, egui::Rect) {
    let middle = area.left() + area.width() * ratio;
    let gap = 16.0;
    (
        egui::Rect::from_min_max(area.min, egui::pos2(middle - gap * 0.5, area.bottom())),
        egui::Rect::from_min_max(
            egui::pos2(middle - gap * 0.5, area.top()),
            egui::pos2(middle + gap * 0.5, area.bottom()),
        ),
        egui::Rect::from_min_max(egui::pos2(middle + gap * 0.5, area.top()), area.max),
    )
}

fn highlight_difference(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    text: &str,
    range: &std::ops::Range<usize>,
    base: usize,
    color: Color32,
) {
    if range.start < base || range.end > base + text.len() {
        return;
    }
    let start = text[..range.start - base].chars().count();
    let end = start + text[range.start - base..range.end - base].chars().count();
    let painter = ui.painter().with_clip_rect(output.text_clip_rect);
    let mut offset = 0;
    for row in &output.galley.rows {
        let row_end = offset + row.glyphs.len();
        if start <= row_end && end > offset {
            let a = output
                .galley
                .pos_from_cursor(egui::text::CCursor::new(start.max(offset)));
            let b = output
                .galley
                .pos_from_cursor(egui::text::CCursor::new(end.min(row_end)));
            let rect = egui::Rect::from_min_max(
                egui::pos2(a.min.x, row.pos.y),
                egui::pos2(b.min.x.max(a.min.x + 3.0), row.pos.y + row.size.y),
            )
            .translate(output.galley_pos.to_vec2());
            painter.rect_filled(rect, 2, color.gamma_multiply(0.12));
            painter.line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.5_f32, color),
            );
        }
        offset = row_end + usize::from(row.ends_with_newline);
    }
}

fn align_content(ui: &mut egui::Ui, top: f32, height: f32) {
    ui.add_space((top + height - ui.cursor().top()).max(0.0));
}

fn empty(ui: &mut egui::Ui, title: &str, description: &str, p: Palette) {
    ui.add_space(32.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("{ · }")
                .size(32.0)
                .monospace()
                .color(p.accent),
        );
        ui.add_space(12.0);
        ui.label(RichText::new(title).size(17.0).strong());
        ui.add_space(4.0);
        ui.label(RichText::new(description).size(13.0).color(p.muted));
    });
    ui.add_space(24.0);
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

fn match_count(selected: usize, stored: usize, total: usize) -> String {
    if total == 0 {
        "0 matches".into()
    } else if total > stored {
        format!("{}/{} · {stored} shown", selected + 1, total)
    } else {
        format!("{}/{}", selected + 1, total)
    }
}

#[derive(Default)]
pub struct CodeCache {
    text: String,
    query: String,
    dark: bool,
    pixels_per_point: f32,
    job: Option<egui::text::LayoutJob>,
}

impl CodeCache {
    fn layout_width(
        &mut self,
        ui: &egui::Ui,
        text: &str,
        dark: bool,
        query: &str,
        width: f32,
    ) -> Arc<egui::Galley> {
        let normal = self.layout(ui, text, dark, query);
        if !width.is_finite() {
            return normal;
        }
        let mut job = (*normal.job).clone();
        job.wrap.max_width = width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    }
    fn layout(&mut self, ui: &egui::Ui, text: &str, dark: bool, query: &str) -> Arc<egui::Galley> {
        if self.text == text
            && self.query == query
            && self.dark == dark
            && self.pixels_per_point == ui.ctx().pixels_per_point()
            && let Some(job) = &self.job
        {
            // Resolve through egui each pass: theme changes and atlas pressure invalidate glyph UVs.
            return ui.fonts_mut(|fonts| fonts.layout_job(job.clone()));
        }
        let p = palette(dark);
        let mut job = egui::text::LayoutJob::default();
        if text.is_empty() {
            job.append(
                "",
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(13.0),
                    color: p.text,
                    ..Default::default()
                },
            );
        }
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let start = index;
            let color = match bytes[index] {
                b'"' => {
                    index += 1;
                    while index < bytes.len() {
                        match bytes[index] {
                            b'\\' => index = (index + 2).min(bytes.len()),
                            b'"' => {
                                index += 1;
                                break;
                            }
                            _ => index += 1,
                        }
                    }
                    let next = text[index..].trim_start().as_bytes().first().copied();
                    if next == Some(b':') {
                        p.accent
                    } else {
                        p.green
                    }
                }
                b'-' | b'0'..=b'9' => {
                    index += 1;
                    while index < bytes.len()
                        && matches!(bytes[index], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
                    {
                        index += 1;
                    }
                    p.amber
                }
                b't' | b'f' | b'n' => {
                    index += 1;
                    while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
                        index += 1;
                    }
                    p.accent
                }
                _ => {
                    index += text[index..].chars().next().unwrap().len_utf8();
                    p.text
                }
            };
            // Invalid Unicode escape sequences remain editable; never split a UTF-8 codepoint.
            while index < text.len() && !text.is_char_boundary(index) {
                index += 1;
            }
            job.append(
                &text[start..index],
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(13.0),
                    color,
                    ..Default::default()
                },
            );
        }
        job.wrap.max_width = f32::INFINITY;
        let matches = json_lens::search::find_text(text, query);
        if !matches.ranges.is_empty() {
            let sections = std::mem::take(&mut job.sections);
            let mut match_index = 0;
            for section in sections {
                let mut start = section.byte_range.start;
                let end = section.byte_range.end;
                while start < end {
                    while match_index < matches.ranges.len()
                        && matches.ranges[match_index].end <= start
                    {
                        match_index += 1;
                    }
                    let found = matches.ranges.get(match_index);
                    let highlighted =
                        found.is_some_and(|range| range.start <= start && start < range.end);
                    let next = found.map_or(end, |range| {
                        if highlighted {
                            range.end.min(end)
                        } else {
                            range.start.min(end)
                        }
                    });
                    let mut format = section.format.clone();
                    if highlighted {
                        format.background = p.tint;
                        format.underline = Stroke::new(1.0_f32, p.accent);
                    }
                    job.sections.push(egui::text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: start..next,
                        format,
                    });
                    start = next;
                }
            }
        }
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job.clone()));
        self.text = text.to_owned();
        self.query = query.to_owned();
        self.dark = dark;
        self.pixels_per_point = ui.ctx().pixels_per_point();
        self.job = Some(job);
        galley
    }
}

#[cfg(test)]
mod feedback_tests {
    use super::*;

    #[test]
    fn wrapped_comparison_keeps_following_lines_aligned_and_cursor_offsets_intact() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        configure(&ctx, false);
        let left = format!(
            "{{\n  \"value\": \"{}\",\n  \"last\": true\n}}",
            "中文 long value ".repeat(12)
        );
        let right = "{\n  \"value\": \"short\",\n  \"last\": true\n}";
        let alignment = json_lens::alignment::align(&left, right);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let a = CodeCache::default().layout_width(ui, &left, false, "", 180.0);
                let b = CodeCache::default().layout_width(ui, right, false, "", 180.0);
                assert!(a.rows.len() > left.lines().count());
                let a = aligned_wrapped_galley(ui, a, Some(&alignment), true, right, 180.0);
                let b = aligned_wrapped_galley(ui, b, Some(&alignment), false, &left, 180.0);
                let a_index = left[..left.find("\"last\"").unwrap()].chars().count();
                let b_index = right[..right.find("\"last\"").unwrap()].chars().count();
                let a_cursor = a.pos_from_cursor(egui::text::CCursor::new(a_index));
                let b_cursor = b.pos_from_cursor(egui::text::CCursor::new(b_index));
                assert_eq!(a_cursor.top(), b_cursor.top());
                assert_eq!(a.rect.height(), b.rect.height());
                assert_eq!(
                    a.cursor_from_pos(a_cursor.center().to_vec2()).index,
                    a_index
                );
                assert_eq!(a.job.text, left);
                assert_eq!(a.rows.iter().filter(|row| row.ends_with_newline).count(), 3);
            });
        });
    }

    #[test]
    fn equal_split_has_mirrored_pane_dimensions() {
        for size in [Vec2::new(1240.0, 660.0), Vec2::new(760.0, 460.0)] {
            let area = egui::Rect::from_min_size(egui::pos2(20.0, 100.0), size);
            let (left, gap, right) = pane_rects(area, 0.5);
            assert_eq!(left.size(), right.size());
            assert_eq!(left.top(), right.top());
            assert_eq!(left.bottom(), right.bottom());
            assert_eq!(gap.center().x, area.center().x);
            assert_eq!(left.right(), gap.left());
            assert_eq!(right.left(), gap.right());
        }
    }

    fn render(
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
    ) -> (egui::Rect, Color32, bool) {
        let mut rect = egui::Rect::NOTHING;
        let mut clicked = false;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(400.0, 200.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response = primary(ui, "Format", palette(false));
                    rect = response.rect;
                    clicked = response.clicked();
                });
            },
        );
        let fill = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(shape)
                    if shape.rect.center().distance(rect.center()) < 0.1
                        && shape.fill != Color32::TRANSPARENT =>
                {
                    Some(shape.fill)
                }
                _ => None,
            })
            .expect("button background");
        (rect, fill, clicked)
    }

    #[test]
    fn comparison_preserves_normal_line_spacing_at_each_pixel_scale() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            configure(&ctx, false);
            let source = "{\n  \"name\": \"中文 🚀\",\n  \"value\": 123,\n  \"ready\": true\n}";
            let alignment = json_lens::alignment::align(source, source);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                ctx.set_pixels_per_point(scale);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let normal = CodeCache::default().layout(ui, source, false, "");
                    let aligned = aligned_galley(ui, normal.clone(), Some(&alignment), true);
                    for (before, after) in normal.rows.iter().zip(&aligned.rows) {
                        assert_eq!(
                            before.pos, after.pos,
                            "line spacing changed at scale {scale}"
                        );
                        assert_eq!(before.size, after.size);
                        assert!(Arc::ptr_eq(&before.row, &after.row));
                    }
                    assert_eq!(normal.rect, aligned.rect);
                    let right = source.replace(
                        "  \"ready\": true",
                        "  \"extra\": {\n    \"nested\": true\n  },\n  \"ready\": true",
                    );
                    let gaps = json_lens::alignment::align(source, &right);
                    let aligned = aligned_galley(ui, normal.clone(), Some(&gaps), true);
                    let line_height = normal.rows[0].size.y;
                    for (index, (before, after)) in
                        normal.rows.iter().zip(&aligned.rows).enumerate()
                    {
                        assert_eq!(before.size, after.size);
                        assert!(Arc::ptr_eq(&before.row, &after.row));
                        assert!(
                            (after.pos.y - gaps.left.rows[index] as f32 * line_height).abs()
                                <= 0.5 / aligned.pixels_per_point
                        );
                    }
                });
            });
        }
    }

    #[test]
    fn aligned_text_preserves_cursor_offsets_and_corresponding_row_heights() {
        let ctx = egui::Context::default();
        configure(&ctx, false);
        let left = "{\n\"a\": 1,\n\"last\": \"中文\"\n}";
        let right = "{\n\"a\": 1,\n\"new\": [\n  1\n],\n\"last\": \"中文\"\n}";
        let alignment = json_lens::alignment::align(left, right);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let a = CodeCache::default().layout(ui, left, false, "");
                let b = CodeCache::default().layout(ui, right, false, "");
                let a = aligned_galley(ui, a, Some(&alignment), true);
                let b = aligned_galley(ui, b, Some(&alignment), false);
                assert_eq!(a.rows[2].pos.y, b.rows[5].pos.y);
                assert_eq!(a.rect.height(), b.rect.height());
                assert_eq!(a.job.text, left);
                assert_eq!(b.job.text, right);
                let index = left[..left.find("中文").unwrap()].chars().count();
                let cursor = egui::text::CCursor::new(index);
                let position = a.pos_from_cursor(cursor);
                assert_eq!(a.cursor_from_pos(position.center().to_vec2()).index, index);
                let edited = format!("{left}\n");
                let normal = CodeCache::default().layout(ui, &edited, false, "");
                let unchanged = aligned_galley(ui, normal.clone(), Some(&alignment), true);
                assert!(Arc::ptr_eq(&normal, &unchanged));
            });
        });
    }

    #[test]
    fn editor_gutter_stays_outside_horizontal_scrolling() {
        let ctx = egui::Context::default();
        configure(&ctx, false);
        let mut text = format!("{}\n", "long line ".repeat(100)).repeat(100);
        let mut cache = CodeCache::default();
        let mut previous = None;
        for frame in 0..2 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(500.0, 400.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let scroll = editor_scroll(
                            ui,
                            EditorViewport {
                                salt: "gutter_test",
                                wrap: false,
                                height: 200.0,
                                offset: Some(0.0),
                                palette: palette(false),
                                left: false,
                                first_line: 1,
                                alignment: None,
                            },
                            |ui| {
                                let mut layouter =
                                    |ui: &egui::Ui, text: &dyn egui::TextBuffer, _: f32| {
                                        cache.layout(ui, text.as_str(), false, "")
                                    };
                                egui::TextEdit::multiline(&mut text)
                                    .code_editor()
                                    .font(egui::TextStyle::Monospace)
                                    .frame(false)
                                    .margin(Vec2::ZERO)
                                    .desired_width(ui.available_width())
                                    .layouter(&mut layouter)
                                    .show(ui)
                            },
                        );
                        if frame == 0 {
                            previous = Some((scroll.inner_rect.left(), scroll.inner.galley_pos.x));
                            let mut state = scroll.state;
                            state.offset.x = 100.0;
                            state.store(ctx, scroll.id);
                        } else {
                            let (viewport_x, text_x) = previous.unwrap();
                            assert_eq!(viewport_x, scroll.inner_rect.left());
                            assert!(scroll.inner.galley_pos.x < text_x - 50.0);
                            assert!(scroll.inner_rect.left() > ui.max_rect().left() + 64.0);
                        }
                    });
                },
            );
        }
    }

    #[test]
    fn primary_button_has_distinct_hover_press_and_release_feedback() {
        let ctx = egui::Context::default();
        configure(&ctx, false);
        let (rect, idle, _) = render(&ctx, 0.0, vec![]);
        let center = rect.center();
        let (_, hovered, _) = render(&ctx, 0.1, vec![egui::Event::PointerMoved(center)]);
        let event = |pressed| egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        let (_, pressed, _) = render(&ctx, 0.2, vec![event(true)]);
        assert_ne!(idle, hovered);
        assert_ne!(hovered, pressed);
        let (_, _, clicked) = render(&ctx, 0.3, vec![event(false)]);
        assert!(clicked);
    }

    #[test]
    fn source_cursor_uses_the_same_height_as_the_monospace_glyphs() {
        let ctx = egui::Context::default();
        configure(&ctx, false);
        let mut cache = CodeCache::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for text in ["{\n  \"name\": 1\n}", "", "name\n"] {
                    let galley = cache.layout(ui, text, false, "");
                    let font_height =
                        ui.fonts_mut(|fonts| fonts.row_height(&FontId::monospace(13.0)));
                    let mut offset = 0;
                    for row in &galley.rows {
                        let cursor = egui::text_selection::text_cursor_state::cursor_rect(
                            &galley,
                            &egui::text::CCursor::new(offset),
                            font_height,
                        );
                        assert!(
                            // egui expands the font-aligned caret by 1.5 points at each end.
                            (cursor.height() - font_height - 3.0).abs() < 1.0,
                            "cursor={} font={}",
                            cursor.height(),
                            font_height
                        );
                        offset += row.glyphs.len() + usize::from(row.ends_with_newline);
                    }
                }
            });
        });
    }

    #[test]
    fn cached_source_layout_tracks_font_atlas_rebuilds() {
        let ctx = egui::Context::default();
        let mut cache = CodeCache::default();
        let text = "{\n  \"name\": \"JSON Lens\",\n  \"value\": 123\n}";
        let mut previous = None;
        for dark in [false, true, true, false, false, true] {
            configure(&ctx, dark);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Populate the atlas differently after each reset to expose stale glyph UVs.
                    ui.label("abcdefghijklmnopqrstuvwxyz 0123456789");
                    let actual = cache.layout(ui, text, dark, "name");
                    let expected =
                        ui.fonts_mut(|fonts| fonts.layout_job(cache.job.as_ref().unwrap().clone()));
                    assert!(Arc::ptr_eq(&actual, &expected));
                    if let Some((old_dark, old)) = &previous
                        && *old_dark != dark
                    {
                        assert!(!Arc::ptr_eq(&actual, old));
                    }
                    assert_eq!(actual.job.text, text);
                    previous = Some((dark, actual));
                });
            });
        }
        // Same colors and text, but reloading fonts still invalidates the old galley.
        let mut definitions = egui::FontDefinitions::default();
        let mono = definitions.families[&egui::FontFamily::Monospace][0].clone();
        let font = definitions.font_data[&mono].clone();
        definitions
            .font_data
            .insert("atlas-reset-test".into(), font);
        definitions
            .families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .insert(0, "atlas-reset-test".into());
        ctx.set_fonts(definitions);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let actual = cache.layout(ui, text, true, "name");
                assert!(!Arc::ptr_eq(&actual, &previous.as_ref().unwrap().1));
                let expected =
                    ui.fonts_mut(|fonts| fonts.layout_job(cache.job.as_ref().unwrap().clone()));
                assert!(Arc::ptr_eq(&actual, &expected));
            });
        });
    }
}
