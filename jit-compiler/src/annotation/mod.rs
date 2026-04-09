//! Structured Modelica annotation parser.

mod parse;
mod types;

pub use parse::{format_icon_diagram_record, parse_annotation, parse_dialog, parse_icon, parse_placement};
pub use types::*;
