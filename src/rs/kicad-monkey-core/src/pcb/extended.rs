//! Remaining top-level PCB collections and focused shared-field writeback.

use super::*;
use std::collections::BTreeMap;

const DEFAULT_VERSION: i64 = 20_260_206;
const DEFAULT_GENERATOR: &str = "pcbnew";
const DEFAULT_GENERATOR_VERSION: &str = "10.0";
const DEFAULT_PAPER: &str = "A4";
const DEFAULT_THICKNESS: f64 = 1.6;

#[derive(Clone, Debug)]
pub(super) struct IndexedTable {
    span: FormSpan,
    cell_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbBoardMetadata {
    pub version: i64,
    pub generator: String,
    pub generator_version: String,
    pub paper: String,
    pub thickness: f64,
    pub legacy_teardrops: bool,
    pub embedded_fonts: bool,
    pub pad_to_mask_clearance: f64,
    pub pad_to_paste_clearance: f64,
    pub pad_to_paste_clearance_ratio: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbBoardVariant {
    pub name: String,
    pub description: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbImage {
    pub at: PcbPoint,
    pub scale: f64,
    pub layer: String,
    pub locked: bool,
    pub encoded_data_bytes: usize,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbBarcode {
    pub at: PcbPoint,
    pub angle: f64,
    pub layer: String,
    pub width: f64,
    pub height: f64,
    pub text: String,
    pub text_height: f64,
    pub kind: String,
    pub ecc_level: Option<String>,
    pub locked: bool,
    pub show_text: bool,
    pub knockout: bool,
    pub margins: PcbPoint,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbTable {
    pub column_count: i64,
    pub layer: String,
    pub border_external: bool,
    pub border_header: bool,
    pub separator_rows: bool,
    pub separator_columns: bool,
    pub column_widths: Vec<f64>,
    pub row_heights: Vec<f64>,
    pub cell_count: usize,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbTableCell {
    pub table_index: usize,
    pub text: String,
    pub start: PcbPoint,
    pub end: PcbPoint,
    pub margins: [f64; 4],
    pub column_span: i64,
    pub row_span: i64,
    pub angle: f64,
    pub layer: String,
    pub locked: bool,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

impl<'a> PcbView<'a> {
    /// Return the board paper declaration, or KiCad Monkey's Python-compatible
    /// A4 default when the form is absent.
    pub fn paper(&self) -> Result<KiCadPaper, Error> {
        let Some(span) = self.first_top_level("paper") else {
            return Ok(KiCadPaper {
                size: DEFAULT_PAPER.to_owned(),
                width: None,
                height: None,
                portrait: false,
                source_range: None,
            });
        };
        let values = bounded_scalar_values(self.source, span, 4)?;
        let size = values
            .first()
            .map(token_string)
            .unwrap_or_else(|| DEFAULT_PAPER.to_owned());
        let mut numeric = values
            .iter()
            .skip(1)
            .filter(|value| token_string(value) != "portrait");
        let width = numeric
            .next()
            .map(|value| parse_f64(value, span))
            .transpose()?;
        let height = numeric
            .next()
            .map(|value| parse_f64(value, span))
            .transpose()?;
        Ok(KiCadPaper {
            size,
            width,
            height,
            portrait: values.iter().any(|value| token_string(value) == "portrait"),
            source_range: Some(span.range.clone()),
        })
    }

    /// Decode the first board title block using the same close-to-format
    /// record that the schematic reader will consume.
    pub fn title_block(&self) -> Result<Option<KiCadTitleBlock>, Error> {
        self.first_top_level("title_block")
            .map(|span| title_block_from_span(self.source, span, self.limits))
            .transpose()
    }

    pub fn metadata(&self) -> Result<PcbBoardMetadata, Error> {
        let general = self.first_top_level("general");
        let setup = self.first_top_level("setup");
        Ok(PcbBoardMetadata {
            version: self.top_level_i64("version")?.unwrap_or(DEFAULT_VERSION),
            generator: self
                .top_level_string("generator")?
                .unwrap_or_else(|| DEFAULT_GENERATOR.to_owned()),
            generator_version: self
                .top_level_string("generator_version")?
                .unwrap_or_else(|| DEFAULT_GENERATOR_VERSION.to_owned()),
            paper: self.paper()?.size,
            thickness: nested_f64(self.source, general, "thickness", self.limits)?
                .unwrap_or(DEFAULT_THICKNESS),
            legacy_teardrops: nested_bool(self.source, general, "legacy_teardrops", self.limits)?,
            embedded_fonts: self
                .top_level_string("embedded_fonts")?
                .is_some_and(|value| value == "yes"),
            pad_to_mask_clearance: nested_f64(
                self.source,
                setup,
                "pad_to_mask_clearance",
                self.limits,
            )?
            .unwrap_or(0.0),
            pad_to_paste_clearance: nested_f64(
                self.source,
                setup,
                "pad_to_paste_clearance",
                self.limits,
            )?
            .unwrap_or(0.0),
            pad_to_paste_clearance_ratio: nested_f64(
                self.source,
                setup,
                "pad_to_paste_clearance_ratio",
                self.limits,
            )?
            .unwrap_or(0.0),
        })
    }

    pub fn variants(&self) -> impl Iterator<Item = Result<PcbBoardVariant, Error>> + '_ {
        self.variants
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Variants))
            .map(|span| board_variant_from_span(self.source, span, self.limits))
    }

    pub fn images(&self) -> impl Iterator<Item = Result<PcbImage, Error>> + '_ {
        self.images
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Images))
            .map(|span| image_from_span(self.source, span, self.limits))
    }

    pub fn barcodes(&self) -> impl Iterator<Item = Result<PcbBarcode, Error>> + '_ {
        self.barcodes
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Barcodes))
            .map(|span| barcode_from_span(self.source, span, self.limits))
    }

    pub fn tables(&self) -> impl Iterator<Item = Result<PcbTable, Error>> + '_ {
        self.tables
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Tables))
            .map(|table| table_from_span(self.source, table, self.limits))
    }

    pub fn table_cells(&self) -> impl Iterator<Item = Result<PcbTableCell, Error>> + '_ {
        self.table_cells
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Tables))
            .map(|indexed| table_cell_from_span(self.source, indexed, self.limits))
    }

    /// Replace the existing layer of one uniquely identified top-level form.
    pub fn set_top_level_layer_by_id(
        &self,
        identifier: &str,
        layer: &str,
    ) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        if identifier.is_empty() {
            return Err(source_error(
                "PCB object identifier cannot be empty",
                self.root.start,
            ));
        }
        let matches = self
            .top_level
            .iter()
            .map(|span| Ok((span, top_level_identifier(self.source, span, self.limits)?)))
            .filter_map(|result| match result {
                Ok((span, Some(found))) if found == identifier => Some(Ok(span)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let [target] = matches.as_slice() else {
            return Err(source_error(
                if matches.is_empty() {
                    "PCB object identifier was not found"
                } else {
                    "PCB object identifier is ambiguous"
                },
                self.root.start,
            ));
        };
        let children = direct_children(
            self.source,
            target,
            self.limits.max_object_children,
            self.limits,
        )?;
        let layers: Vec<_> = children
            .iter()
            .filter(|child| child.head.as_deref() == Some("layer"))
            .collect();
        let [layer_span] = layers.as_slice() else {
            return Err(source_error(
                if layers.is_empty() {
                    "PCB object has no layer field"
                } else {
                    "PCB object layer field is ambiguous"
                },
                target.start,
            ));
        };
        let tokens = scalar_values(self.source, layer_span)?;
        let token = tokens
            .first()
            .ok_or_else(|| source_error("PCB object layer has no value", layer_span.end))?;
        if token_string(token) == layer {
            return Ok(PcbEdit {
                source: self.source.to_owned(),
                changed: false,
            });
        }
        let replacement = build_with_limit(
            &Sexp::Quoted(layer.to_owned()),
            self.limits.max_output_bytes,
        )?;
        Ok(PcbEdit {
            source: apply_patches_with_limit(
                self.source,
                &[Patch::new(
                    layer_span.range.start + token.position.offset,
                    layer_span.range.start + token.position.offset + token.lexeme.len(),
                    replacement,
                )],
                self.limits.max_output_bytes,
            )?,
            changed: true,
        })
    }

    pub(super) fn first_top_level(&self, head: &str) -> Option<&FormSpan> {
        self.top_level
            .iter()
            .find(|span| span.head.as_deref() == Some(head))
    }

    fn top_level_string(&self, head: &str) -> Result<Option<String>, Error> {
        self.first_top_level(head)
            .map(|span| first_string(self.source, span))
            .transpose()
            .map(Option::flatten)
    }

    fn top_level_i64(&self, head: &str) -> Result<Option<i64>, Error> {
        let Some(span) = self.first_top_level(head) else {
            return Ok(None);
        };
        let values = scalar_values(self.source, span)?;
        values
            .first()
            .map(|token| parse_i64(token, span))
            .transpose()
    }
}

fn title_block_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<KiCadTitleBlock, Error> {
    let children = direct_children(source, span, limits.max_title_block_children, limits)?;
    let mut comments = BTreeMap::new();
    for comment in children
        .iter()
        .filter(|child| child.head.as_deref() == Some("comment"))
    {
        let values = bounded_scalar_values(source, comment, 2)?;
        if values.len() < 2 {
            continue;
        }
        let number = parse_i64(&values[0], comment)?;
        if !comments.contains_key(&number) && comments.len() >= limits.max_title_block_comments {
            return Err(limit_error());
        }
        comments.insert(number, token_string(&values[1]));
    }
    Ok(KiCadTitleBlock {
        title: optional_child_string(source, &children, "title")?.unwrap_or_default(),
        date: optional_child_string(source, &children, "date")?.unwrap_or_default(),
        revision: optional_child_string(source, &children, "rev")?.unwrap_or_default(),
        company: optional_child_string(source, &children, "company")?.unwrap_or_default(),
        comments,
        source_range: span.range.clone(),
    })
}

pub(super) fn index_variants(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    let children = direct_children(source, span, limits.max_variants, limits)?;
    for child in children
        .into_iter()
        .filter(|child| child.head.as_deref() == Some("variant"))
    {
        bounded_push(&mut index.variants, child, limits.max_variants)?;
    }
    index.counts.variants = index.variants.len();
    Ok(())
}

pub(super) fn index_table(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    if index.tables.len() >= limits.max_tables {
        return Err(limit_error());
    }
    let table_index = index.tables.len();
    let cell_start = index.table_cells.len();
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let Some(cells) = child(&children, "cells") else {
        bounded_push(
            &mut index.tables,
            IndexedTable {
                span: span.clone(),
                cell_range: cell_start..cell_start,
            },
            limits.max_tables,
        )?;
        index.counts.tables += 1;
        return Ok(());
    };
    for cell in direct_children(source, cells, limits.max_table_cells, limits)?
        .into_iter()
        .filter(|cell| cell.head.as_deref() == Some("table_cell"))
    {
        bounded_push(
            &mut index.table_cells,
            IndexedNestedForm {
                parent_index: table_index,
                span: cell,
            },
            limits.max_table_cells,
        )?;
    }
    let cell_end = index.table_cells.len();
    bounded_push(
        &mut index.tables,
        IndexedTable {
            span: span.clone(),
            cell_range: cell_start..cell_end,
        },
        limits.max_tables,
    )?;
    index.counts.tables += 1;
    index.counts.table_cells = index.table_cells.len();
    Ok(())
}

fn board_variant_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbBoardVariant, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    Ok(PcbBoardVariant {
        name: optional_child_string(source, &children, "name")?.unwrap_or_default(),
        description: optional_child_string(source, &children, "description")?
            .filter(|value| !value.is_empty()),
        source_range: span.range.clone(),
    })
}

