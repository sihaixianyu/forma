use super::*;

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
