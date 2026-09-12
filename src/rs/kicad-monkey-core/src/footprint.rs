//! Typed, source-backed views and focused edits for standalone footprints.

#[cfg(feature = "embedded-resource-zstd")]
use crate::embedded_resource::{EmbeddedDecodeLimits, decoded_data};
use crate::embedded_resource::{EmbeddedFile, EmbeddedFileOwner, encoded_data, metadata_from_span};
use crate::pcb::{
    PcbFootprint, PcbGraphic, PcbLimits, PcbModelReference, PcbPad, StandaloneFootprintCounts,
    standalone_footprint_from_span, standalone_graphic_from_span, standalone_model_from_span,
    standalone_pad_from_span,
};
use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Patch, Position, Sexp, Token, TokenKind,
    apply_patches_with_limit, build_with_limit, decode_quoted,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::ops::Range;

/// Resource ceilings for one typed standalone-footprint read or edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FootprintLimits {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_depth: usize,
    pub max_properties: usize,
    pub max_pads: usize,
    pub max_models: usize,
    pub max_graphics: usize,
    pub max_texts: usize,
    pub max_text_boxes: usize,
    pub max_text_carriers: usize,
    pub max_object_nodes: usize,
    pub max_embedded_files: usize,
}

impl Default for FootprintLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_properties: 4096,
            max_pads: 100_000,
            max_models: 100_000,
            max_graphics: 100_000,
            max_texts: 100_000,
            max_text_boxes: 100_000,
            max_text_carriers: 100_000,
            max_object_nodes: 100_000,
            max_embedded_files: 100_000,
        }
    }
}

/// One lazily decoded top-level property backed by the original source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FootprintProperty<'a> {
    pub name: Cow<'a, str>,
    pub value: Cow<'a, str>,
    value_range: Range<usize>,
}

/// A typed standalone-footprint view that retains only selected source spans.
#[derive(Clone, Debug)]
pub struct FootprintView<'a> {
    pub(crate) source: &'a str,
    pub(crate) root: FormSpan,
    pub(crate) properties: Vec<FormSpan>,
    pub(crate) texts: Vec<FormSpan>,
    pub(crate) text_boxes: Vec<FormSpan>,
    pads: Vec<FormSpan>,
    models: Vec<FormSpan>,
    graphics: Vec<FormSpan>,
    embedded_files: Vec<FormSpan>,
    pad_count: usize,
    pub(crate) limits: FootprintLimits,
}

/// Owned standalone-footprint source with bounded read and write behavior.
#[derive(Clone, Debug)]
pub struct FootprintDocument {
    source: String,
    limits: FootprintLimits,
}

impl FootprintDocument {
    /// Validate and own one UTF-8 `.kicad_mod` source buffer.
    pub fn parse(source: String, limits: FootprintLimits) -> Result<Self, Error> {
        FootprintView::parse(&source, limits)?;
        Ok(Self { source, limits })
    }

    /// Read at most the configured source ceiling plus one sentinel byte.
    pub fn from_reader(mut reader: impl Read, limits: FootprintLimits) -> Result<Self, Error> {
        let read_limit = limits
            .max_source_bytes
            .checked_add(1)
            .ok_or_else(limit_error)?;
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(read_limit as u64)
            .read_to_end(&mut bytes)
            .map_err(footprint_io_error)?;
        if bytes.len() > limits.max_source_bytes {
            return Err(limit_error());
        }
        let source = crate::sexpr::utf8_text(&bytes)?.to_owned();
        Self::parse(source, limits)
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn into_source(self) -> String {
        self.source
    }

    pub fn limits(&self) -> FootprintLimits {
        self.limits
    }

    pub fn view(&self) -> Result<FootprintView<'_>, Error> {
        FootprintView::parse(&self.source, self.limits)
    }

    /// Write the current source after checking the configured output ceiling.
    pub fn write_to(&self, mut writer: impl Write) -> Result<(), Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(Error::build(
                ErrorKind::ResourceLimit,
                "Footprint output exceeds max_output_bytes",
            ));
        }
        writer
            .write_all(self.source.as_bytes())
            .map_err(footprint_io_error)
    }
}

