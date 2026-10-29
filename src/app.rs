use super::*;

mod init;
mod jobs;
mod receive;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_theme(ctx);
        self.receive();
        self.tick_automatic_jobs(Instant::now());
        #[cfg(target_arch = "wasm32")]
        if let Some(doc) = self.document.clone() {
            self.search.check_pointer(doc, &self.pointer);
        }
        if self.search.right.pending && self.compare_search_source.as_str() != self.compare_text {
            self.compare_search_source = Arc::new(self.compare_text.clone());
        }
        self.search.tick(
            ctx,
            self.search_source.clone(),
            if self.dirty {
                None
            } else {
                self.document.clone()
            },
            self.compare_search_source.clone(),
        );
        #[cfg(target_arch = "wasm32")]
        if std::mem::take(&mut self.search.pointer_reset) {
            self.pointer.clear();
            self.canvas.focus(String::new());
            self.search.pointer_valid = true;
        }
        if self.search.jump {
            self.prepare_search_jump();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.smoke_test {
                let capture = ctx.input(|input| {
                    input.events.iter().find_map(|event| {
                        if let egui::Event::Screenshot { image, .. } = event {
                            Some(image.clone())
                        } else {
                            None
                        }
                    })
                });
                if let Some(image) = capture {
                    if let Some(path) = std::env::var_os("JSON_LENS_CAPTURE") {
                        let mut bytes =
                            format!("P6\n{} {}\n255\n", image.size[0], image.size[1]).into_bytes();
                        for pixel in &image.pixels {
                            bytes.extend_from_slice(&pixel.to_array()[..3]);
                        }
                        std::fs::write(path, bytes).expect("save UI capture");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            if self.smoke_test && self.document.is_some() && !self.busy {
                self.smoke_frames += 1;
                ctx.request_repaint();
                if self.smoke_frames == 1 && std::env::var_os("JSON_LENS_COMPARE").is_some() {
                    self.view = View::Compare;
                    self.compare_text = self.editor.replace("Getting started", "Changed item");
                    if std::env::var_os("JSON_LENS_ALIGN").is_some() {
                        let mut right: Value = serde_json::from_str(&self.compare_text).unwrap();
                        right["items"][0]["extra"] = serde_json::json!({"nested": true});
                        self.compare_text = serde_json::to_string_pretty(&right).unwrap();
                    }
                    self.submit(Job::Compare(
                        self.full_text.clone(),
                        Arc::new(self.compare_text.clone()),
                    ));
                }
                if self.smoke_frames == 1 && std::env::var_os("JSON_LENS_FEEDBACK").is_some() {
                    self.parse(Format::Pretty);
                }
                if self.smoke_frames == 1
                    && std::env::var_os("JSON_LENS_EDIT").is_some()
                    && let Some(doc) = self.document.clone()
                {
                    self.pointer = "/items/0/name".into();
                    self.submit(Job::Inspect(doc, self.pointer.clone()));
                }
                if self.smoke_frames == 1 && std::env::var_os("JSON_LENS_SEARCH").is_some() {
                    self.search.text.visible = true;
                    self.search.tree.visible = true;
                    self.search.text.query = "name".into();
                    self.search.text.changed();
                    self.search.tree.query = "name".into();
                    self.search.tree.changed();
                }
                if self.smoke_frames >= 10
                    && !self.smoke_capture_requested
                    && (std::env::var_os("JSON_LENS_SEARCH").is_none()
                        || (!self.search.text.pending && !self.search.tree.pending))
                {
                    self.smoke_capture_requested = true;
                    println!(
                        "Native window rendered · {} · {}",
                        self.gpu_info, self.status
                    );
                    if std::env::var_os("JSON_LENS_CAPTURE").is_some() {
                        ctx.send_viewport_cmd(
                            egui::ViewportCommand::Screenshot(Default::default()),
                        );
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            if self.smoke_test && std::env::var_os("JSON_LENS_GRAPH").is_some() {
                self.view = View::Graph;
                if std::env::var_os("JSON_LENS_GRAPH_WIDE").is_some() {
                    self.graph_wide = true;
                }
            }
        }
        self.workspace(ctx);
        if self.auto_due.is_some() || (self.view == View::Compare && self.compare_due.is_some()) {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests;
