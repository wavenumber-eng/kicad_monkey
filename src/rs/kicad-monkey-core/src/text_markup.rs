//! Bounded parser for KiCad's `^{}`, `_{}`, and `~{}` text markup.
//!
//! Semantics match the accepted Python reference `kicad_text._parse_markup`,
//! which mirrors KiCad's markup grammar for outline text: a marker character
//! immediately followed by `{` opens a styled group, `}` closes the innermost
//! open group, a `}` outside any group is literal text, a marker without a
//! following `{` is literal text, and an unterminated group extends to the end
//! of the input.

use crate::{TextContourError, TextContourErrorKind};
use std::ops::Range;

/// One KiCad markup group style marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextMarkupMarker {
    /// `_{...}` renders children at the subscript size and offset.
    Subscript,
    /// `^{...}` renders children at the superscript size and offset.
    Superscript,
    /// `~{...}` renders children normally and then draws an overbar.
    Overbar,
}

/// One parsed markup node holding byte spans into the source line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TextMarkupNode {
    /// Literal text between markup groups.
    Text(Range<usize>),
    /// One marker group and its ordered children.
    Group {
        marker: TextMarkupMarker,
        children: Vec<TextMarkupNode>,
    },
}

/// Parse one line of KiCad markup into a bounded node tree.
///
/// Every retained text or group node is charged against `node_budget` before
/// it is created, so adversarial marker-heavy input fails closed without
/// unbounded stack or node growth. The shared budget lets one multiline block
/// charge every line against a single aggregate ceiling.
pub(crate) fn parse_text_markup(
    text: &str,
    node_budget: &mut usize,
    max_nodes: usize,
) -> Result<Vec<TextMarkupNode>, TextContourError> {
    let bytes = text.as_bytes();
    // Each stack entry is one charged open group: its marker plus the sibling
    // list that was active before the group opened.
    let mut stack: Vec<(TextMarkupMarker, Vec<TextMarkupNode>)> = Vec::new();
    let mut current: Vec<TextMarkupNode> = Vec::new();
    let mut text_start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'}' && !stack.is_empty() {
            flush_text(&mut current, text_start..index, node_budget, max_nodes)?;
            let (marker, parent) = stack.pop().expect("stack emptiness was checked");
            let children = std::mem::replace(&mut current, parent);
            current.push(TextMarkupNode::Group { marker, children });
            index += 1;
            text_start = index;
            continue;
        }
        if let Some(marker) = marker_for(byte)
            && bytes.get(index + 1) == Some(&b'{')
        {
            flush_text(&mut current, text_start..index, node_budget, max_nodes)?;
            charge_node(node_budget, max_nodes)?;
            stack.push((marker, std::mem::take(&mut current)));
            index += 2;
            text_start = index;
            continue;
        }
        index += 1;
    }
    flush_text(
        &mut current,
        text_start..bytes.len(),
        node_budget,
        max_nodes,
    )?;
    // Unterminated groups keep their children through the end of the input.
    while let Some((marker, parent)) = stack.pop() {
        let children = std::mem::replace(&mut current, parent);
        current.push(TextMarkupNode::Group { marker, children });
    }
    Ok(current)
}

fn marker_for(byte: u8) -> Option<TextMarkupMarker> {
    match byte {
        b'_' => Some(TextMarkupMarker::Subscript),
        b'^' => Some(TextMarkupMarker::Superscript),
        b'~' => Some(TextMarkupMarker::Overbar),
        _ => None,
    }
}

fn flush_text(
    nodes: &mut Vec<TextMarkupNode>,
    span: Range<usize>,
    node_budget: &mut usize,
    max_nodes: usize,
) -> Result<(), TextContourError> {
    if span.is_empty() {
        return Ok(());
    }
    charge_node(node_budget, max_nodes)?;
    nodes.push(TextMarkupNode::Text(span));
    Ok(())
}

