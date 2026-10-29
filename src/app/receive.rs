use super::*;

impl App {
    pub(crate) fn receive(&mut self) {
        #[cfg(target_arch = "wasm32")]
        while let Ok(event) = self.files.events.try_recv() {
            match event {
                browser::FileEvent::Opened { name, text } => {
                    self.filename = name.clone();
                    self.revision += 1;
                    let job = Job::Import(Arc::new(text), name);
                    if self.busy {
                        self.queued_actions.push_back(QueuedAction {
                            job,
                            revision: self.revision,
                        });
                    } else {
                        self.submit(job);
                    }
                }
                browser::FileEvent::Saved(name) => {
                    self.error = false;
                    self.status = format!("Downloaded {name}");
                    self.notify(feedback::Kind::Success, self.status.clone());
                }
                browser::FileEvent::Error(error) => {
                    self.error = true;
                    self.status = error;
                    self.notify(feedback::Kind::Error, self.status.clone());
                }
            }
        }
        while let Ok(done) = self.completed.try_recv() {
            self.busy = false;
            self.automatic_job = false;
            if done.revision != self.revision {
                log::debug!(target: "json_lens", "event=stale_result revision={} current_revision={}", done.revision, self.revision);
                continue;
            }
            let feedback_before = self.full_text.clone();
            match done.result {
                Ok(Output::Document(doc, graph)) => {
                    self.syntax_error = None;
                    if let Some(target) = done.target {
                        self.pointer = target.clone();
                        self.pending_reveal = Some(target);
                    }
                    if done.reset_view {
                        self.canvas = canvas::Canvas::default();
                        self.pointer.clear();
                    } else if !Arc::ptr_eq(&self.graph, &graph) {
                        self.canvas.reconcile(&self.graph, &graph);
                    }
                    self.graph = graph;
                    #[cfg(not(target_arch = "wasm32"))]
                    if doc.value.pointer(&self.pointer).is_none() {
                        self.pointer.clear();
                        self.canvas.focus(String::new());
                    }
                    self.edit_dialog = None;
                    self.auto_due = None;
                    self.error = false;
                    if done.user_action {
                        let unchanged = *feedback_before == *doc.text;
                        let message = match done.label {
                            "Validate" => "JSON is valid".to_owned(),
                            "Format" if unchanged => "Already formatted — no changes needed".into(),
                            "Minify" if unchanged => "Already minified — no changes needed".into(),
                            "Open file" => "File opened".into(),
                            _ if unchanged => {
                                format!("{} complete — no changes needed", done.label)
                            }
                            _ => format!("{} complete", done.label),
                        };
                        self.notify(
                            feedback::Kind::Success,
                            format!("{message} · {} bytes", doc.text.len()),
                        );
                    }
                    self.full_text = doc.text.clone();
                    self.reset_search();
                    self.editable =
                        doc.text.len() <= EDIT_LIMIT || (self.editable && !done.reset_view);
                    self.editor = if self.editable {
                        (*doc.text).clone()
                    } else {
                        preview(&doc.text, PREVIEW_LIMIT).into()
                    };
                    self.dirty = false;
                    self.clear_comparison();
                    self.document = Some(Arc::new(doc));
                    self.status = format!(
                        "Valid JSON · {} bytes · {:.1} ms",
                        self.full_text.len(),
                        done.elapsed.as_secs_f64() * 1000.0
                    );
                }
                Ok(Output::InvalidComparison(left, right)) => {
                    self.syntax_error = left;
                    self.right_syntax_error = right;
                    self.differences.clear();
                    self.compare_alignment = None;
                    self.comparison_done = false;
                    self.compare_left_range = None;
                    self.compare_right_range = None;
                    self.compare_jump = false;
                    self.compare_due = None;
                    self.error = true;
                    self.status = [
                        self.syntax_error
                            .as_ref()
                            .map(|error| format!("Left JSON: {}", error.message)),
                        self.right_syntax_error
                            .as_ref()
                            .map(|error| format!("Right JSON: {}", error.message)),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                    if done.user_action {
                        self.notify(feedback::Kind::Error, self.status.clone());
                    }
                }
                Ok(Output::Comparison(diffs, alignment, left, right)) => {
                    self.syntax_error = None;
                    self.right_syntax_error = None;
                    self.error = false;
                    self.comparison_done = true;
                    self.compare_alignment = Some(alignment);
                    self.status = if diffs.is_empty() {
                        "Documents are equal".into()
                    } else {
                        format!(
                            "{} differences{} · {:.1} ms",
                            diffs.len(),
                            if diffs.len() == 1000 {
                                " (report limit reached)"
                            } else {
                                ""
                            },
                            done.elapsed.as_secs_f64() * 1000.0
                        )
                    };
                    if done.user_action {
                        self.notify(feedback::Kind::Success, self.status.clone());
                    }
                    self.differences = diffs;
                    self.compare_selected = 0;
                    if done.user_action {
                        self.apply_diff_location(left, right);
                        self.view = View::Compare;
                    } else {
                        self.compare_left_range = left;
                        self.compare_right_range = right;
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                Ok(Output::Saved) => {
                    self.error = false;
                    self.status = "File saved".into();
                    if done.user_action {
                        self.notify(feedback::Kind::Success, self.status.clone());
                    }
                }
                Ok(Output::Invalid(text, error)) => {
                    self.syntax_error = Some(syntax::SyntaxError::new(&error, &text));
                    self.editable = text.len() <= EDIT_LIMIT || (self.editable && !done.reset_view);
                    self.editor = if self.editable {
                        (*text).clone()
                    } else {
                        preview(&text, PREVIEW_LIMIT).into()
                    };
                    self.full_text = text;
                    self.reset_search();
                    if !self.editable
                        && let Some(offset) =
                            self.syntax_error.as_ref().and_then(|error| error.offset)
                    {
                        let mut start = offset.saturating_sub(4096);
                        while !self.full_text.is_char_boundary(start) {
                            start -= 1;
                        }
                        self.preview_line = self.full_text[..start]
                            .bytes()
                            .filter(|byte| *byte == b'\n')
                            .count()
                            + 1;
                        self.preview_start = start;
                        self.editor = preview(&self.full_text[start..], PREVIEW_LIMIT).into();
                    }
                    self.dirty = false;
                    self.document = None;
                    self.clear_comparison();
                    self.error = true;
                    self.status = error;
                    if done.user_action {
                        self.notify(feedback::Kind::Error, self.status.clone());
                    }
                }
                Ok(Output::Located(range)) => {
                    self.error = false;
                    self.status = "Source selection located".into();
                    self.source_target = Some(range);
                    self.search.jump = true;
                }
                Ok(Output::SourcePointer(pointer)) => {
                    if self.view == View::Graph
                        && self.pending_source_offset.is_none()
                        && let Some(pointer) = pointer
                    {
                        self.pointer = pointer.clone();
                        self.view = View::Graph;
                        self.canvas.focus(pointer);
                    }
                }
                Ok(Output::FocusedGraph(graph, pointer)) => {
                    if self.view == View::Graph && self.pointer == pointer {
                        self.canvas.reconcile(&self.graph, &graph);
                        self.graph = graph;
                        self.canvas.focus(pointer);
                    }
                }
                Ok(Output::RightText(text)) => {
                    self.right_syntax_error = None;
                    self.compare_text = (*text).clone();
                    self.search.right.changed();
                    self.clear_comparison();
                    self.notify(feedback::Kind::Success, "Right JSON formatted");
                }
                Ok(Output::DiffLocation(left, right)) => self.apply_diff_location(left, right),
                Ok(Output::Editing(dialog)) => {
                    self.error = false;
                    self.status = "Editing selected node".into();
                    self.edit_dialog = Some(dialog);
                }
                Err(error) => {
                    if let Some(message) = error.strip_prefix("Left JSON: ") {
                        self.syntax_error =
                            Some(syntax::SyntaxError::new(message, &self.full_text));
                    } else if let Some(message) = error.strip_prefix("Right JSON: ") {
                        self.right_syntax_error =
                            Some(syntax::SyntaxError::new(message, &self.compare_text));
                    } else if done.label == "Format right JSON" {
                        self.right_syntax_error =
                            Some(syntax::SyntaxError::new(&error, &self.compare_text));
                    }
                    if let Some(dialog) = &mut self.edit_dialog {
                        dialog.error = error.clone();
                    }
                    self.error = true;
                    self.status = error;
                    if done.user_action {
                        self.notify(feedback::Kind::Error, self.status.clone());
                    }
                }
            }
        }
        self.dispatch_next();
    }
}
