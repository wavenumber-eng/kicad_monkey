//! Focused source-preserving edits, separate from view indexing and records.
use super::*;

impl PcbView<'_> {
    /// Remove one unambiguous identified top-level object by `uuid` or legacy `id`.
    pub fn remove_top_level_by_id(&self, identifier: &str) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        if identifier.is_empty() {
            return Err(source_error(
                "PCB object identifier cannot be empty",
                self.root.start,
            ));
        }
        let mut matches = Vec::new();
        for span in &self.top_level {
            if top_level_identifier(self.source, span, self.limits)?.as_deref() == Some(identifier)
            {
                matches.push(span);
            }
        }
        match matches.as_slice() {
            [] => Ok(PcbEdit {
                source: self.source.to_owned(),
                changed: false,
            }),
            [span] => Ok(PcbEdit {
                source: apply_patches_with_limit(
                    self.source,
                    &[Patch::new(span.range.start, span.range.end, "")],
                    self.limits.max_output_bytes,
                )?,
                changed: true,
            }),
            _ => Err(source_error(
                "PCB object identifier is ambiguous",
                self.root.start,
            )),
        }
    }

    /// Replace one unambiguous top-level board property without rewriting the board.
    pub fn set_property(&self, name: &str, value: &str) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        let matches = self.matching_properties(name)?;
        let [property] = matches.as_slice() else {
            return Err(source_error(
                if matches.is_empty() {
                    "PCB property was not found"
                } else {
                    "PCB property name is ambiguous"
                },
                self.root.start,
            ));
        };
        if property.value == value {
            return Ok(PcbEdit {
                source: self.source.to_owned(),
                changed: false,
            });
        }
        let replacement = build_with_limit(
            &Sexp::Quoted(value.to_owned()),
            self.limits.max_output_bytes,
        )?;
        let source = apply_patches_with_limit(
            self.source,
            &[Patch::new(
                property.value_range.start,
                property.value_range.end,
                replacement,
            )],
            self.limits.max_output_bytes,
        )?;
        Ok(PcbEdit {
            source,
            changed: true,
        })
    }

    /// Update or append one unambiguous top-level board property.
    pub fn upsert_property(&self, name: &str, value: &str) -> Result<PcbEdit, Error> {
        let matches = self.matching_properties(name)?;
        match matches.as_slice() {
            [_] => self.set_property(name, value),
            [] => self.insert_property(name, value),
            _ => Err(source_error(
                "PCB property name is ambiguous",
                self.root.start,
            )),
        }
    }

    /// Remove one unambiguous top-level board property by name.
    pub fn remove_property(&self, name: &str) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        let matches = self.matching_properties(name)?;
        match matches.as_slice() {
            [] => Ok(PcbEdit {
                source: self.source.to_owned(),
                changed: false,
            }),
            [property] => Ok(PcbEdit {
                source: apply_patches_with_limit(
                    self.source,
                    &[Patch::new(
                        property.source_range.start,
                        property.source_range.end,
                        "",
                    )],
                    self.limits.max_output_bytes,
                )?,
                changed: true,
            }),
            _ => Err(source_error(
                "PCB property name is ambiguous",
                self.root.start,
            )),
        }
    }

    fn matching_properties(&self, name: &str) -> Result<Vec<PcbProperty>, Error> {
        self.top_level
            .iter()
            .filter(|span| span.head.as_deref() == Some("property"))
            .map(|span| property_from_span(self.source, span))
            .filter_map(|property| match property {
                Ok(property) if property.name == name => Some(Ok(property)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    fn insert_property(&self, name: &str, value: &str) -> Result<PcbEdit, Error> {
        let property_count = self
            .top_level
            .iter()
            .filter(|span| span.head.as_deref() == Some("property"))
            .count();
        if property_count >= self.limits.max_properties {
            return Err(limit_error());
        }
        let form = build_with_limit(
            &Sexp::List(vec![
                Sexp::Atom("property".to_owned()),
                Sexp::Quoted(name.to_owned()),
                Sexp::Quoted(value.to_owned()),
            ]),
            self.limits.max_output_bytes,
        )?;
        let offset = self.root.range.end.saturating_sub(1);
        let newline = if self.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let prefix =
            if self.source[..offset].ends_with('\n') || self.source[..offset].ends_with('\r') {
                ""
            } else {
                newline
            };
        let replacement = format!("{prefix}  {form}{newline}");
        Ok(PcbEdit {
            source: apply_patches_with_limit(
                self.source,
                &[Patch::new(offset, offset, replacement)],
                self.limits.max_output_bytes,
            )?,
            changed: true,
        })
    }
}
