use super::*;

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
}
