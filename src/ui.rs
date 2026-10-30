//! Workspace controls, document panes, editor rendering, search and shared styles.
use super::{App, Job, SortState, View};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use json_lens::{Format, preview};
use std::sync::Arc;

mod code;
mod compare;
#[path = "ui/decode.rs"]
pub(crate) mod decode_ui;
mod editor;
pub(crate) mod feedback;
mod search;
mod source;
mod style;
mod workspace;

pub use code::CodeCache;
use code::EDITOR_FONT_SIZE;
#[cfg(test)]
use code::{aligned_galley, aligned_wrapped_galley};
use editor::*;
pub use feedback::EditDialog;
use search::{error_shortcut, reveal_error};
use style::*;
pub use style::{configure, install_fonts};

#[cfg(test)]
mod tests;
