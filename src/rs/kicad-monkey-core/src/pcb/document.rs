//! Owned PCB source with reparsing source-preserving mutations.

use super::*;
use crate::sexpr::utf8_text;
use std::io::{Read, Write};

/// An owned KiCad board document.
///
/// The document owns only source text and limits. Each [`PcbDocument::view`]
/// borrows that text and builds the bounded structural index, avoiding a
/// self-referential object and keeping unknown forms byte-preserved.
#[derive(Clone, Debug)]
pub struct PcbDocument {
    source: String,
    limits: PcbLimits,
}

impl PcbDocument {
    /// Validate and own one UTF-8 `kicad_pcb` source buffer.
    pub fn parse(source: String, limits: PcbLimits) -> Result<Self, Error> {
        PcbView::parse(&source, limits)?;
        Ok(Self { source, limits })
    }

    /// Read at most the configured source ceiling plus one sentinel byte.
    pub fn from_reader(mut reader: impl Read, limits: PcbLimits) -> Result<Self, Error> {
        let read_limit = limits
            .max_source_bytes
            .checked_add(1)
            .ok_or_else(limit_error)?;
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(read_limit as u64)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() > limits.max_source_bytes {
            return Err(limit_error());
        }
        let source = utf8_text(&bytes)?.to_owned();
        Self::parse(source, limits)
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn into_source(self) -> String {
        self.source
    }

    pub fn limits(&self) -> PcbLimits {
        self.limits
    }

    /// Build a fresh source-backed view over the current owned text.
    pub fn view(&self) -> Result<PcbView<'_>, Error> {
        PcbView::parse(&self.source, self.limits)
    }

    /// Build a dependency-aware selected view over the current source.
    pub fn view_selected(&self, selection: PcbSelection) -> Result<PcbView<'_>, Error> {
        PcbView::parse_selected(&self.source, self.limits, selection)
    }

    /// Write the current source verbatim after checking the output ceiling.
    pub fn write_to(&self, mut writer: impl Write) -> Result<(), Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        writer.write_all(self.source.as_bytes()).map_err(io_error)
    }

    /// Replace one unambiguous board property and reparse before committing.
    pub fn set_property(&mut self, name: &str, value: &str) -> Result<bool, Error> {
        let edit = self.view()?.set_property(name, value)?;
        self.commit(edit)
    }

    /// Remove one unambiguous identified top-level form and reparse before committing.
    pub fn remove_top_level_by_id(&mut self, identifier: &str) -> Result<bool, Error> {
        let edit = self.view()?.remove_top_level_by_id(identifier)?;
        self.commit(edit)
    }

    /// Replace one identified top-level object's singular layer field.
    pub fn set_top_level_layer_by_id(
        &mut self,
        identifier: &str,
        layer: &str,
    ) -> Result<bool, Error> {
        let edit = self.view()?.set_top_level_layer_by_id(identifier, layer)?;
        self.commit(edit)
    }

    fn commit(&mut self, edit: PcbEdit) -> Result<bool, Error> {
        if !edit.changed {
            return Ok(false);
        }
        PcbView::parse(&edit.source, self.limits)?;
        self.source = edit.source;
        Ok(true)
    }
}

fn io_error(error: std::io::Error) -> Error {
    Error::build(ErrorKind::Io, format!("PCB source I/O failed: {error}"))
}
