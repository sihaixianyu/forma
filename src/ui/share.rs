use super::*;
use forma::share::{Snapshot, WEB_APP, link};

#[derive(Clone)]
struct ShareDialog {
    snapshot: Snapshot,
    link: Result<String, String>,
    opened_frame: u64,
}

impl App {
    pub(super) fn open_share(&mut self, ctx: &egui::Context) {
        let compare = self.view == View::Compare;
        let snapshot = Snapshot {
            left: self.source().as_str().to_owned(),
            right: if compare {
                self.review_original_right
                    .as_deref()
                    .unwrap_or(&self.compare_text)
                    .clone()
            } else {
                String::new()
            },
            compare,
        };
        // Desktop links open the hosted web app; browser links retain their deployment path.
        #[cfg(target_arch = "wasm32")]
        let base = web_sys::window()
            .and_then(|window| window.location().href().ok())
            .unwrap_or_else(|| WEB_APP.into());
        #[cfg(not(target_arch = "wasm32"))]
        let base = WEB_APP.to_owned();
        let dialog = ShareDialog {
            link: link(&base, &snapshot),
            snapshot,
            opened_frame: ctx.cumulative_frame_nr(),
        };
        #[cfg(target_arch = "wasm32")]
        if let Ok(link) = &dialog.link
            && let Some(window) = web_sys::window()
            && window
                .history()
                .and_then(|history| {
                    history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(link))
                })
                .is_err()
        {
            self.notify(
                feedback::Kind::Error,
                "Could not update the address bar. Use Copy link instead.",
            );
        }
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("share_dialog"), dialog));
    }

    pub(super) fn share_window(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new("share_dialog");
        let Some(dialog) = ctx.data(|data| data.get_temp::<ShareDialog>(id)) else {
            return;
        };
        let mut close = false;
        let p = palette(self.dark);
        let width = (ctx.content_rect().width() - 64.0).clamp(240.0, 460.0);
        let response = egui::Modal::new(id)
            .frame(
                egui::Frame::new()
                    .fill(p.canvas)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .corner_radius(16)
                    .inner_margin(24),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 12.0;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Share JSON")
                            .size(22.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::click());
                        let response = response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Close");
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Close")
                        });
                        if response.hovered() {
                            ui.painter().rect_filled(rect, 6, p.tint);
                        }
                        let center = rect.center();
                        for direction in [-1.0, 1.0] {
                            let offset = Vec2::new(4.0, 4.0 * direction);
                            ui.painter().line_segment(
                                [center - offset, center + offset],
                                Stroke::new(1.5_f32, p.muted),
                            );
                        }
                        close = response.clicked();
                    });
                });
                ui.horizontal(|ui| {
                    badge(
                        ui,
                        if dialog.snapshot.compare {
                            "Compare"
                        } else {
                            "Graph"
                        },
                        p.accent,
                        p.accent.gamma_multiply(0.12),
                    );
                    ui.label(
                        RichText::new(if dialog.snapshot.compare {
                            "Both JSON inputs included"
                        } else {
                            "Current JSON included"
                        })
                        .size(12.0)
                        .color(p.muted),
                    );
                });
                ui.add_space(2.0);
                match &dialog.link {
                    Ok(link) => {
                        egui::Frame::new()
                            .fill(p.subtle)
                            .stroke(Stroke::new(1.0_f32, p.border))
                            .corner_radius(10)
                            .inner_margin(14)
                            .show(ui, |ui| {
                                ui.set_width((width - 30.0).max(0.0));
                                ui.label(
                                    RichText::new("Snapshot link")
                                        .size(12.0)
                                        .strong()
                                        .color(p.muted),
                                );
                                let mut text = link.as_str();
                                ui.add(
                                    egui::TextEdit::singleline(&mut text)
                                        .font(FontId::monospace(12.0))
                                        .text_color(p.text)
                                        .frame(false)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.label(
                                    RichText::new(format!("{} characters", link.len()))
                                        .size(11.0)
                                        .color(p.muted),
                                );
                            });
                        ui.label(
                            RichText::new("Opens a fixed snapshot. Later edits stay here.")
                                .size(12.0)
                                .color(p.muted),
                        );
                        if action_button(ui, "Copy link", p, true, Vec2::new(width, 38.0)).clicked()
                        {
                            ctx.copy_text(link.clone());
                            self.notify(feedback::Kind::Success, "Share link copied");
                        }
                        if link.len() > 2_000 {
                            ui.label(
                                RichText::new("Some messaging apps may truncate long links.")
                                    .size(11.0)
                                    .color(p.amber),
                            );
                        }
                    }
                    Err(error) => {
                        egui::Frame::new()
                            .fill(p.red.gamma_multiply(0.08))
                            .corner_radius(10)
                            .inner_margin(14)
                            .show(ui, |ui| {
                                ui.set_width((width - 28.0).max(0.0));
                                ui.label(
                                    RichText::new("Unable to create link").strong().color(p.red),
                                );
                                ui.label(RichText::new(error).size(12.0).color(p.text));
                            });
                    }
                }
                ui.add_space(2.0);
                ui.separator();
                ui.label(
                    RichText::new("Anyone with the link can read this snapshot.")
                        .size(12.0)
                        .color(p.muted),
                );
                ui.label(
                    RichText::new("Links are not encrypted and cannot be revoked.")
                        .size(11.0)
                        .color(p.muted),
                );
            });
        if close || (ctx.cumulative_frame_nr() > dialog.opened_frame && response.should_close()) {
            ctx.data_mut(|data| data.remove::<ShareDialog>(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_button_opens_a_dialog_that_survives_the_opening_click() {
        let ctx = egui::Context::default();
        let mut app = App::create(ctx.clone(), "test".into());
        app.busy = false;
        let frame = |app: &mut App, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| app.workspace(ctx),
            )
        };
        let output = frame(&mut app, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text.contains("Share")
                {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .expect("Share button");
        for pressed in [true, false] {
            frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(
            ctx.data(|data| data.get_temp::<ShareDialog>(egui::Id::new("share_dialog")))
                .is_some()
        );
        frame(&mut app, vec![]);
        assert!(
            ctx.data(|data| data.get_temp::<ShareDialog>(egui::Id::new("share_dialog")))
                .is_some()
        );
    }

    #[test]
    fn sharing_captures_dirty_input_and_original_comparison_as_a_fixed_snapshot() {
        let ctx = egui::Context::default();
        let mut app = App::create(ctx.clone(), "test".into());
        app.editor = "{\"fresh\": 123456789012345678901234567890}".into();
        app.dirty = true;
        app.view = View::Compare;
        app.compare_text = "{\"display_order\": true}".into();
        app.review_original_right = Some(Arc::new("{\"original_order\": true}".into()));
        app.open_share(&ctx);
        app.editor.clear();
        let dialog = ctx
            .data(|data| data.get_temp::<ShareDialog>(egui::Id::new("share_dialog")))
            .unwrap();
        assert_eq!(
            dialog.snapshot.left,
            "{\"fresh\": 123456789012345678901234567890}"
        );
        assert_eq!(dialog.snapshot.right, "{\"original_order\": true}");
        assert!(dialog.snapshot.compare);
        assert!(dialog.link.unwrap().starts_with(WEB_APP));

        app.view = View::Graph;
        app.open_share(&ctx);
        let dialog = ctx
            .data(|data| data.get_temp::<ShareDialog>(egui::Id::new("share_dialog")))
            .unwrap();
        assert!(dialog.snapshot.right.is_empty());
        assert!(!dialog.snapshot.compare);
    }
}
