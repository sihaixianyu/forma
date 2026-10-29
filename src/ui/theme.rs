use eframe::egui::{self, Color32, FontId, Stroke, Vec2};
use std::sync::Arc;

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
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
        .insert(0, "Maple Mono CN".into());
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
        "Bungee".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Bungee-Regular.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name("Brand".into()),
        vec!["Bungee".into(), "Inter".into()],
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
    style.visuals.text_cursor.stroke = Stroke::new(
        2.0_f32,
        if dark {
            Color32::from_rgb(94, 211, 255)
        } else {
            Color32::from_rgb(0, 132, 240)
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
