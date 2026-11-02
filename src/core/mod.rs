//! JSON parsing, transformation, navigation and processing diagnostics.
pub mod alignment;
mod comparison;
pub mod decode;
pub mod diagnostics;
mod document;
pub mod edit;
pub mod graph;
mod parser;
pub mod search;
mod value;

pub use comparison::{Difference, compare};
pub use document::{
    Document, EDIT_LIMIT, Format, PREVIEW_LIMIT, csv_to_json, format_document, process,
};
pub use parser::parse_json;
pub use value::{append_pointer, pointer_child, preview, summary};
