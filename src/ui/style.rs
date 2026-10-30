use super::*;

pub(super) const APP_HEADER_HEIGHT: f32 = 36.0;

pub(super) fn brand_title(ui: &mut egui::Ui, dark: bool) {
    let color = palette(dark).text;
    let title = ui.painter().layout_no_wrap(
        "JSON Lens".into(),
        FontId::new(20.0, egui::FontFamily::Name("Brand".into())),
        color,
    );
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(title.mesh_bounds.width(), 26.0),
        egui::Sense::hover(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), "JSON Lens")
    });
    // Center the visible glyphs rather than the font's padded line box.
    let origin = rect.center() - title.mesh_bounds.center().to_vec2();
    ui.painter().galley(origin, title, color);
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "JetBrains Mono".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "Maple Mono CN".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/MapleMono-NF-CN-Regular.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .splice(0..0, ["Maple Mono CN".into(), "JetBrains Mono".into()]);
    fonts.font_data.insert(
        "Inter".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter.ttf"
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
        "Pixelify Sans Bold".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/PixelifySans-Bold.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name("Brand".into()),
        vec!["Pixelify Sans Bold".into(), "Inter".into()],
    );
    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) canvas: Color32,
    pub(super) surface: Color32,
    pub(super) subtle: Color32,
    pub(super) border: Color32,
    pub(super) text: Color32,
    pub(super) muted: Color32,
    pub(super) accent: Color32,
    pub(super) tint: Color32,
    pub(super) green: Color32,
    pub(super) amber: Color32,
    pub(super) red: Color32,
}

pub(super) fn palette(dark: bool) -> Palette {
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
            surface: Color32::from_rgb(247, 248, 252),
            subtle: Color32::from_rgb(237, 240, 246),
            border: Color32::from_rgb(219, 224, 234),
            text: Color32::from_rgb(48, 55, 73),
            muted: Color32::from_rgb(121, 132, 151),
            accent: Color32::from_rgb(128, 104, 199),
            tint: Color32::from_rgb(238, 235, 248),
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
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        FontId::monospace(EDITOR_FONT_SIZE),
    );
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
    style.visuals.text_cursor.stroke = Stroke::new(
        2.0_f32,
        if dark {
            Color32::from_rgb(94, 211, 255)
        } else {
            p.text
        },
    );
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

pub(super) fn action_button(
    ui: &mut egui::Ui,
    text: &str,
    p: Palette,
    primary: bool,
    size: Vec2,
) -> egui::Response {
    action_button_with_radius(ui, text, p, primary, size, 7.into())
}

pub(super) fn action_button_with_radius(
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
                    p.subtle
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

pub(super) fn segmented_button(
    ui: &mut egui::Ui,
    text: &str,
    p: Palette,
    selected: bool,
    size: Vec2,
) -> egui::Response {
    segmented_button_with_fill(
        ui,
        text,
        p,
        selected,
        size,
        if selected {
            p.tint
        } else {
            Color32::TRANSPARENT
        },
    )
}

pub(super) fn segmented_button_with_fill(
    ui: &mut egui::Ui,
    text: &str,
    p: Palette,
    selected: bool,
    size: Vec2,
    fill: Color32,
) -> egui::Response {
    ui.scope(|ui| {
        let style = ui.style_mut();
        style.visuals.widgets.inactive.weak_bg_fill = fill;
        style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        style.visuals.widgets.hovered.weak_bg_fill = p.tint;
        style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        style.visuals.widgets.active.weak_bg_fill = p.tint.lerp_to_gamma(p.accent, 0.18);
        style.visuals.widgets.active.bg_stroke = Stroke::NONE;
        style.visuals.widgets.active.expansion = 0.0;
        ui.add(
            egui::Button::new(RichText::new(text).color(if selected { p.accent } else { p.text }))
                .corner_radius(5)
                .min_size(size),
        )
    })
    .inner
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn sort_button(ui: &mut egui::Ui, p: Palette, state: SortState) -> egui::Response {
    let label = "\u{2003}  Sort";
    let label_width = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(12.0), p.text)
        .size()
        .x;
    let response = segmented_button(ui, label, p, false, Vec2::new(label_width + 16.0, 24.0));
    let center = egui::pos2(
        response.rect.center().x - label_width * 0.5 + 6.0,
        response.rect.center().y,
    );
    let font = FontId::proportional(12.0);
    let icon =
        ui.painter()
            .layout_no_wrap("\u{f0dc}".into(), font.clone(), p.muted.gamma_multiply(0.6));
    let origin = center - icon.mesh_bounds.center().to_vec2();
    ui.painter().galley(origin, icon, p.muted);
    let active = match state {
        SortState::Original => None,
        SortState::Ascending => Some("\u{f0de}"),
        SortState::Descending => Some("\u{f0dd}"),
    };
    if let Some(active) = active {
        let arrow = ui.painter().layout_no_wrap(active.into(), font, p.accent);
        ui.painter().galley(origin, arrow, p.accent);
    }
    response
}

pub(super) fn segment_divider(ui: &mut egui::Ui, p: Palette, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), egui::Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, p.border));
}