struct FootprintChildren {
    properties: Vec<FormSpan>,
    texts: Vec<FormSpan>,
    text_boxes: Vec<FormSpan>,
    pads: Vec<FormSpan>,
    models: Vec<FormSpan>,
    graphics: Vec<FormSpan>,
    embedded_files: Vec<FormSpan>,
    pad_count: usize,
}

impl<'a> FootprintView<'a> {
    /// Validate and index the top-level footprint, properties, and pads.
    pub fn parse(source: &'a str, limits: FootprintLimits) -> Result<Self, Error> {
        let child_limit = limits
            .max_text_carriers
            .checked_add(limits.max_pads)
            .and_then(|value| value.checked_add(limits.max_models))
            .and_then(|value| value.checked_add(limits.max_graphics))
            .and_then(|value| value.checked_add(limits.max_embedded_files))
            .ok_or_else(limit_error)?;
        let projection_limits = |max_selected_forms| ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms,
            ..ProjectionLimits::default()
        };
        let root_selector = Selector {
            min_depth: Some(0),
            max_depth: Some(0),
            ..Selector::default()
        };
        let roots = scan_form_spans_with_limits(source, &root_selector, projection_limits(2))?;
        let [root] = roots.try_into().map_err(|_| {
            source_error(
                "Expected exactly one top-level footprint form",
                Position::START,
            )
        })?;
        if root.head.as_deref() != Some("footprint") {
            return Err(source_error("Expected a footprint root", root.start));
        }

