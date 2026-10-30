#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use web_time::Instant;
mod app;
#[cfg(target_arch = "wasm32")]
mod browser;
mod canvas;
#[cfg(all(not(target_arch = "wasm32"), feature = "bench-ui"))]
#[path = "../bench/editor.rs"]
mod editor_bench;
mod ui;
mod worker;

use app::{find, jobs};
use eframe::egui;
use jobs::*;
use json_lens::{
    Difference, Document, EDIT_LIMIT, Format, PREVIEW_LIMIT, compare, csv_to_json, format_document,
    preview, process,
};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};
use ui::{decode_ui, feedback};
#[cfg(target_arch = "wasm32")]
use worker::{web_tasks, web_worker};

#[derive(PartialEq)]
enum View {
    Graph,
    Compare,
}

#[derive(Clone, Copy, PartialEq)]
enum SortState {
    Original,
    Ascending,
    Descending,
}

impl SortState {
    fn after_single_click(self) -> Self {
        match self {
            Self::Original => Self::Ascending,
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Original,
        }
    }
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
    #[cfg(not(target_arch = "wasm32"))]
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
    syntax_error: Option<crate::jobs::SyntaxError>,
    right_syntax_error: Option<crate::jobs::SyntaxError>,
    error_jump: Option<bool>,
    view: View,
    graph: Arc<json_lens::graph::Graph>,
    canvas: canvas::Canvas,
    graph_wide: bool,
    split_ratio: f32,
    active_right: bool,
    sort_state: SortState,
    sort_original: Option<String>,
    compare_text: String,
    right_json_valid: bool,
    compare_code: ui::CodeCache,
    compare_search_source: Arc<String>,
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

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    json_lens::diagnostics::init();
    #[cfg(feature = "bench-ui")]
    if std::env::args().any(|arg| arg == "--bench-editor") {
        editor_bench::run();
        return Ok(());
    }
    let viewport = egui::ViewportBuilder::default()
        .with_app_id("json-lens")
        .with_title("JSON Lens")
        .with_inner_size(if std::env::var_os("JSON_LENS_NARROW").is_some() {
            [900.0, 650.0]
        } else {
            [1280.0, 800.0]
        })
        .with_min_inner_size([800.0, 500.0])
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!(
                "../packaging/icons/json-lens-256.png"
            ))
            .expect("bundled application icon must be a valid PNG"),
        );
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_fullsize_content_view(false)
        .with_titlebar_shown(true)
        .with_title_shown(true);
    eframe::run_native(
        "JSON Lens",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    json_lens::diagnostics::init();
    if web_sys::window().is_some() {
        browser::start();
    }
}