pub(super) fn button(ui: &mut egui::Ui, text: &str, p: Palette) -> egui::Response {
    action_button(ui, text, p, false, Vec2::new(76.0, 34.0))
}

pub(super) fn repository_link(ui: &mut egui::Ui, p: Palette) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("View source on GitHub");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Link,
            ui.is_enabled(),
            "JSON Lens on GitHub",
        )
    });
    let highlighted = response.hovered() || response.has_focus();
    if highlighted {
        ui.painter().rect_filled(rect, 3, p.tint);
    }
    let color = if highlighted { p.accent } else { p.text };
    let icon = ui
        .painter()
        .layout_no_wrap("\u{f09b}".into(), FontId::monospace(15.0), color);
    let origin = rect.center() - icon.mesh_bounds.center().to_vec2();
    ui.painter().galley(origin, icon, color);
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(
            "https://github.com/sihaixianyu/json-lens",
        ));
    }
}

pub(super) fn app_logo(ui: &mut egui::Ui, p: Palette, size: f32) {
    let (logo, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let point = |x: f32, y: f32| logo.min + egui::vec2(x, y) * (logo.width() / 128.0);
    let curves = [
        [(46.0, 28.0), (33.0, 28.0), (34.0, 38.0), (34.0, 48.0)],
        [(34.0, 48.0), (34.0, 59.0), (27.0, 61.0), (22.0, 64.0)],
        [(22.0, 64.0), (27.0, 67.0), (34.0, 69.0), (34.0, 80.0)],
        [(34.0, 80.0), (34.0, 90.0), (33.0, 100.0), (46.0, 100.0)],
    ];
    for mirrored in [false, true] {
        for curve in curves {
            ui.painter()
                .add(egui::epaint::CubicBezierShape::from_points_stroke(
                    curve.map(|(x, y)| point(if mirrored { 128.0 - x } else { x }, y)),
                    false,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.5, p.accent),
                ));
        }
    }
    for x in [54.0, 64.0, 74.0] {
        ui.painter().circle_filled(point(x, 64.0), 0.7, p.accent);
    }
}

pub(super) fn primary(ui: &mut egui::Ui, text: &str, p: Palette) -> egui::Response {
    action_button(ui, text, p, true, Vec2::new(80.0, 34.0))
}

pub(super) fn badge(ui: &mut egui::Ui, text: &str, color: Color32, fill: Color32) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(color));
        });
}

pub(super) const PANE_GAP: f32 = 6.0;

pub(super) fn pane_rects(area: egui::Rect, ratio: f32) -> (egui::Rect, egui::Rect, egui::Rect) {
    let middle = area.left() + area.width() * ratio;
    let gap = PANE_GAP;
    (
        egui::Rect::from_min_max(area.min, egui::pos2(middle - gap * 0.5, area.bottom())),
        egui::Rect::from_min_max(
            egui::pos2(middle - gap * 0.5, area.top()),
            egui::pos2(middle + gap * 0.5, area.bottom()),
        ),
        egui::Rect::from_min_max(egui::pos2(middle + gap * 0.5, area.top()), area.max),
    )
}

pub(super) fn align_content(ui: &mut egui::Ui, top: f32, height: f32) {
    ui.add_space((top + height - ui.cursor().top()).max(0.0));
}

pub(super) fn empty(ui: &mut egui::Ui, title: &str, description: &str, p: Palette) {
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

pub(super) fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

pub(super) fn match_count(selected: usize, stored: usize, total: usize) -> String {
    if total == 0 {
        "0 matches".into()
    } else if total > stored {
        format!("{}/{} · {stored} shown", selected + 1, total)
    } else {
        format!("{}/{}", selected + 1, total)
    }
}
