//! Close-to-format document metadata shared by KiCad board and schematic files.

use std::collections::BTreeMap;
use std::ops::Range;

/// A KiCad paper declaration.
///
/// Boards and schematics use the same scalar layout: a named size, optional
/// custom width and height, and an optional `portrait` flag.
#[derive(Clone, Debug, PartialEq)]
pub struct KiCadPaper {
    pub size: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub portrait: bool,
    pub source_range: Option<Range<usize>>,
}

/// A KiCad drawing-sheet title block shared by boards and schematics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiCadTitleBlock {
    pub title: String,
    pub date: String,
    pub revision: String,
    pub company: String,
    pub comments: BTreeMap<i64, String>,
    pub source_range: Range<usize>,
}
