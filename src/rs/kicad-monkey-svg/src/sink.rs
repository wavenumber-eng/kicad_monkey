use crate::{SvgError, SvgErrorKind};
use std::collections::HashSet;

pub(crate) struct SvgSink {
    output: String,
    max_bytes: usize,
    max_elements: usize,
    max_work: usize,
    elements: usize,
    work: usize,
    ids: HashSet<String>,
}

impl SvgSink {
    pub(crate) fn new(max_bytes: usize, max_elements: usize, max_work: usize) -> Self {
        Self {
            output: String::new(),
            max_bytes,
            max_elements,
            max_work,
            elements: 0,
            work: 0,
            ids: HashSet::new(),
        }
    }

    pub(crate) fn raw(&mut self, value: &str) -> Result<(), SvgError> {
        self.work = self.work.checked_add(value.len()).ok_or_else(|| {
            SvgError::new(
                SvgErrorKind::ArithmeticOverflow,
                "SVG render work overflowed",
            )
        })?;
        if self.work > self.max_work {
            return Err(SvgError::new(
                SvgErrorKind::ResourceLimit,
                "SVG render work exceeds the configured limit",
            ));
        }
        let length = self.output.len().checked_add(value.len()).ok_or_else(|| {
            SvgError::new(
                SvgErrorKind::ArithmeticOverflow,
                "SVG byte count overflowed",
            )
        })?;
        if length > self.max_bytes {
            return Err(SvgError::new(
                SvgErrorKind::ResourceLimit,
                "SVG bytes exceed the configured limit",
            ));
        }
        self.output.push_str(value);
        Ok(())
    }

    pub(crate) fn escaped(&mut self, value: &str) -> Result<(), SvgError> {
        reject_xml_controls(value)?;
        let mut span_start = 0_usize;
        for (index, byte) in value.bytes().enumerate() {
            let replacement = match byte {
                b'&' => "&amp;",
                b'<' => "&lt;",
                b'>' => "&gt;",
                b'"' => "&quot;",
                b'\'' => "&apos;",
                _ => continue,
            };
            if span_start < index {
                self.raw(&value[span_start..index])?;
            }
            self.raw(replacement)?;
            span_start = index + 1;
        }
        if span_start < value.len() {
            self.raw(&value[span_start..])?;
        }
        Ok(())
    }

    pub(crate) fn attribute(&mut self, name: &str, value: &str) -> Result<(), SvgError> {
        self.raw(" ")?;
        self.raw(name)?;
        self.raw("=\"")?;
        self.escaped(value)?;
        self.raw("\"")
    }

    pub(crate) fn id_attribute(&mut self, value: &str) -> Result<(), SvgError> {
        if value.is_empty() {
            return Ok(());
        }
        if !self.ids.insert(value.to_owned()) {
            return Err(SvgError::new(
                SvgErrorKind::Serialization,
                format!("duplicate nonempty SVG id {value}"),
            ));
        }
        self.attribute("id", value)
    }

    pub(crate) fn element(&mut self) -> Result<(), SvgError> {
        self.elements = self.elements.checked_add(1).ok_or_else(|| {
            SvgError::new(
                SvgErrorKind::ArithmeticOverflow,
                "SVG element count overflowed",
            )
        })?;
        if self.elements > self.max_elements {
            Err(SvgError::new(
                SvgErrorKind::ResourceLimit,
                "SVG elements exceed the configured limit",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn finish(self) -> Result<(String, usize, usize), SvgError> {
        if self.output.is_empty() {
            Err(SvgError::new(
                SvgErrorKind::Serialization,
                "SVG serializer produced no output",
            ))
        } else {
            Ok((self.output, self.elements, self.work))
        }
    }
}

fn reject_xml_controls(value: &str) -> Result<(), SvgError> {
    if value.chars().any(|character| {
        let code = character as u32;
        (code < 0x20 && !matches!(character, '\t' | '\n' | '\r')) || matches!(code, 0xFFFE | 0xFFFF)
    }) {
        Err(SvgError::new(
            SvgErrorKind::Serialization,
            "text contains an XML 1.0 control character",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SvgSink;

    #[test]
    fn ids_are_nonempty_unique_and_xml_noncharacters_fail_closed() {
        let mut sink = SvgSink::new(1024, 10, 1024);
        sink.raw("<g").expect("group");
        sink.id_attribute("").expect("empty id omitted");
        sink.id_attribute("owned").expect("first id");
        assert!(sink.id_attribute("owned").is_err());
        assert!(sink.escaped("\u{fffe}").is_err());
        assert!(sink.escaped("\u{ffff}").is_err());
    }

    #[test]
    fn escaped_writes_utf8_spans_and_exact_xml_entities() {
        let mut sink = SvgSink::new(1024, 10, 1024);
        sink.escaped("café & <tag attr=\"x\">'ok'")
            .expect("escaped text");
        let (output, _, _) = sink.finish().expect("finished output");
        assert_eq!(
            output,
            "café &amp; &lt;tag attr=&quot;x&quot;&gt;&apos;ok&apos;"
        );
    }
}
