use super::*;

pub(crate) mod find;
pub(crate) mod jobs;
mod lifecycle;

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
                    self.compare_text = self.editor.replacen(
                        "\"name\": \"JSON Lens\"",
                        "\"name\": \"JSON Lens (comparison)\"",
                        1,
                    );
                    if std::env::var_os("JSON_LENS_ALIGN").is_some() {
                        let mut right: Value = serde_json::from_str(&self.compare_text).unwrap();
                        right["controls"]["extra"] = serde_json::json!({"nested": true});
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
                    self.pointer = "/name".into();
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

impl App {
    pub(crate) fn reset_sort_state(&mut self) {
        self.sort_state = SortState::Original;
        self.sort_original = None;
    }

    pub(crate) fn apply_sort_state(&mut self, state: SortState) {
        if state == SortState::Original {
            if let Some(original) = self.sort_original.take() {
                self.editor = original;
                self.dirty = true;
                self.sort_state = SortState::Original;
                self.parse(Format::Keep);
            }
            return;
        }
        if self.sort_state == SortState::Original {
            self.sort_original = Some((*self.source()).clone());
        }
        self.sort_state = state;
        self.parse(match state {
            SortState::Ascending => Format::SortAscending,
            SortState::Descending => Format::SortDescending,
            SortState::Original => Format::Keep,
        });
    }

    pub(crate) fn set_theme_preference(
        &mut self,
        ctx: &egui::Context,
        preference: egui::ThemePreference,
    ) {
        self.theme_preference = preference;
        self.sync_theme(ctx);
    }

    pub(crate) fn sync_theme(&mut self, ctx: &egui::Context) {
        let dark = match self.theme_preference {
            egui::ThemePreference::System => {
                ctx.system_theme().unwrap_or(egui::Theme::Light) == egui::Theme::Dark
            }
            egui::ThemePreference::Light => false,
            egui::ThemePreference::Dark => true,
        };
        if self.dark != dark {
            self.dark = dark;
            ui::configure(ctx, dark);
            ctx.request_discard("Theme changed");
        }
        ctx.set_theme(self.theme_preference);
    }
    pub(crate) fn clear_comparison(&mut self) {
        self.differences.clear();
        self.compare_alignment = None;
        self.comparison_done = false;
        self.compare_left_range = None;
        self.compare_right_range = None;
        self.compare_jump = false;
        self.compare_due = (self.view == View::Compare && !self.compare_text.trim().is_empty())
            .then(|| Instant::now() + Duration::from_millis(300));
    }

    pub(crate) fn tick_automatic_jobs(&mut self, now: Instant) {
        if self.busy {
            return;
        }
        if self.auto_due.is_some_and(|due| due <= now) {
            self.auto_due = None;
            self.start(
                Job::Process(self.search_source.clone(), Format::Keep),
                false,
            );
        } else if self.view == View::Compare && self.compare_due.is_some_and(|due| due <= now) {
            if self.dirty {
                self.start(
                    Job::Process(self.search_source.clone(), Format::Keep),
                    false,
                );
            } else {
                self.compare_due = None;
                let left = self.source();
                self.start(
                    Job::Compare(left, Arc::new(self.compare_text.clone())),
                    false,
                );
            }
        }
    }
    pub(crate) fn apply_diff_location(
        &mut self,
        left: Option<std::ops::Range<usize>>,
        right: Option<std::ops::Range<usize>>,
    ) {
        self.compare_left_range = left.clone();
        self.compare_right_range = right;
        self.source_target = left;
        self.search.jump = self.source_target.is_some();
        self.compare_jump = true;
    }

    pub(crate) fn notify(&mut self, kind: feedback::Kind, text: impl Into<String>) {
        self.feedback = Some(feedback::Feedback::new(kind, text));
    }

    pub(crate) fn submit(&mut self, job: Job) {
        if self.busy {
            if self.automatic_job {
                let label = job.label();
                self.queued_actions.push_back(QueuedAction {
                    job,
                    revision: self.revision,
                });
                self.notify(
                    feedback::Kind::Info,
                    format!("{label} queued — waiting for preview update"),
                );
            } else {
                self.notify(
                    feedback::Kind::Info,
                    "An operation is still running. Please wait.",
                );
            }
            return;
        }
        self.start(job, true);
    }

    pub(crate) fn start(&mut self, job: Job, user_action: bool) {
        let job = match job {
            Job::RequestFormat(format) => {
                self.auto_due = None;
                if !self.dirty
                    && let Some(doc) = self.document.clone()
                {
                    Job::Transform(doc, format, self.graph.clone())
                } else {
                    Job::Process(self.source(), format)
                }
            }
            job => job,
        };
        let label = job.label();
        if matches!(job, Job::Compare(..)) {
            self.compare_due = None;
        }
        let quiet = matches!(
            job,
            Job::Locate(..)
                | Job::Inspect(..)
                | Job::SourcePointer(..)
                | Job::LocateDiff(..)
                | Job::FocusGraph(..)
        );
        let reset_view = match job {
            Job::Import(..) => true,
            #[cfg(not(target_arch = "wasm32"))]
            Job::Open(_) => true,
            _ => false,
        };
        if self
            .jobs
            .send(Work {
                job,
                revision: self.revision,
                reset_view,
                label,
                user_action,
            })
            .is_ok()
        {
            if reset_view {
                self.reset_sort_state();
            }
            self.busy = true;
            self.automatic_job = !user_action;
            self.status = if user_action {
                format!("{label} in progress…")
            } else {
                "Updating preview…".into()
            };
            if user_action && !quiet {
                self.notify(feedback::Kind::Working, self.status.clone());
            }
        } else {
            self.error = true;
            self.status = "Processing thread unavailable".into();
            self.notify(feedback::Kind::Error, self.status.clone());
        }
    }

    pub(crate) fn dispatch_next(&mut self) {
        while !self.busy {
            let Some(action) = self.queued_actions.pop_front() else {
                break;
            };
            if action.revision != self.revision {
                self.notify(
                    feedback::Kind::Info,
                    "Queued operation cancelled because the source changed",
                );
                continue;
            }
            self.start(action.job, true);
        }
    }

    pub(crate) fn source(&mut self) -> Arc<String> {
        if self.dirty && self.full_text.as_str() != self.editor {
            self.full_text = Arc::new(self.editor.clone());
        }
        self.full_text.clone()
    }

    pub(crate) fn parse(&mut self, format: Format) {
        self.submit(Job::RequestFormat(format));
    }
}

#[cfg(test)]
mod tests;
