//! Streaming Newstroke text-box wrapping.

use super::stroke_font_widths::NEWSTROKE_WIDTH_UNITS;

#[derive(Default)]
struct MarkupWidthState {
    stack: Vec<bool>,
    styled_depth: usize,
    /// Advance in 1/105 font units: normal deltas use x5, styled x4.
    width_units: u64,
}

impl MarkupWidthState {
    fn reset(&mut self) {
        self.stack.clear();
        self.styled_depth = 0;
        self.width_units = 0;
    }

    fn feed(&mut self, text: &str) {
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if matches!(ch, '_' | '^' | '~') && chars.peek() == Some(&'{') {
                let styled = matches!(ch, '_' | '^');
                self.stack.push(styled);
                self.styled_depth += usize::from(styled);
                chars.next();
                continue;
            }
            if ch == '}'
                && let Some(styled) = self.stack.pop()
            {
                self.styled_depth -= usize::from(styled);
                continue;
            }
            let Some(index) = (ch as usize).checked_sub(0x20) else {
                continue;
            };
            let Some(units) = NEWSTROKE_WIDTH_UNITS.get(index) else {
                continue;
            };
            let multiplier = if self.styled_depth > 0 { 4 } else { 5 };
            self.width_units = self
                .width_units
                .saturating_add(u64::from(*units) * multiplier);
        }
    }
}

/// Python `_wrap_text_box_lines`, implemented incrementally. Each byte is
/// appended/measured at most twice (once more when it triggers a wrap), so a
/// long paragraph cannot degrade into repeated whole-prefix measurement.
pub(crate) fn wrap_text_box(text: &str, max_width_mm: f64, size_x_nm: i64) -> String {
    if max_width_mm <= 0.0 || !text.contains(' ') {
        return text.to_owned();
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        let mut state = MarkupWidthState::default();
        for word in paragraph.split(' ') {
            let previous_len = current.len();
            if !current.is_empty() {
                current.push(' ');
                state.feed(" ");
            }
            current.push_str(word);
            state.feed(word);
            let scaled_width = state.width_units as f64 * size_x_nm as f64;
            if previous_len > 0 && scaled_width > max_width_mm * 105_000_000.0 {
                current.truncate(previous_len);
                lines.push(std::mem::take(&mut current));
                state.reset();
                current.push_str(word);
                state.feed(word);
            }
        }
        lines.push(current);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_table_and_streaming_wrapper_match_python_edges() {
        let width = |character: char| NEWSTROKE_WIDTH_UNITS[character as usize - 0x20];
        assert_eq!(width('A'), 18);
        assert_eq!(width(' '), 16);
        assert_eq!(width('\u{03a9}'), 24);
        assert_eq!(width('\u{2bff}'), 24);

        assert_eq!(wrap_text_box("A A", 5.2, 2_100_000), "A A");
        assert_eq!(wrap_text_box("A A", 5.19, 2_100_000), "A\nA");
        assert_eq!(wrap_text_box("oversized", 0.01, 1_270_000), "oversized");
        assert_eq!(wrap_text_box("A  A ", 1.8, 1_270_000), "A\nA\n");
        assert_eq!(wrap_text_box("A _{A A}", 7.0, 2_100_000), "A _{A\nA}");
        assert_eq!(wrap_text_box("A A", 3.15, 1_270_000), "A A");
        assert_eq!(wrap_text_box("A A", 3.13, 1_270_000), "A\nA");
        assert_eq!(wrap_text_box("A 中 A", 4.2, 1_270_000), "A 中 A");
    }
}