        let child_selector = Selector {
            paths: Some(BTreeSet::from([
                vec!["footprint".to_owned(), "property".to_owned()],
                vec!["footprint".to_owned(), "pad".to_owned()],
                vec!["footprint".to_owned(), "fp_text".to_owned()],
                vec!["footprint".to_owned(), "fp_text_box".to_owned()],
                vec!["footprint".to_owned(), "model".to_owned()],
                vec!["footprint".to_owned(), "fp_line".to_owned()],
                vec!["footprint".to_owned(), "fp_rect".to_owned()],
                vec!["footprint".to_owned(), "fp_arc".to_owned()],
                vec!["footprint".to_owned(), "fp_circle".to_owned()],
                vec!["footprint".to_owned(), "fp_poly".to_owned()],
                vec!["footprint".to_owned(), "fp_curve".to_owned()],
                vec![
                    "footprint".to_owned(),
                    "embedded_files".to_owned(),
                    "file".to_owned(),
                ],
            ])),
            min_depth: Some(1),
            max_depth: Some(2),
            ..Selector::default()
        };
        let spans =
            scan_form_spans_with_limits(source, &child_selector, projection_limits(child_limit))?;
        let children = partition_children(spans, limits)?;
        Ok(Self {
            source,
            root,
            properties: children.properties,
            texts: children.texts,
            text_boxes: children.text_boxes,
            pads: children.pads,
            models: children.models,
            graphics: children.graphics,
            embedded_files: children.embedded_files,
            pad_count: children.pad_count,
            limits,
        })
    }

    /// Decode the footprint name without materializing its child forms.
    pub fn name(&self) -> Result<Cow<'a, str>, Error> {
        let text = self.root.text(self.source)?;
        (|| {
            let mut lexer = Lexer::new(text);
            expect_kind(
                lexer.next(),
                TokenKind::Left,
                "Expected footprint opening parenthesis",
            )?;
            expect_atom(lexer.next(), "footprint", "Expected footprint root")?;
            let token = next_value(lexer.next(), "Expected footprint name")?;
            Ok(decoded(token))
        })()
        .map_err(|error| rebase_error(error, &self.root))
    }

    pub fn pad_count(&self) -> usize {
        self.pad_count
    }

    /// Decode footprint metadata and local defaults using the same record as
    /// an embedded board footprint. Placement fields remain local/absent.
    pub fn metadata(&self) -> Result<PcbFootprint, Error> {
        standalone_footprint_from_span(
            self.source,
            &self.root,
            self.pcb_limits(),
            StandaloneFootprintCounts {
                properties: self.properties.len(),
                graphics: self.graphics.len(),
                texts: self.texts.len(),
                text_boxes: self.text_boxes.len(),
                pads: self.pads.len(),
                models: self.models.len(),
                embedded_files: self.embedded_files.len(),
            },
        )
    }

    /// Decode standalone pads in source order. `footprint_index` is zero for
    /// this single-root view; `source_range` is the stable member identity.
    pub fn pads(&self) -> impl Iterator<Item = Result<PcbPad, Error>> + '_ {
        let limits = self.pcb_limits();
        self.pads
            .iter()
            .map(move |span| standalone_pad_from_span(self.source, span, limits))
    }

    /// Decode standalone 3D model references in source order.
    pub fn models(&self) -> impl Iterator<Item = Result<PcbModelReference, Error>> + '_ {
        let limits = self.pcb_limits();
        self.models
            .iter()
            .map(move |span| standalone_model_from_span(self.source, span, limits))
    }

    /// Decode non-text footprint graphics in source order.
    pub fn graphics(&self) -> impl Iterator<Item = Result<PcbGraphic, Error>> + '_ {
        let limits = self.pcb_limits();
        self.graphics
            .iter()
            .map(move |span| standalone_graphic_from_span(self.source, span, limits))
    }

    /// Inspect standalone-footprint resource declarations without decoding payloads.
    pub fn embedded_files(&self) -> impl Iterator<Item = Result<EmbeddedFile, Error>> + '_ {
        self.embedded_files.iter().map(|span| {
            metadata_from_span(self.source, span, EmbeddedFileOwner::StandaloneFootprint)
        })
    }

    /// Return one resource's joined base64 text without decompressing it.
    pub fn embedded_file_encoded_data(
        &self,
        file: &EmbeddedFile,
        maximum: usize,
    ) -> Result<Option<String>, Error> {
        if file.owner != EmbeddedFileOwner::StandaloneFootprint
            || !self
                .embedded_files
                .iter()
                .any(|span| span.range == file.source_range)
        {
            return Err(Error::build(
                ErrorKind::InvalidSpan,
                format!(
                    "Embedded resource {:?} does not belong to this footprint view",
                    file.name
                ),
            ));
        }
        encoded_data(self.source, file, maximum)
    }

    /// Decode and checksum-verify one zstd/base64 payload on demand.
    #[cfg(feature = "embedded-resource-zstd")]
    pub fn decode_embedded_file(
        &self,
        file: &EmbeddedFile,
        limits: EmbeddedDecodeLimits,
    ) -> Result<Option<Vec<u8>>, Error> {
        if file.owner != EmbeddedFileOwner::StandaloneFootprint
            || !self
                .embedded_files
                .iter()
                .any(|span| span.range == file.source_range)
        {
            return Err(Error::build(
                ErrorKind::InvalidSpan,
                format!(
                    "Embedded resource {:?} does not belong to this footprint view",
                    file.name
                ),
            ));
        }
        decoded_data(self.source, file, limits)
    }

    /// Decode properties one at a time from their selected source ranges.
    pub fn properties(&self) -> impl Iterator<Item = Result<FootprintProperty<'a>, Error>> + '_ {
        self.properties
            .iter()
            .map(|span| property_from_span(self.source, span))
    }

    /// Replace an existing top-level property value and preserve every other byte.
    pub fn set_property(
        &self,
        name: &str,
        value: &str,
        max_output_bytes: usize,
    ) -> Result<FootprintEdit, Error> {
        if self.source.len() > max_output_bytes {
            return Err(Error::build(
                ErrorKind::ResourceLimit,
                "Footprint output exceeds max_output_bytes",
            ));
        }
        let mut matching = self.properties().filter_map(|property| match property {
            Ok(property) if property.name == name => Some(Ok(property)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        });
        let property = matching
            .next()
            .transpose()?
            .ok_or_else(|| source_error("Footprint property was not found", self.root.start))?;
        if matching.next().transpose()?.is_some() {
            return Err(source_error(
                "Footprint property name is ambiguous",
                self.root.start,
            ));
        }
        if property.value == value {
            return Ok(FootprintEdit {
                source: self.source.to_owned(),
                changed: false,
            });
        }
        let replacement = build_with_limit(&Sexp::Quoted(value.to_owned()), max_output_bytes)?;
        let source = apply_patches_with_limit(
            self.source,
            &[Patch::new(
                property.value_range.start,
                property.value_range.end,
                replacement,
            )],
            max_output_bytes,
        )?;
        Ok(FootprintEdit {
            source,
            changed: true,
        })
    }

    pub(crate) fn pcb_limits(&self) -> PcbLimits {
        let mut limits = PcbLimits::default();
        limits.max_source_bytes = self.limits.max_source_bytes;
        limits.max_output_bytes = self.limits.max_output_bytes;
        limits.max_depth = self.limits.max_depth;
        limits.max_properties = self.limits.max_properties;
        limits.max_pads = self.limits.max_pads;
        limits.max_models = self.limits.max_models;
        limits.max_graphics = self.limits.max_graphics;
        limits.max_footprint_graphics = self.limits.max_graphics;
        limits.max_embedded_files = self.limits.max_embedded_files;
        limits.max_object_children = self.limits.max_object_nodes;
        limits.max_footprint_children = self.limits.max_object_nodes;
        limits.max_footprint_header_scalars = self.limits.max_object_nodes;
        limits.max_layers = limits.max_layers.min(self.limits.max_object_nodes);
        limits.max_footprint_attributes = limits
            .max_footprint_attributes
            .min(self.limits.max_object_nodes);
        limits.max_text_effect_children = limits
            .max_text_effect_children
            .min(self.limits.max_object_nodes);
        limits.max_text_font_children = limits
            .max_text_font_children
            .min(self.limits.max_object_nodes);
        limits.max_text_justify_tokens = limits
            .max_text_justify_tokens
            .min(self.limits.max_object_nodes);
        limits.max_text_box_points = limits.max_text_box_points.min(self.limits.max_object_nodes);
        limits.max_pad_header_scalars = limits
            .max_pad_header_scalars
            .min(self.limits.max_object_nodes);
        limits.max_pad_children = limits.max_pad_children.min(self.limits.max_object_nodes);
        limits.max_pad_chamfer_corners = limits
            .max_pad_chamfer_corners
            .min(self.limits.max_object_nodes);
        limits.max_pad_custom_primitives = limits
            .max_pad_custom_primitives
            .min(self.limits.max_object_nodes);
        limits.max_pad_custom_point_forms = limits
            .max_pad_custom_point_forms
            .min(self.limits.max_object_nodes);
        limits.max_pad_custom_points = limits
            .max_pad_custom_points
            .min(self.limits.max_object_nodes);
        limits.max_model_children = limits.max_model_children.min(self.limits.max_object_nodes);
        limits.max_graphic_points = limits.max_graphic_points.min(self.limits.max_object_nodes);
        limits.max_manufacturing_children = limits
            .max_manufacturing_children
            .min(self.limits.max_object_nodes);
        limits.max_teardrop_scalars = limits
            .max_teardrop_scalars
            .min(self.limits.max_object_nodes);
        limits.max_zone_layer_connections = limits
            .max_zone_layer_connections
            .min(self.limits.max_object_nodes);
        limits
    }
}

