use crate::SvgError;
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
        self.work = self
            .work
            .checked_add(value.len())
            .ok_or_else(|| SvgError("SVG render work overflowed".to_owned()))?;
        if self.work > self.max_work {
            return Err(SvgError(
                "SVG render work exceeds the configured limit".to_owned(),
            ));
        }
        let length = self
            .output
            .len()
            .checked_add(value.len())
            .ok_or_else(|| SvgError("SVG byte count overflowed".to_owned()))?;
        if length > self.max_bytes {
            return Err(SvgError("SVG bytes exceed the configured limit".to_owned()));
        }
        self.output.push_str(value);
        Ok(())
    }

    pub(crate) fn escaped(&mut self, value: &str) -> Result<(), SvgError> {
        reject_xml_controls(value)?;
        for character in value.chars() {
            match character {
                '&' => self.raw("&amp;")?,
                '<' => self.raw("&lt;")?,
                '>' => self.raw("&gt;")?,
                '"' => self.raw("&quot;")?,
                '\'' => self.raw("&apos;")?,
                _ => {
                    let mut buffer = [0u8; 4];
                    self.raw(character.encode_utf8(&mut buffer))?;
                }
            }
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
            return Err(SvgError(format!("duplicate nonempty SVG id {value}")));
        }
        self.attribute("id", value)
    }

    pub(crate) fn element(&mut self) -> Result<(), SvgError> {
        self.elements = self
            .elements
            .checked_add(1)
            .ok_or_else(|| SvgError("SVG element count overflowed".to_owned()))?;
        if self.elements > self.max_elements {
            Err(SvgError(
                "SVG elements exceed the configured limit".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn finish(self) -> Result<(String, usize, usize), SvgError> {
        if self.output.is_empty() {
            Err(SvgError("SVG serializer produced no output".to_owned()))
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
        Err(SvgError(
            "text contains an XML 1.0 control character".to_owned(),
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
}
