#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use web_time::Instant;
#[cfg(target_arch = "wasm32")]
mod browser;
mod canvas;
mod decode_ui;
mod feedback;
mod find;
mod ui;
mod worker;

use eframe::egui;
use json_lens::{
    Difference, Document, EDIT_LIMIT, Format, PREVIEW_LIMIT, compare, csv_to_json, format_document,
    preview, process,
};
use serde_json::Value;
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

enum Job {
    RequestFormat(Format),
    Process(Arc<String>, Format),
    Transform(Arc<Document>, Format, Arc<json_lens::graph::Graph>),
    Locate(Arc<Document>, String),
    SourcePointer(Arc<Document>, usize),
    Inspect(Arc<Document>, String),
    Edit(Arc<Document>, String, String, bool),
    #[cfg(not(target_arch = "wasm32"))]
    Open(PathBuf),
    Import(Arc<String>, String),
    Compare(Arc<String>, Arc<String>),
    FormatRight(Arc<String>),
    LocateDiff(Arc<String>, Arc<String>, String),
    #[cfg(not(target_arch = "wasm32"))]
    Save(Arc<String>, PathBuf),
}

impl Job {
    fn label(&self) -> &'static str {
        let format = match self {
            Job::RequestFormat(format) | Job::Process(_, format) | Job::Transform(_, format, _) => {
                Some(format)
            }
            _ => None,
        };
        if let Some(format) = format {
            return match format {
                Format::Keep => "Validate",
                Format::Pretty => "Format",
                Format::Compact => "Minify",
                Format::SortAscending => "Sort ascending",
                Format::SortDescending => "Sort descending",
                Format::Nested => "Expand nested JSON",
            };
        }
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Job::Open(_) => "Open file",
            Job::Import(..) => "Open file",
            Job::Compare(..) => "Compare JSON",
            Job::FormatRight(..) => "Format right JSON",
            Job::LocateDiff(..) => "Locate difference",
            #[cfg(not(target_arch = "wasm32"))]
            Job::Save(..) => "Save JSON",
            Job::Locate(..) => "Locate source",
            Job::SourcePointer(..) => "Locate graph node",
            Job::Inspect(..) => "Inspect node",
            Job::Edit(_, _, _, true) => "Rename property",
            Job::Edit(..) => "Edit node",
            _ => "Process JSON",
        }
    }
}
enum Output {
    Document(Document, Arc<json_lens::graph::Graph>),
    Invalid(Arc<String>, String),
    Comparison(
        Vec<Difference>,
        Arc<json_lens::alignment::Alignment>,
        Option<std::ops::Range<usize>>,
        Option<std::ops::Range<usize>>,
    ),
    RightText(Arc<String>),
    DiffLocation(
        Option<std::ops::Range<usize>>,
        Option<std::ops::Range<usize>>,
    ),
    #[cfg(not(target_arch = "wasm32"))]
    Saved,
    Located(std::ops::Range<usize>),
    SourcePointer(Option<String>),
    Editing(ui::EditDialog),
}
struct Work {
    job: Job,
    revision: u64,
    reset_view: bool,
    label: &'static str,
    user_action: bool,
}

struct QueuedAction {
    job: Job,
    revision: u64,
}

struct Completed {
    target: Option<String>,
    revision: u64,
    reset_view: bool,
    result: Result<Output, String>,
    elapsed: Duration,
    label: &'static str,
    user_action: bool,
}

fn diff_ranges(
    left: &str,
    right: &str,
    pointer: &str,
) -> (
    Option<std::ops::Range<usize>>,
    Option<std::ops::Range<usize>>,
) {
    let pointer = if pointer == "/ (root)" { "" } else { pointer };
    (
        json_lens::edit::source_range(left, pointer).ok(),
        json_lens::edit::source_range(right, pointer).ok(),
    )
}

fn document_output(doc: Document) -> Output {
    let graph = Arc::new(json_lens::graph::build_graph(&doc.value));
    Output::Document(doc, graph)
}

