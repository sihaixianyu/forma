use super::*;

impl App {
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