fn partition_children(
    spans: Vec<FormSpan>,
    limits: FootprintLimits,
) -> Result<FootprintChildren, Error> {
    let mut children = FootprintChildren {
        properties: Vec::new(),
        texts: Vec::new(),
        text_boxes: Vec::new(),
        pads: Vec::new(),
        models: Vec::new(),
        graphics: Vec::new(),
        embedded_files: Vec::new(),
        pad_count: 0,
    };
    for span in spans {
        match (span.depth, span.head.as_deref()) {
            (1, Some("property")) => children.properties.push(span),
            (1, Some("pad")) => {
                children.pad_count = children.pad_count.saturating_add(1);
                children.pads.push(span);
            }
            (1, Some("fp_text")) => children.texts.push(span),
            (1, Some("fp_text_box")) => children.text_boxes.push(span),
            (1, Some("model")) => children.models.push(span),
            (1, Some("fp_line" | "fp_rect" | "fp_arc" | "fp_circle" | "fp_poly" | "fp_curve")) => {
                children.graphics.push(span);
            }
            (2, Some("file")) if span.path == ["footprint", "embedded_files", "file"] => {
                children.embedded_files.push(span);
            }
            _ => {}
        }
    }
    validate_child_limits(&children, limits)?;
    Ok(children)
}