fn execute(job: Job) -> Result<Output, String> {
    match job {
        Job::RequestFormat(_) => Err("Unresolved format request".into()),
        Job::Transform(doc, format, graph) => {
            let formatted = format_document(&doc, format)?;
            if Arc::ptr_eq(&doc.value, &formatted.value) {
                Ok(Output::Document(formatted, graph))
            } else {
                Ok(document_output(formatted))
            }
        }
        Job::Locate(doc, pointer) => {
            json_lens::edit::source_range(&doc.text, &pointer).map(Output::Located)
        }
        Job::SourcePointer(doc, offset) => {
            json_lens::edit::pointer_at_offset(&doc.text, offset).map(Output::SourcePointer)
        }
        Job::Inspect(doc, pointer) => {
            let value = doc.value.pointer(&pointer).ok_or("Pointer not found")?;
            let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
            if json.len() > EDIT_LIMIT {
                return Err("Select a smaller node to edit (maximum 1 MiB)".into());
            }
            let key = pointer.rsplit_once('/').and_then(|(parent, key)| {
                doc.value
                    .pointer(parent)
                    .filter(|value| value.is_object())
                    .map(|_| key.replace("~1", "/").replace("~0", "~"))
            });
            Ok(Output::Editing(ui::EditDialog {
                pointer,
                json,
                key,
                rename: false,
                error: String::new(),
            }))
        }
        Job::Edit(doc, pointer, replacement, rename) => if rename {
            json_lens::edit::rename_key(&doc, &pointer, &replacement)
        } else {
            json_lens::edit::replace_value(&doc, &pointer, &replacement)
        }
        .map(document_output),
        Job::Process(text, format) => Ok(match process(text.clone(), format) {
            Ok(doc) => document_output(doc),
            Err(error) => Output::Invalid(text, error),
        }),
        #[cfg(not(target_arch = "wasm32"))]
        Job::Open(path) => {
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            execute(Job::Import(
                Arc::new(text),
                path.to_string_lossy().into_owned(),
            ))
        }
        Job::Import(text, name) => {
            let text = if name
                .rsplit('.')
                .next()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
            {
                Arc::new(
                    serde_json::to_string_pretty(&csv_to_json(&text)?)
                        .map_err(|e| e.to_string())?,
                )
            } else {
                text
            };
            Ok(match process(text.clone(), Format::Keep) {
                Ok(doc) => document_output(doc),
                Err(error) => Output::Invalid(text, error),
            })
        }
        Job::Compare(left, right) => {
            let a: Value = serde_json::from_str(&left).map_err(|e| format!("Left JSON: {e}"))?;
            let b: Value = serde_json::from_str(&right).map_err(|e| format!("Right JSON: {e}"))?;
            let diffs = compare(&a, &b, 1000);
            let (a, b) = diffs
                .first()
                .map(|d| diff_ranges(&left, &right, &d.pointer))
                .unwrap_or_default();
            Ok(Output::Comparison(
                diffs,
                Arc::new(json_lens::alignment::align(&left, &right)),
                a,
                b,
            ))
        }
        Job::FormatRight(text) => {
            process(text, Format::Pretty).map(|doc| Output::RightText(doc.text))
        }
        Job::LocateDiff(left, right, pointer) => {
            let (a, b) = diff_ranges(&left, &right, &pointer);
            Ok(Output::DiffLocation(a, b))
        }
        #[cfg(not(target_arch = "wasm32"))]
        Job::Save(text, path) => {
            std::fs::write(path, text.as_bytes()).map_err(|e| e.to_string())?;
            Ok(Output::Saved)
        }
    }
}

#[derive(PartialEq)]
enum View {
    Graph,
    Compare,
}

struct App {
    #[cfg(not(target_arch = "wasm32"))]
    smoke_test: bool,
    #[cfg(not(target_arch = "wasm32"))]
    smoke_frames: usize,
    #[cfg(not(target_arch = "wasm32"))]
    smoke_capture_requested: bool,
    editor: String,
    full_text: Arc<String>,
    document: Option<Arc<Document>>,
    editable: bool,
    dirty: bool,
    busy: bool,
    status: String,
    gpu_info: String,
    filename: String,
    dark: bool,
    theme_preference: egui::ThemePreference,
    code: ui::CodeCache,
    search: find::Search,
    search_source: Arc<String>,
    preview_start: usize,
    preview_line: usize,
    error: bool,
    view: View,
    graph: Arc<json_lens::graph::Graph>,
    canvas: canvas::Canvas,
    graph_wide: bool,
    split_ratio: f32,
    active_right: bool,
    sort_descending: bool,
    compare_text: String,
    compare_code: ui::CodeCache,
    compare_alignment: Option<Arc<json_lens::alignment::Alignment>>,
    compare_scroll: f32,
    pane_content_top: f32,
    right_header_height: f32,
    source_header_height: f32,
    compare_selected: usize,
    compare_left_range: Option<std::ops::Range<usize>>,
    compare_right_range: Option<std::ops::Range<usize>>,
    compare_jump: bool,
    pointer: String,
    differences: Vec<Difference>,
    comparison_done: bool,
    #[cfg(target_arch = "wasm32")]
    files: browser::Files,
    jobs: worker::Sender<Work>,
    revision: u64,
    auto_due: Option<Instant>,
    compare_due: Option<Instant>,
    automatic_job: bool,
    pending_reveal: Option<String>,
    pending_source_offset: Option<usize>,
    source_target: Option<std::ops::Range<usize>>,
    edit_dialog: Option<ui::EditDialog>,
    feedback: Option<feedback::Feedback>,
    queued_actions: std::collections::VecDeque<QueuedAction>,
    completed: mpsc::Receiver<Completed>,
}

