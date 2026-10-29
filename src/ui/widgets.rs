use super::theme::Palette;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};

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

pub(super) fn button(ui: &mut egui::Ui, text: &str, p: Palette) -> egui::Response {
    action_button(ui, text, p, false, Vec2::new(76.0, 34.0))
}

pub(super) fn repository_link(ui: &mut egui::Ui, p: Palette) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::click());
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
        ui.painter().rect_filled(rect, 8, p.tint);
    }
    // The bundled Nerd Font includes the GitHub mark, so it works offline
    // and inherits the current theme without a separate image loader.
    let color = if highlighted { p.accent } else { p.text };
    let icon = ui
        .painter()
        .layout_no_wrap("\u{f09b}".into(), FontId::monospace(22.0), color);
    // Center the visible mark rather than the font's advance and line box.
    let origin = rect.center() - icon.mesh_bounds.center().to_vec2();
    ui.painter().galley(origin, icon, color);
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(
            "https://github.com/sihaixianyu/json-lens",
        ));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn header_drag(ui: &egui::Ui, response: &egui::Response) {
    if response.double_clicked_by(egui::PointerButton::Primary) {
        let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
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

pub(super) fn pane_rects(area: egui::Rect, ratio: f32) -> (egui::Rect, egui::Rect, egui::Rect) {
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