fn image_from_span(source: &str, span: &FormSpan, limits: PcbLimits) -> Result<PcbImage, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let at = optional_point(source, &children, "at")?.unwrap_or(PcbPoint { x: 0.0, y: 0.0 });
    Ok(PcbImage {
        at,
        scale: optional_child_f64(source, &children, "scale")?.unwrap_or(1.0),
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "F.SilkS".to_owned()),
        locked: nested_bool(source, Some(span), "locked", limits)?,
        encoded_data_bytes: joined_data_bytes(source, &children, limits)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn barcode_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbBarcode, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let at_values = child_values(source, &children, "at")?;
    let size = pair_or(source, &children, "size", [0.0, 0.0])?;
    let margins = pair_or(source, &children, "margins", [0.0, 0.0])?;
    Ok(PcbBarcode {
        at: PcbPoint {
            x: numeric_at(&at_values, 0, 0.0, span)?,
            y: numeric_at(&at_values, 1, 0.0, span)?,
        },
        angle: numeric_at(&at_values, 2, 0.0, span)?,
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "F.SilkS".to_owned()),
        width: size[0],
        height: size[1],
        text: optional_child_string(source, &children, "text")?.unwrap_or_default(),
        text_height: optional_child_f64(source, &children, "text_height")?.unwrap_or(1.0),
        kind: optional_child_string(source, &children, "type")?
            .unwrap_or_else(|| "code39".to_owned()),
        ecc_level: optional_child_string(source, &children, "ecc_level")?
            .filter(|value| !value.is_empty()),
        locked: nested_bool(source, Some(span), "locked", limits)?,
        show_text: !nested_bool(source, Some(span), "hide", limits)?,
        knockout: nested_bool(source, Some(span), "knockout", limits)?,
        margins: PcbPoint {
            x: margins[0],
            y: margins[1],
        },
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn table_from_span(
    source: &str,
    indexed: &IndexedTable,
    limits: PcbLimits,
) -> Result<PcbTable, Error> {
    let span = &indexed.span;
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let border = child(&children, "border");
    let separators = child(&children, "separators");
    Ok(PcbTable {
        column_count: optional_child_i64(source, &children, "column_count")?.unwrap_or(1),
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "F.Cu".to_owned()),
        border_external: nested_named_bool(source, border, "external", true, limits)?,
        border_header: nested_named_bool(source, border, "header", false, limits)?,
        separator_rows: nested_named_bool(source, separators, "rows", true, limits)?,
        separator_columns: nested_named_bool(source, separators, "cols", true, limits)?,
        column_widths: numeric_list(source, &children, "column_widths", limits)?,
        row_heights: numeric_list(source, &children, "row_heights", limits)?,
        cell_count: indexed.cell_range.len(),
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn table_cell_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbTableCell, Error> {
    let header = scalar_values(source, &indexed.span)?;
    let children = direct_children(source, &indexed.span, limits.max_object_children, limits)?;
    let margins = numeric_array(source, &children, "margins", [0.0; 4])?;
    let span_values = integer_pair(source, &children, "span", [1, 1])?;
    Ok(PcbTableCell {
        table_index: indexed.parent_index,
        text: header.first().map(token_string).unwrap_or_default(),
        start: optional_point(source, &children, "start")?.unwrap_or(PcbPoint { x: 0.0, y: 0.0 }),
        end: optional_point(source, &children, "end")?.unwrap_or(PcbPoint { x: 0.0, y: 0.0 }),
        margins,
        column_span: span_values[0],
        row_span: span_values[1],
        angle: optional_child_f64(source, &children, "angle")?.unwrap_or(0.0),
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "F.Cu".to_owned()),
        locked: nested_bool(source, Some(&indexed.span), "locked", limits)?,
        uuid: optional_uuid(source, &children)?,
        source_range: indexed.span.range.clone(),
    })
}

fn nested_f64(
    source: &str,
    parent: Option<&FormSpan>,
    head: &str,
    limits: PcbLimits,
) -> Result<Option<f64>, Error> {
    let Some(parent) = parent else {
        return Ok(None);
    };
    let children = direct_children(source, parent, limits.max_object_children, limits)?;
    optional_child_f64(source, &children, head)
}

fn nested_bool(
    source: &str,
    parent: Option<&FormSpan>,
    head: &str,
    limits: PcbLimits,
) -> Result<bool, Error> {
    nested_named_bool(source, parent, head, false, limits)
}

fn nested_named_bool(
    source: &str,
    parent: Option<&FormSpan>,
    head: &str,
    default: bool,
    limits: PcbLimits,
) -> Result<bool, Error> {
    let Some(parent) = parent else {
        return Ok(default);
    };
    let children = direct_children(source, parent, limits.max_object_children, limits)?;
    let Some(field) = child(&children, head) else {
        return Ok(false);
    };
    let values = scalar_values(source, field)?;
    Ok(values
        .first()
        .is_some_and(|value| matches!(token_string(value).as_str(), "yes" | "true" | "1")))
}

fn optional_point(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<PcbPoint>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let values = scalar_values(source, span)?;
    Ok(Some(PcbPoint {
        x: numeric_at(&values, 0, 0.0, span)?,
        y: numeric_at(&values, 1, 0.0, span)?,
    }))
}

fn child_values<'a>(
    source: &'a str,
    children: &[FormSpan],
    head: &str,
) -> Result<Vec<Token<'a>>, Error> {
    child(children, head)
        .map(|span| scalar_values(source, span))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn numeric_at(
    values: &[Token<'_>],
    index: usize,
    default: f64,
    span: &FormSpan,
) -> Result<f64, Error> {
    optional_f64(values.get(index), span).map(|value| value.unwrap_or(default))
}

fn pair_or(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; 2],
) -> Result<[f64; 2], Error> {
    let Some(span) = child(children, head) else {
        return Ok(default);
    };
    let values = scalar_values(source, span)?;
    Ok([
        numeric_at(&values, 0, default[0], span)?,
        numeric_at(&values, 1, default[1], span)?,
    ])
}

fn numeric_array<const N: usize>(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; N],
) -> Result<[f64; N], Error> {
    let Some(span) = child(children, head) else {
        return Ok(default);
    };
    let values = scalar_values(source, span)?;
    let mut result = default;
    for (index, output) in result.iter_mut().enumerate() {
        *output = numeric_at(&values, index, *output, span)?;
    }
    Ok(result)
}

fn integer_pair(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [i64; 2],
) -> Result<[i64; 2], Error> {
    let Some(span) = child(children, head) else {
        return Ok(default);
    };
    let values = scalar_values(source, span)?;
    Ok([
        values
            .first()
            .map(|value| parse_i64(value, span))
            .transpose()?
            .unwrap_or(default[0]),
        values
            .get(1)
            .map(|value| parse_i64(value, span))
            .transpose()?
            .unwrap_or(default[1]),
    ])
}

fn numeric_list(
    source: &str,
    children: &[FormSpan],
    head: &str,
    limits: PcbLimits,
) -> Result<Vec<f64>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(Vec::new());
    };
    let values = bounded_scalar_values(source, span, limits.max_table_values)?;
    values.iter().map(|value| parse_f64(value, span)).collect()
}

fn joined_data_bytes(
    source: &str,
    children: &[FormSpan],
    limits: PcbLimits,
) -> Result<usize, Error> {
    let Some(data) = child(children, "data") else {
        return Ok(0);
    };
    let values = bounded_scalar_values(source, data, limits.max_image_data_parts)?;
    Ok(values.iter().map(token_string).map(|part| part.len()).sum())
}
