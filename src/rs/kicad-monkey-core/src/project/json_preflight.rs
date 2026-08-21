use super::{PROJECT_MAX_JSON_DEPTH, ProjectError, ProjectLimits, check_count, limit_error};

/// Count JSON values and container nesting without allocating the generic DOM.
///
/// Serde remains the authoritative JSON parser. This pass only recognizes the
/// token boundaries needed to reject dense or deeply nested input before
/// `serde_json::Value` can amplify it in memory. Malformed input that cannot be
/// classified conservatively is left for Serde's structured syntax error.
pub(super) fn preflight_json_structure(
    source: &str,
    limits: ProjectLimits,
) -> Result<(), ProjectError> {
    JsonPreflight::new(source.as_bytes(), limits).scan()
}

struct JsonPreflight<'a> {
    bytes: &'a [u8],
    index: usize,
    nodes: usize,
    depth: usize,
    limits: ProjectLimits,
}

impl<'a> JsonPreflight<'a> {
    const fn new(bytes: &'a [u8], limits: ProjectLimits) -> Self {
        Self {
            bytes,
            index: 0,
            nodes: 0,
            depth: 0,
            limits,
        }
    }

    fn scan(mut self) -> Result<(), ProjectError> {
        while self.index < self.bytes.len() {
            self.scan_token()?;
        }
        Ok(())
    }

    fn scan_token(&mut self) -> Result<(), ProjectError> {
        match self.bytes[self.index] {
            b'{' | b'[' => self.open_container(),
            b'}' | b']' => {
                self.depth = self.depth.saturating_sub(1);
                self.index += 1;
                Ok(())
            }
            b'"' => self.scan_string(),
            b'-' | b'0'..=b'9' => self.scan_number(),
            b't' | b'f' | b'n' => self.scan_literal(),
            _ => {
                self.index += 1;
                Ok(())
            }
        }
    }

    fn open_container(&mut self) -> Result<(), ProjectError> {
        self.bump_node()?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| limit_error("project JSON depth overflows"))?;
        check_count(
            self.depth,
            self.limits.max_json_depth.min(PROJECT_MAX_JSON_DEPTH),
            "project JSON depth",
        )?;
        self.index += 1;
        Ok(())
    }

    fn scan_string(&mut self) -> Result<(), ProjectError> {
        self.index += 1;
        while self.index < self.bytes.len() {
            match self.bytes[self.index] {
                b'\\' => self.index = self.index.saturating_add(2),
                b'"' => {
                    self.index += 1;
                    break;
                }
                _ => self.index += 1,
            }
        }
        let mut lookahead = self.index;
        while lookahead < self.bytes.len() && self.bytes[lookahead].is_ascii_whitespace() {
            lookahead += 1;
        }
        if self.bytes.get(lookahead) == Some(&b':') {
            Ok(())
        } else {
            self.bump_node()
        }
    }

    fn scan_number(&mut self) -> Result<(), ProjectError> {
        self.bump_node()?;
        self.index += 1;
        while self.index < self.bytes.len() && !json_scalar_delimiter(self.bytes[self.index]) {
            self.index += 1;
        }
        Ok(())
    }

    fn scan_literal(&mut self) -> Result<(), ProjectError> {
        self.bump_node()?;
        self.index += 1;
        while self.index < self.bytes.len() && self.bytes[self.index].is_ascii_alphabetic() {
            self.index += 1;
        }
        Ok(())
    }

    fn bump_node(&mut self) -> Result<(), ProjectError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| limit_error("project JSON node count overflows"))?;
        check_count(self.nodes, self.limits.max_json_nodes, "project JSON nodes")
    }
}

const fn json_scalar_delimiter(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}')
}
