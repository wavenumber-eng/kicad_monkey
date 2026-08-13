//! Close-to-format text metadata shared by KiCad document families.

use std::ops::Range;

/// An optional RGBA color authored in a KiCad font or stroke block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KiCadColor {
    pub red: i64,
    pub green: i64,
    pub blue: i64,
    pub alpha: f64,
}

/// Font fields common to PCB, footprint, schematic, and symbol text.
#[derive(Clone, Debug, PartialEq)]
pub struct KiCadFont {
    pub face: Option<String>,
    pub size_x: f64,
    pub size_y: f64,
    pub thickness: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub line_spacing: Option<f64>,
    pub color: Option<KiCadColor>,
}

impl Default for KiCadFont {
    fn default() -> Self {
        Self {
            face: None,
            size_x: 1.27,
            size_y: 1.27,
            thickness: None,
            bold: false,
            italic: false,
            line_spacing: None,
            color: None,
        }
    }
}

/// Text effects shared across KiCad document families.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KiCadTextEffects {
    pub font: KiCadFont,
    pub justify: Vec<String>,
    pub hidden: bool,
    pub href: Option<String>,
    pub source_range: Option<Range<usize>>,
}
