//! Owner-scoped resource access on the source-backed PCB view.
use super::*;

impl PcbView<'_> {
    /// Iterate board-owned embedded resources in source order.
    ///
    /// This preserves the pre-existing board-scope behavior. Use
    /// [`Self::footprint_embedded_files`] or [`Self::all_embedded_files`] for
    /// nested declarations.
    pub fn embedded_files(&self) -> impl Iterator<Item = Result<PcbEmbeddedFile, Error>> + '_ {
        self.embedded_files
            .iter()
            .filter(move |indexed| {
                self.selection.contains(PcbFamily::EmbeddedFiles)
                    && indexed.owner == PcbEmbeddedFileOwner::Board
            })
            .map(|indexed| embedded_file_from_span(self.source, indexed, self.limits))
    }

    /// Iterate footprint-owned embedded resources in board and child source order.
    pub fn footprint_embedded_files(
        &self,
    ) -> impl Iterator<Item = Result<PcbEmbeddedFile, Error>> + '_ {
        self.embedded_files
            .iter()
            .filter(move |indexed| {
                self.selection.contains(PcbFamily::FootprintEmbeddedFiles)
                    && matches!(
                        indexed.owner,
                        PcbEmbeddedFileOwner::EmbeddedFootprint { .. }
                    )
            })
            .map(|indexed| embedded_file_from_span(self.source, indexed, self.limits))
    }

    /// Iterate all board and embedded-footprint resource declarations.
    pub fn all_embedded_files(&self) -> impl Iterator<Item = Result<PcbEmbeddedFile, Error>> + '_ {
        self.embedded_files
            .iter()
            .filter(move |indexed| match indexed.owner {
                PcbEmbeddedFileOwner::Board => self.selection.contains(PcbFamily::EmbeddedFiles),
                PcbEmbeddedFileOwner::EmbeddedFootprint { .. } => {
                    self.selection.contains(PcbFamily::FootprintEmbeddedFiles)
                }
                PcbEmbeddedFileOwner::StandaloneFootprint => false,
            })
            .map(|indexed| embedded_file_from_span(self.source, indexed, self.limits))
    }

    /// Return the exact joined base64 payload without decoding it.
    ///
    /// `None` means no `(data ...)` form was authored; `Some("")` is an
    /// explicitly empty payload.
    pub fn embedded_file_encoded_data(
        &self,
        file: &PcbEmbeddedFile,
        maximum: usize,
    ) -> Result<Option<String>, Error> {
        if !self
            .embedded_files
            .iter()
            .any(|indexed| indexed.span.range == file.source_range && indexed.owner == file.owner)
        {
            return Err(Error::build(
                ErrorKind::InvalidSpan,
                format!(
                    "Embedded resource {:?} does not belong to this PCB view",
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
        file: &PcbEmbeddedFile,
        limits: PcbEmbeddedDecodeLimits,
    ) -> Result<Option<Vec<u8>>, Error> {
        if !self
            .embedded_files
            .iter()
            .any(|indexed| indexed.span.range == file.source_range && indexed.owner == file.owner)
        {
            return Err(Error::build(
                ErrorKind::InvalidSpan,
                format!(
                    "Embedded resource {:?} does not belong to this PCB view",
                    file.name
                ),
            ));
        }
        decoded_data(self.source, file, limits)
    }
}