impl App {
    fn set_theme_preference(&mut self, ctx: &egui::Context, preference: egui::ThemePreference) {
        self.theme_preference = preference;
        self.sync_theme(ctx);
    }

    fn sync_theme(&mut self, ctx: &egui::Context) {
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
    fn clear_comparison(&mut self) {
        self.differences.clear();
        self.compare_alignment = None;
        self.comparison_done = false;
        self.compare_left_range = None;
        self.compare_right_range = None;
        self.compare_jump = false;
        self.compare_due = (self.view == View::Compare && !self.compare_text.trim().is_empty())
            .then(|| Instant::now() + Duration::from_millis(300));
    }

    fn tick_automatic_jobs(&mut self, now: Instant) {
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
    fn apply_diff_location(
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

    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let gpu = cc
            .wgpu_render_state
            .as_ref()
            .map(|state| {
                let info = state.adapter.get_info();
                format!("{:?} · {}", info.backend, info.name)
            })
            .unwrap_or_else(|| "wgpu".into());
        Self::create(cc.egui_ctx.clone(), gpu)
    }

    fn create(ctx: egui::Context, gpu: String) -> Self {
        ui::install_fonts(&ctx);
        ctx.options_mut(|options| options.fallback_theme = egui::Theme::Light);
        let (sender, completed) = mpsc::channel();
        let jobs = worker::spawn(ctx.clone(), sender, false, |work: Work| {
            let start = Instant::now();
            let target = match &work.job {
                Job::Edit(_, pointer, key, true) => pointer
                    .rsplit_once('/')
                    .map(|(parent, _)| json_lens::pointer_child(parent, key)),
                _ => None,
            };
            let result = execute(work.job);
            Completed {
                target,
                label: work.label,
                user_action: work.user_action,
                revision: work.revision,
                reset_view: work.reset_view,
                result,
                elapsed: start.elapsed(),
            }
        });
        let text = Arc::new(
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "JSON Lens",
                "version": "0.1.0",
                "workspace": {
                    "theme": "light",
                    "autosave": false,
                    "features": ["Graph explorer", "Compare"]
                },
                "items": [
                    {"id": 1, "name": "Getting started", "status": "ready", "size": 1024},
                    {"id": 2, "name": "Your next document", "status": "draft", "size": 2048}
                ],
                "encoded": {
                    "base64": "SGVsbG8sIEpTT04gTGVucyE=",
                    "base64_chinese": "5Lit5paH",
                    "url": "Hello%2C%20JSON%20Lens%21",
                    "hex": "48656c6c6f2c204a534f4e204c656e7321"
                },
                "updated": null
            }))
            .expect("valid example"),
        );
        let mut app = Self {
            #[cfg(not(target_arch = "wasm32"))]
            smoke_test: cfg!(not(target_arch = "wasm32"))
                && std::env::args().any(|arg| arg == "--smoke-test"),
            #[cfg(not(target_arch = "wasm32"))]
            smoke_frames: 0,
            #[cfg(not(target_arch = "wasm32"))]
            smoke_capture_requested: false,
            editor: (*text).clone(),
            full_text: text.clone(),
            search_source: text,
            search: find::Search::new(ctx.clone()),
            preview_start: 0,
            preview_line: 1,
            document: None,
            editable: true,
            dirty: false,
            busy: false,
            status: String::new(),
            filename: "Untitled.json".into(),
            dark: ctx.system_theme().unwrap_or(egui::Theme::Light) == egui::Theme::Dark,
            theme_preference: egui::ThemePreference::System,
            code: ui::CodeCache::default(),
            error: false,
            gpu_info: gpu,
            view: View::Graph,
            graph: Arc::new(json_lens::graph::Graph::default()),
            canvas: canvas::Canvas::default(),
            graph_wide: false,
            split_ratio: 0.5,
            active_right: false,
            sort_descending: false,
            compare_text: String::new(),
            compare_code: ui::CodeCache::default(),
            compare_alignment: None,
            compare_scroll: 0.0,
            pane_content_top: 0.0,
            right_header_height: 0.0,
            source_header_height: 0.0,
            compare_selected: 0,
            compare_left_range: None,
            compare_right_range: None,
            compare_jump: false,
            pointer: String::new(),
            differences: Vec::new(),
            comparison_done: false,
            #[cfg(target_arch = "wasm32")]
            files: browser::Files::new(ctx.clone()),
            jobs,
            revision: 0,
            auto_due: None,
            compare_due: None,
            automatic_job: false,
            pending_reveal: None,
            pending_source_offset: None,
            source_target: None,
            edit_dialog: None,
            feedback: None,
            queued_actions: std::collections::VecDeque::new(),
            completed,
        };
        #[cfg(target_arch = "wasm32")]
        app.parse(Format::Keep);
        #[cfg(not(target_arch = "wasm32"))]
        if cfg!(test) {
            app.parse(Format::Keep);
        } else if let Some(path) = std::env::args_os()
            .skip(1)
            .find(|arg| arg != "--smoke-test")
        {
            let path = PathBuf::from(path);
            app.filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into();
            app.submit(Job::Open(path));
        } else {
            app.parse(Format::Keep);
        }
        ui::configure(&ctx, app.dark);
        ctx.set_theme(app.theme_preference);
        app
    }

    fn notify(&mut self, kind: feedback::Kind, text: impl Into<String>) {
        self.feedback = Some(feedback::Feedback::new(kind, text));
    }

    fn submit(&mut self, job: Job) {
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

    fn start(&mut self, job: Job, user_action: bool) {
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
            Job::Locate(..) | Job::Inspect(..) | Job::SourcePointer(..) | Job::LocateDiff(..)
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

    fn dispatch_next(&mut self) {
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

    fn source(&mut self) -> Arc<String> {
        if self.dirty {
            self.full_text = Arc::new(self.editor.clone());
        }
        self.full_text.clone()
    }

    fn parse(&mut self, format: Format) {
        self.submit(Job::RequestFormat(format));
    }

    fn receive(&mut self) {
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
                continue;
            }
            let feedback_before = self.full_text.clone();
            match done.result {
                Ok(Output::Document(doc, graph)) => {
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
                    self.editable = doc.text.len() <= EDIT_LIMIT;
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
                Ok(Output::Comparison(diffs, alignment, left, right)) => {
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
                    self.editable = text.len() <= EDIT_LIMIT;
                    self.editor = if self.editable {
                        (*text).clone()
                    } else {
                        preview(&text, PREVIEW_LIMIT).into()
                    };
                    self.full_text = text;
                    self.reset_search();
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
                Ok(Output::RightText(text)) => {
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_theme(ctx);
        self.receive();
        self.tick_automatic_jobs(Instant::now());
        self.search.tick(
            ctx,
            self.search_source.clone(),
            if self.dirty {
                None
            } else {
                self.document.as_ref().map(|doc| doc.value.clone())
            },
            if self.search.right.pending {
                Arc::new(self.compare_text.clone())
            } else {
                Arc::new(String::new())
            },
        );
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

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "JSON Lens",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(if std::env::var_os("JSON_LENS_NARROW").is_some() {
                    [900.0, 650.0]
                } else {
                    [1280.0, 800.0]
                })
                .with_min_inner_size([800.0, 500.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    browser::start();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_browser_text_uses_the_same_json_and_csv_processing() {
        let json: Arc<String> = Arc::new("{\"中文\":123456789012345678901234567890}".into());
        let Output::Document(doc, _) =
            execute(Job::Import(json.clone(), "example.json".into())).unwrap()
        else {
            panic!("document expected");
        };
        assert_eq!(*doc.text, *json);
        let Output::Document(doc, _) = execute(Job::Import(
            Arc::new("name,value\n中文,42\n".into()),
            "example.CSV".into(),
        ))
        .unwrap() else {
            panic!("document expected");
        };
        assert_eq!(doc.value[0]["name"], "中文");
        assert_eq!(doc.value[0]["value"], "42");
        assert!(matches!(
            execute(Job::Import(Arc::new("{bad}".into()), "bad.json".into())).unwrap(),
            Output::Invalid(..)
        ));
    }

    fn ready_app() -> App {
        let mut app = App::create(egui::Context::default(), "test".into());
        let done = app.completed.recv_timeout(Duration::from_secs(3)).unwrap();
        let (sender, receiver) = mpsc::channel();
        sender.send(done).unwrap();
        app.completed = receiver;
        app.receive();
        app
    }

    #[test]
    fn formatting_reuses_graph_and_retains_navigation() {
        let doc = Arc::new(process(Arc::new("{\"b\":2,\"a\":1}".into()), Format::Keep).unwrap());
        let graph = Arc::new(json_lens::graph::build_graph(&doc.value));
        let Output::Document(formatted, reused) =
            execute(Job::Transform(doc.clone(), Format::Pretty, graph.clone())).unwrap()
        else {
            panic!("document expected");
        };
        assert!(Arc::ptr_eq(&graph, &reused));
        assert!(Arc::ptr_eq(&doc.value, &formatted.value));
        let Output::Document(_, sorted) =
            execute(Job::Transform(doc, Format::SortAscending, graph.clone())).unwrap()
        else {
            panic!("document expected");
        };
        assert!(!Arc::ptr_eq(&graph, &sorted));
    }

    #[test]
    fn stale_auto_parse_cannot_overwrite_more_recent_input() {
        let mut app = ready_app();
        app.revision = 2;
        app.editor = "{\"new\":true}".into();
        app.search_source = Arc::new(app.editor.clone());
        app.dirty = true;
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Completed {
                label: "Validate",
                user_action: false,
                target: None,
                revision: 1,
                reset_view: false,
                result: execute(Job::Process(
                    Arc::new("{\"old\":true}".into()),
                    Format::Keep,
                )),
                elapsed: Duration::ZERO,
            })
            .unwrap();
        app.completed = receiver;
        app.receive();
        assert_eq!(app.editor, "{\"new\":true}");
        assert!(app.dirty);
    }

    fn finish(app: &mut App, work: Work) {
        let result = execute(work.job);
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Completed {
                target: None,
                revision: work.revision,
                reset_view: work.reset_view,
                label: work.label,
                user_action: work.user_action,
                result,
                elapsed: Duration::ZERO,
            })
            .unwrap();
        app.completed = receiver;
        app.receive();
    }

    #[test]
    fn actions_clicked_during_auto_refresh_run_in_order_on_the_latest_document() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        app.busy = true;
        app.automatic_job = true;
        app.parse(Format::SortAscending);
        app.parse(Format::Compact);
        assert_eq!(app.queued_actions.len(), 2);
        assert!(app.feedback.as_ref().unwrap().text.contains("queued"));
        app.busy = false;
        app.automatic_job = false;
        app.dispatch_next();
        let sort = receiver.recv().unwrap();
        assert_eq!(sort.label, "Sort ascending");
        finish(&mut app, sort);
        let sorted_keys: Vec<_> = app
            .document
            .as_ref()
            .unwrap()
            .value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let minify = receiver.recv().unwrap();
        assert_eq!(minify.label, "Minify");
        finish(&mut app, minify);
        let keys: Vec<_> = app
            .document
            .as_ref()
            .unwrap()
            .value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, sorted_keys);
        assert!(!app.full_text.contains('\n'));
        assert_eq!(app.feedback.as_ref().unwrap().kind, feedback::Kind::Success);
    }

    #[test]
    fn formatting_unchanged_content_still_reports_success() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        app.parse(Format::Pretty);
        finish(&mut app, receiver.recv().unwrap());
        assert!(
            app.feedback
                .as_ref()
                .unwrap()
                .text
                .contains("Already formatted")
        );
    }

    #[test]
    fn failed_validation_reports_a_visible_error() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        app.editor = "{bad}".into();
        app.dirty = true;
        app.parse(Format::Keep);
        finish(&mut app, receiver.recv().unwrap());
        let feedback = app.feedback.as_ref().unwrap();
        assert_eq!(feedback.kind, feedback::Kind::Error);
        assert!(feedback.text.contains("line"));
    }

    #[test]
    fn blank_space_at_the_bottom_of_a_tall_editor_remains_editable() {
        let mut app = ready_app();
        app.view = View::Compare;
        app.editor = "{}".into();
        let ctx = egui::Context::default();
        ui::install_fonts(&ctx);
        ui::configure(&ctx, false);
        let position = egui::pos2(300.0, 1050.0);
        for (frame, events) in [
            vec![],
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![egui::Event::Text(" ".into())],
        ]
        .into_iter()
        .enumerate()
        {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 1200.0),
                    )),
                    time: Some(frame as f64 * 0.1),
                    events,
                    ..Default::default()
                },
                |ctx| app.workspace(ctx),
            );
        }
        assert_eq!(app.editor, "{} ");
        assert!(app.dirty);
    }

    #[test]
    fn compare_rows_and_gutters_remain_aligned_when_scrolling_the_right_editor() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        let values: Vec<_> = (0..100).collect();
        app.editor = serde_json::to_string_pretty(&values).unwrap();
        app.dirty = true;
        app.parse(Format::Keep);
        finish(&mut app, receiver.recv().unwrap());
        app.view = View::Compare;
        app.compare_text = app.editor.replacen("  1,\n", "  1,\n  1000,\n  1001,\n", 1);
        app.start(
            Job::Compare(app.full_text.clone(), Arc::new(app.compare_text.clone())),
            false,
        );
        finish(&mut app, receiver.recv().unwrap());
        let ctx = egui::Context::default();
        ui::install_fonts(&ctx);
        ui::configure(&ctx, false);
        for frame in 0..6 {
            let events = if frame == 1 {
                vec![
                    egui::Event::PointerMoved(egui::pos2(900.0, 500.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -120.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                vec![]
            };
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 800.0),
                    )),
                    time: Some(frame as f64 * 0.1),
                    events,
                    ..Default::default()
                },
                |ctx| app.workspace(ctx),
            );
            let text_shapes: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::epaint::Shape::Text(text) = &shape.shape {
                        Some(text)
                    } else {
                        None
                    }
                })
                .collect();
            let left = text_shapes
                .iter()
                .find(|text| text.galley.job.text == app.editor)
                .unwrap();
            let left_clip = output.shapes.iter().find(|shape| {
                matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.job.text == app.editor)
            }).unwrap().clip_rect;
            assert!(left_clip.bottom() > 720.0);
            if frame == 0 {
                assert!(
                    left_clip.top() < 200.0,
                    "editor header reserves too much space"
                );
            }
            let right = text_shapes
                .iter()
                .find(|text| text.galley.job.text == app.compare_text)
                .unwrap();
            // The shared row after the two inserted lines has the same screen position.
            assert!(
                (left.pos.y + left.galley.rows[3].pos.y - right.pos.y - right.galley.rows[5].pos.y)
                    .abs()
                    < 0.5,
                "frame {frame}: left {:?} row {}; right {:?} row {}; shared scroll {}",
                left.pos,
                left.galley.rows[3].pos.y,
                right.pos,
                right.galley.rows[5].pos.y,
                app.compare_scroll
            );
            if frame == 0 {
                let numbers: Vec<_> = text_shapes
                    .iter()
                    .filter(|text| text.galley.job.text == "1")
                    .collect();
                assert_eq!(numbers.len(), 2);
                assert!((numbers[0].pos.y - numbers[1].pos.y).abs() < 0.5);
            }
        }
        assert!(app.compare_scroll > 20.0);
    }

    #[test]
    fn pasted_json_automatically_highlights_without_moving_the_editor_cursor() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        app.view = View::Compare;
        app.compare_text = app.editor.replace("Getting started", "Changed item");
        app.clear_comparison();
        let due = app.compare_due.unwrap();
        app.tick_automatic_jobs(due - Duration::from_millis(1));
        assert!(receiver.try_recv().is_err());
        app.tick_automatic_jobs(due);
        let work = receiver.recv().unwrap();
        assert!(matches!(work.job, Job::Compare(..)));
        assert!(!work.user_action);
        finish(&mut app, work);
        assert!(app.comparison_done);
        assert_eq!(app.differences.len(), 1);
        assert_eq!(
            &app.compare_text[app.compare_right_range.clone().unwrap()],
            "\"Changed item\""
        );
        assert!(app.compare_left_range.is_some());
        assert!(!app.compare_jump && !app.search.jump);
        assert!(app.source_target.is_none());
    }

    #[test]
    fn automatic_comparison_discards_stale_results_and_recovers_from_invalid_input() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        app.view = View::Compare;
        app.compare_text = "{}".into();
        app.clear_comparison();
        app.tick_automatic_jobs(app.compare_due.unwrap());
        let stale = receiver.recv().unwrap();
        app.revision += 1;
        app.compare_text = "{bad}".into();
        app.clear_comparison();
        finish(&mut app, stale);
        assert!(!app.comparison_done && app.compare_right_range.is_none());
        app.tick_automatic_jobs(app.compare_due.unwrap());
        finish(&mut app, receiver.recv().unwrap());
        assert!(app.error && app.status.starts_with("Right JSON:"));
        assert!(app.compare_due.is_none());
        app.revision += 1;
        app.compare_text = app.editor.clone();
        app.clear_comparison();
        app.tick_automatic_jobs(app.compare_due.unwrap());
        finish(&mut app, receiver.recv().unwrap());
        assert!(app.comparison_done && !app.error && app.differences.is_empty());
    }

    #[test]
    fn left_edits_parse_before_refreshing_automatic_comparison() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        app.view = View::Compare;
        app.editor = "{\"value\":2}".into();
        app.search_source = Arc::new(app.editor.clone());
        app.dirty = true;
        app.revision += 1;
        app.compare_text = "{\"value\":3}".into();
        app.clear_comparison();
        app.tick_automatic_jobs(app.compare_due.unwrap());
        let parse = receiver.recv().unwrap();
        assert!(matches!(parse.job, Job::Process(..)));
        finish(&mut app, parse);
        assert!(!app.dirty && !app.comparison_done);
        app.tick_automatic_jobs(app.compare_due.unwrap());
        finish(&mut app, receiver.recv().unwrap());
        assert!(app.comparison_done);
        assert_eq!(&app.editor[app.compare_left_range.clone().unwrap()], "2");
        assert_eq!(
            &app.compare_text[app.compare_right_range.clone().unwrap()],
            "3"
        );
        app.submit(Job::FormatRight(Arc::new(app.compare_text.clone())));
        finish(&mut app, receiver.recv().unwrap());
        app.tick_automatic_jobs(app.compare_due.unwrap());
        finish(&mut app, receiver.recv().unwrap());
        assert_eq!(
            &app.compare_text[app.compare_right_range.clone().unwrap()],
            "3"
        );
    }

    #[test]
    fn pasted_json_comparison_returns_source_ranges_on_both_sides() {
        let left: Arc<String> =
            Arc::new("{\"items\":[{\"name\":\"中文\"}],\"removed\":true}".into());
        let right: Arc<String> =
            Arc::new("{\"items\":[{\"name\":\"updated\"}],\"added\":null}".into());
        let Output::Comparison(diffs, _, a, b) =
            execute(Job::Compare(left.clone(), right.clone())).unwrap()
        else {
            panic!("comparison expected");
        };
        assert_eq!(diffs.len(), 3);
        assert_eq!(&left[a.unwrap()], "\"中文\"");
        assert_eq!(&right[b.unwrap()], "\"updated\"");
        let (a, b) = diff_ranges(&left, &right, "/removed");
        assert_eq!(&left[a.unwrap()], "true");
        assert!(b.is_none());
    }

    #[test]
    fn pasted_json_comparison_reports_equality_and_which_input_is_invalid() {
        let Output::Comparison(diffs, _, _, _) = execute(Job::Compare(
            Arc::new("{\"a\":1,\"b\":2}".into()),
            Arc::new("{\"b\":2,\"a\":1}".into()),
        ))
        .unwrap() else {
            panic!("comparison expected");
        };
        assert!(diffs.is_empty());
        let error = execute(Job::Compare(
            Arc::new("{}".into()),
            Arc::new("{bad}".into()),
        ))
        .err()
        .unwrap();
        assert!(error.starts_with("Right JSON:"));
        let error = execute(Job::Compare(
            Arc::new("{bad}".into()),
            Arc::new("{}".into()),
        ))
        .err()
        .unwrap();
        assert!(error.starts_with("Left JSON:"));
    }

    #[test]
    fn right_editor_formatting_does_not_replace_left_document() {
        let mut app = ready_app();
        let left = app.full_text.clone();
        let (sender, receiver) = mpsc::channel();
        app.jobs = sender.into();
        app.submit(Job::FormatRight(Arc::new("{\"right\":true}".into())));
        finish(&mut app, receiver.recv().unwrap());
        assert_eq!(*app.full_text, *left);
        assert!(app.compare_text.contains('\n'));
        assert!(app.compare_text.contains("right"));
    }

    #[test]
    fn system_theme_is_default_and_manual_choices_override_system_changes() {
        let mut app = ready_app();
        assert_eq!(app.theme_preference, egui::ThemePreference::System);
        let ctx = egui::Context::default();
        for preference in [
            egui::ThemePreference::System,
            egui::ThemePreference::Light,
            egui::ThemePreference::Dark,
            egui::ThemePreference::System,
        ] {
            for theme in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
                let _ = ctx.run(
                    egui::RawInput {
                        system_theme: Some(theme),
                        ..Default::default()
                    },
                    |ctx| {
                        app.set_theme_preference(ctx, preference);
                        app.sync_theme(ctx);
                        let expected = match preference {
                            egui::ThemePreference::System => theme,
                            egui::ThemePreference::Light => egui::Theme::Light,
                            egui::ThemePreference::Dark => egui::Theme::Dark,
                        };
                        assert_eq!(app.dark, expected == egui::Theme::Dark);
                        assert_eq!(ctx.theme(), expected);
                        assert_eq!(ctx.options(|options| options.theme_preference), preference);
                    },
                );
            }
        }
    }

    #[test]
    fn find_shortcut_toggles_only_the_active_pane_and_escape_closes_it() {
        let mut app = ready_app();
        assert!(!app.search.text.visible && !app.search.tree.visible && !app.search.right.visible);
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut key = |app: &mut App, key, modifiers| {
            time += 0.1;
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    modifiers,
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    app.search_shortcuts(ctx);
                },
            );
            time += 0.1;
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    modifiers,
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ctx| app.search_shortcuts(ctx),
            );
        };
        key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
        assert!(app.search.text.visible && app.search.text.focus);
        assert!(!app.search.tree.visible);
        app.search.text.query = "name".into();
        key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
        assert!(!app.search.text.visible && app.search.text.query.is_empty());
        app.active_right = true;
        key(&mut app, egui::Key::F, egui::Modifiers::COMMAND);
        assert!(app.search.tree.visible && app.search.tree.focus);
        key(&mut app, egui::Key::F, egui::Modifiers::COMMAND);
        assert!(!app.search.tree.visible);
        app.view = View::Compare;
        key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
        assert!(app.search.right.visible && app.search.right.focus);
        key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
        assert!(!app.search.right.visible);
        key(&mut app, egui::Key::F, egui::Modifiers::CTRL);
        key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(!app.search.right.visible);
    }

    #[test]
    fn holding_find_shortcut_keeps_search_open() {
        let mut app = ready_app();
        let ctx = egui::Context::default();
        for (frame, repeat) in [false, true, true].into_iter().enumerate() {
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(frame as f64 * 0.1),
                    modifiers: egui::Modifiers::CTRL,
                    events: vec![egui::Event::Key {
                        key: egui::Key::F,
                        physical_key: None,
                        pressed: true,
                        repeat,
                        modifiers: egui::Modifiers::CTRL,
                    }],
                    ..Default::default()
                },
                |ctx| app.search_shortcuts(ctx),
            );
            assert!(app.search.text.visible);
            assert!(!app.search.tree.visible && !app.search.right.visible);
        }
    }

    #[test]
    fn source_click_selects_graph_without_requesting_a_source_reveal_loop() {
        let mut app = ready_app();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Completed {
                target: None,
                revision: app.revision,
                reset_view: false,
                result: Ok(Output::SourcePointer(Some("/items/0/name".into()))),
                elapsed: Duration::ZERO,
                label: "Locate graph node",
                user_action: false,
            })
            .unwrap();
        app.completed = receiver;
        app.receive();
        assert!(matches!(app.view, View::Graph));
        assert_eq!(app.pointer, "/items/0/name");
        assert!(app.pending_reveal.is_none());
    }
}