fn charge_node(node_budget: &mut usize, max_nodes: usize) -> Result<(), TextContourError> {
    let next = node_budget
        .checked_add(1)
        .filter(|next| *next <= max_nodes)
        .ok_or_else(|| TextContourError {
            kind: TextContourErrorKind::ResourceLimit,
            path: "$.markup".to_owned(),
            message: "markup node limit exceeded",
        })?;
    *node_budget = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str, max_nodes: usize) -> Result<Vec<TextMarkupNode>, TextContourError> {
        let mut budget = 0usize;
        parse_text_markup(text, &mut budget, max_nodes)
    }

    fn text(range: Range<usize>) -> TextMarkupNode {
        TextMarkupNode::Text(range)
    }

    fn group(marker: TextMarkupMarker, children: Vec<TextMarkupNode>) -> TextMarkupNode {
        TextMarkupNode::Group { marker, children }
    }

    #[test]
    fn plain_text_is_one_node() {
        assert_eq!(parse("VOUT", 16).unwrap(), vec![text(0..4)]);
        assert_eq!(parse("", 16).unwrap(), Vec::new());
    }

    #[test]
    fn markers_split_text_into_styled_groups() {
        assert_eq!(
            parse("V_{IN}2", 16).unwrap(),
            vec![
                text(0..1),
                group(TextMarkupMarker::Subscript, vec![text(3..5)]),
                text(6..7),
            ],
        );
        assert_eq!(
            parse("X^{2}", 16).unwrap(),
            vec![
                text(0..1),
                group(TextMarkupMarker::Superscript, vec![text(3..4)]),
            ],
        );
        assert_eq!(
            parse("~{RST}", 16).unwrap(),
            vec![group(TextMarkupMarker::Overbar, vec![text(2..5)])],
        );
    }

    #[test]
    fn groups_nest_and_close_innermost_first() {
        assert_eq!(
            parse("~{A_{B}C}", 16).unwrap(),
            vec![group(
                TextMarkupMarker::Overbar,
                vec![
                    text(2..3),
                    group(TextMarkupMarker::Subscript, vec![text(5..6)]),
                    text(7..8),
                ],
            )],
        );
    }

    #[test]
    fn literal_markers_and_braces_stay_text() {
        // A marker without `{` and a `}` outside any group are literal.
        assert_eq!(parse("a_b^c~d}e", 16).unwrap(), vec![text(0..9)]);
        assert_eq!(parse("a{b}c", 16).unwrap(), vec![text(0..5)]);
    }

    #[test]
    fn unterminated_group_extends_to_end_of_input() {
        assert_eq!(
            parse("A_{BC", 16).unwrap(),
            vec![
                text(0..1),
                group(TextMarkupMarker::Subscript, vec![text(3..5)]),
            ],
        );
        assert_eq!(
            parse("~{A_{B", 16).unwrap(),
            vec![group(
                TextMarkupMarker::Overbar,
                vec![
                    text(2..3),
                    group(TextMarkupMarker::Subscript, vec![text(5..6)]),
                ],
            )],
        );
    }

    #[test]
    fn empty_group_holds_no_children() {
        assert_eq!(
            parse("~{}", 16).unwrap(),
            vec![group(TextMarkupMarker::Overbar, Vec::new())],
        );
    }

    #[test]
    fn multibyte_text_keeps_valid_byte_spans() {
        let source = "µ_{Ω}";
        let parsed = parse(source, 16).unwrap();
        assert_eq!(
            parsed,
            vec![
                text(0..2),
                group(TextMarkupMarker::Subscript, vec![text(4..6)]),
            ],
        );
        assert_eq!(&source[0..2], "µ");
        assert_eq!(&source[4..6], "Ω");
    }

    #[test]
    fn node_ceiling_is_inclusive_and_fails_closed() {
        // "V_{IN}2" retains exactly four nodes: text, group, inner text, text.
        assert!(parse("V_{IN}2", 4).is_ok());
        let error = parse("V_{IN}2", 3).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
        assert_eq!(error.path, "$.markup");
    }

    #[test]
    fn open_groups_are_charged_before_the_stack_grows() {
        let adversarial = "^{".repeat(64);
        let error = parse(&adversarial, 8).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    }

    #[test]
    fn shared_budget_spans_multiple_calls() {
        let mut budget = 0usize;
        parse_text_markup("AB", &mut budget, 2).unwrap();
        assert_eq!(budget, 1);
        let error = parse_text_markup("C_{D}", &mut budget, 2).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
    }
}
