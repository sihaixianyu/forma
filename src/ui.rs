//! Workspace UI, with editor rendering and pane controls in dedicated modules.
use super::{App, Job, View};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use json_lens::{Format, preview};
use std::sync::Arc;

mod cache;
mod compare;
mod diagnostics;
mod edit;
mod editor;
mod explorer;
mod feedback;
mod files;
mod layout;
mod search;
mod source;
mod theme;
mod viewport;
mod widgets;
mod workspace;

pub use cache::CodeCache;
use diagnostics::*;
pub use edit::EditDialog;
use editor::*;
use layout::*;
use theme::{Palette, palette};
pub use theme::{configure, install_fonts};
use viewport::*;
use widgets::*;

#[cfg(test)]
mod tests;
