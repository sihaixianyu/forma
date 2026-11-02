//! Workspace controls, document panes, editor rendering, search and shared styles.
use crate::app::{App, SortState, View, jobs::Job};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use forma::{Format, preview};
use std::sync::Arc;

mod code;
mod compare;
pub(crate) mod decode_ui;
mod editor;
pub(crate) mod feedback;
mod search;
mod source;
pub(crate) mod style;
mod workspace;

use code::EDITOR_FONT_SIZE;
#[cfg(test)]
use code::EDITOR_LINE_HEIGHT;
pub use code::{CodeCache, EditorTypography};
#[cfg(test)]
use code::{aligned_galley, aligned_wrapped_galley};
use editor::*;
pub use feedback::EditDialog;
use search::{error_shortcut, reveal_error};
use style::*;
pub use style::{configure, install_fonts};

#[cfg(test)]
mod tests;
