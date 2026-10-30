use super::*;
use std::time::Duration;
use web_time::Instant;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Working,
    Success,
    Info,
    Error,
}

pub struct Feedback {
    pub text: String,
    pub kind: Kind,
    pub created: Instant,
}

impl Feedback {
    pub fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind,
            created: Instant::now(),
        }
    }
    pub fn expired(&self) -> bool {
        self.kind != Kind::Working
            && self.created.elapsed()
                >= Duration::from_secs(if self.kind == Kind::Error { 7 } else { 4 })
    }
}

impl App {
    pub(super) fn feedback_panel(&mut self, ctx: &egui::Context, p: Palette) {
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
            .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-24.0, -24.0))
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
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct EditDialog {
    pub pointer: String,
    pub json: String,
    pub key: Option<String>,
    pub rename: bool,
    pub error: String,
}

impl App {
    pub(super) fn edit_window(&mut self, ctx: &egui::Context, p: Palette) {
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
}
