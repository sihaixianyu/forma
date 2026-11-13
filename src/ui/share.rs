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
        if let Some(result) = ctx.data_mut(|data| {
            let id = egui::Id::new("share_export_result");
            let result = data.get_temp::<Result<(), String>>(id);
            data.remove::<Result<(), String>>(id);
            result
        }) {
            match result {
                Ok(()) => self.notify(feedback::Kind::Success, "JSON exported"),
                Err(error) => self.notify(feedback::Kind::Error, error),
            }
        }
        let id = egui::Id::new("share_dialog");
        let Some(dialog) = ctx.data(|data| data.get_temp::<ShareDialog>(id)) else {
            return;
        };
        let mut close = false;
        let response = egui::Modal::new(id).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading("Share JSON");
            ui.label(if dialog.snapshot.compare { "Snapshot includes both JSON inputs and Compare mode." } else { "Snapshot includes the current JSON and Graph mode." });
            ui.label("Anyone with this link can read the content. Links are not encrypted and cannot be revoked.");
            ui.add_space(8.0);
            match &dialog.link {
                Ok(link) => {
                    let mut text = link.as_str();
                    ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
                    ui.label(format!("{} characters · some messaging apps may truncate long links", link.len()));
                    if ui.button("Copy link").clicked() {
                        ctx.copy_text(link.clone());
                        self.notify(feedback::Kind::Success, "Share link copied");
                    }
                }
                Err(error) => { ui.colored_label(palette(self.dark).red, error); }
            }
            ui.horizontal(|ui| {
                if ui.button(if dialog.snapshot.compare { "Export left JSON" } else { "Export JSON" }).clicked() {
                    export(ctx, "document.json", dialog.snapshot.left.clone());
                }
                if dialog.snapshot.compare && ui.button("Export right JSON").clicked() {
                    export(ctx, "comparison.json", dialog.snapshot.right.clone());
                }
                close = ui.button("Close").clicked();
            });
        });
        if close || (ctx.cumulative_frame_nr() > dialog.opened_frame && response.should_close()) {
            ctx.data_mut(|data| data.remove::<ShareDialog>(id));
        }
    }
}

fn export(ctx: &egui::Context, name: &str, text: String) {
    let ctx = ctx.clone();
    #[cfg(target_arch = "wasm32")]
    {
        let name = name.to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_file_name(name)
                .save_file()
                .await
            {
                let result = file
                    .write(text.as_bytes())
                    .await
                    .map_err(|error| format!("Could not export JSON: {error}"));
                ctx.data_mut(|data| data.insert_temp(egui::Id::new("share_export_result"), result));
                ctx.request_repaint();
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = rfd::FileDialog::new().set_file_name(name).save_file() {
        let result =
            std::fs::write(path, text).map_err(|error| format!("Could not export JSON: {error}"));
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("share_export_result"), result));
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