fn validate_child_limits(
    children: &FootprintChildren,
    limits: FootprintLimits,
) -> Result<(), Error> {
    let carrier_count = children.properties.len().saturating_add(
        children
            .texts
            .len()
            .saturating_add(children.text_boxes.len()),
    );
    if children.properties.len() > limits.max_properties
        || children.pad_count > limits.max_pads
        || children.texts.len() > limits.max_texts
        || children.text_boxes.len() > limits.max_text_boxes
        || carrier_count > limits.max_text_carriers
        || children.embedded_files.len() > limits.max_embedded_files
        || children.models.len() > limits.max_models
        || children.graphics.len() > limits.max_graphics
    {
        return Err(limit_error());
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FootprintEdit {
    pub source: String,
    pub changed: bool,
}

fn property_from_span<'a>(
    source: &'a str,
    span: &FormSpan,
) -> Result<FootprintProperty<'a>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(
            lexer.next(),
            TokenKind::Left,
            "Expected property opening parenthesis",
        )?;
        expect_atom(lexer.next(), "property", "Expected property form")?;
        let name = next_value(lexer.next(), "Expected property name")?;
        let value = next_value(lexer.next(), "Expected property value")?;
        Ok(FootprintProperty {
            name: decoded(name),
            value: decoded(value.clone()),
            value_range: (span.range.start + value.position.offset)
                ..(span.range.start + value.position.offset + value.lexeme.len()),
        })
    })()
    .map_err(|error| rebase_error(error, span))
}

pub(crate) fn rebase_error(mut error: Error, span: &FormSpan) -> Error {
    if let Some(position) = error.position {
        error.position = Some(Position {
            offset: span.range.start.saturating_add(position.offset),
            line: span
                .start
                .line
                .saturating_add(position.line.saturating_sub(1)),
            column: if position.line == 1 {
                span.start
                    .column
                    .saturating_add(position.column.saturating_sub(1))
            } else {
                position.column
            },
        });
    }
    error
}

fn decoded(token: Token<'_>) -> Cow<'_, str> {
    if token.kind == TokenKind::QuotedString {
        Cow::Owned(decode_quoted(token.lexeme))
    } else {
        Cow::Borrowed(token.lexeme)
    }
}

fn next_value<'a>(
    token: Option<Result<Token<'a>, Error>>,
    message: &'static str,
) -> Result<Token<'a>, Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if matches!(token.kind, TokenKind::Left | TokenKind::Right) {
        return Err(source_error(message, token.position));
    }
    Ok(token)
}

fn expect_kind(
    token: Option<Result<Token<'_>, Error>>,
    kind: TokenKind,
    message: &'static str,
) -> Result<(), Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if token.kind != kind {
        return Err(source_error(message, token.position));
    }
    Ok(())
}

fn expect_atom(
    token: Option<Result<Token<'_>, Error>>,
    atom: &str,
    message: &'static str,
) -> Result<(), Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if token.kind != TokenKind::Atom || token.lexeme != atom {
        return Err(source_error(message, token.position));
    }
    Ok(())
}

fn source_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Footprint typed read exceeds configured limits",
        Position::START,
    )
}

fn footprint_io_error(error: std::io::Error) -> Error {
    Error::build(
        ErrorKind::Io,
        format!("Footprint source I/O failed: {error}"),
    )
}
