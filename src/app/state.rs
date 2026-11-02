use super::*;

#[derive(PartialEq)]
pub(crate) enum View {
    Graph,
    Compare,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SortState {
    Original,
    Ascending,
    Descending,
}

impl SortState {
    pub(crate) fn after_single_click(self) -> Self {
        match self {
            Self::Original => Self::Ascending,
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Original,
        }
    }
}

pub(crate) struct App {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) smoke_test: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) smoke_frames: usize,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) smoke_capture_requested: bool,
    pub(crate) editor: String,
    pub(crate) full_text: Arc<String>,
    pub(crate) document: Option<Arc<Document>>,
    pub(crate) editable: bool,
    pub(crate) dirty: bool,
    pub(crate) busy: bool,
    pub(crate) status: String,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) gpu_info: String,
    pub(crate) filename: String,
    pub(crate) dark: bool,
    pub(crate) theme_preference: egui::ThemePreference,
    pub(crate) editor_typography: ui::EditorTypography,
    pub(crate) code: ui::CodeCache,
    pub(crate) search: find::Search,
    pub(crate) search_source: Arc<String>,
    pub(crate) preview_start: usize,
    pub(crate) preview_line: usize,
    pub(crate) error: bool,
    pub(crate) syntax_error: Option<jobs::SyntaxError>,
    pub(crate) right_syntax_error: Option<jobs::SyntaxError>,
    pub(crate) error_jump: Option<bool>,
    pub(crate) view: View,
    pub(crate) graph: Arc<forma::graph::Graph>,
    pub(crate) canvas: canvas::Canvas,
    pub(crate) graph_wide: bool,
    pub(crate) split_ratio: f32,
    pub(crate) active_right: bool,
    pub(crate) sort_state: SortState,
    pub(crate) sort_original: Option<String>,
    pub(crate) compare_text: String,
    pub(crate) right_json_valid: bool,
    pub(crate) compare_code: ui::CodeCache,
    pub(crate) compare_search_source: Arc<String>,
    pub(crate) compare_alignment: Option<Arc<forma::alignment::Alignment>>,
    pub(crate) compare_scroll: f32,
    pub(crate) right_header_height: f32,
    pub(crate) source_header_height: f32,
    pub(crate) compare_selected: usize,
    pub(crate) compare_left_range: Option<std::ops::Range<usize>>,
    pub(crate) compare_right_range: Option<std::ops::Range<usize>>,
    pub(crate) compare_jump: bool,
    pub(crate) pointer: String,
    pub(crate) differences: Vec<Difference>,
    pub(crate) comparison_done: bool,
    #[cfg(target_arch = "wasm32")]
    pub(crate) files: crate::browser::Files,
    pub(crate) jobs: worker::Sender<Work>,
    pub(crate) revision: u64,
    pub(crate) auto_due: Option<Instant>,
    pub(crate) compare_due: Option<Instant>,
    pub(crate) automatic_job: bool,
    pub(crate) pending_reveal: Option<String>,
    pub(crate) pending_source_offset: Option<usize>,
    pub(crate) source_target: Option<std::ops::Range<usize>>,
    pub(crate) edit_dialog: Option<ui::EditDialog>,
    pub(crate) feedback: Option<ui::feedback::Feedback>,
    pub(crate) queued_actions: std::collections::VecDeque<QueuedAction>,
    pub(crate) completed: mpsc::Receiver<Completed>,
}
