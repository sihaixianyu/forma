use super::*;

impl App {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
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

    pub(crate) fn create(ctx: egui::Context, gpu: String) -> Self {
        ui::install_fonts(&ctx);
        ctx.options_mut(|options| options.fallback_theme = egui::Theme::Light);
        let (sender, completed) = mpsc::channel();
        let jobs = worker::spawn(ctx.clone(), sender, false, |work: Work| {
            log::debug!(target: "json_lens", "event=work_metadata revision={} user_action={} reset_view={}", work.revision, work.user_action, work.reset_view);
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
            syntax_error: None,
            right_syntax_error: None,
            error_jump: None,
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
            compare_search_source: Arc::new(String::new()),
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
}
