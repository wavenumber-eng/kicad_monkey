use super::resource::limit_error;
use crate::SourceBundleError;

pub(super) struct GlobWorkBudget {
    used: usize,
    maximum: usize,
}

impl GlobWorkBudget {
    pub(super) const fn new(maximum: usize) -> Self {
        Self { used: 0, maximum }
    }

    fn reserve(&mut self, work: usize) -> Result<(), SourceBundleError> {
        self.used = self
            .used
            .checked_add(work)
            .ok_or_else(|| limit_error("KiCad netlist wildcard match work overflows"))?;
        if self.used > self.maximum {
            return Err(limit_error(
                "KiCad netlist wildcard match work exceeds its limit",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GlobPattern {
    tokens: Vec<Token>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Literal(char),
    AnyChar,
    AnySequence,
    Class(CharacterClass),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CharacterClass {
    negated: bool,
    literals: Vec<char>,
    ranges: Vec<(char, char)>,
}

impl GlobPattern {
    pub(super) fn compile(source: &str) -> Self {
        let characters = source.chars().collect::<Vec<_>>();
        let mut tokens = Vec::with_capacity(characters.len());
        let mut position = 0;
        while position < characters.len() {
            match characters[position] {
                '*' => {
                    if !matches!(tokens.last(), Some(Token::AnySequence)) {
                        tokens.push(Token::AnySequence);
                    }
                    position += 1;
                }
                '?' => {
                    tokens.push(Token::AnyChar);
                    position += 1;
                }
                '[' => {
                    if let Some((class, next)) = parse_class(&characters, position) {
                        tokens.push(Token::Class(class));
                        position = next;
                    } else {
                        tokens.push(Token::Literal('['));
                        position += 1;
                    }
                }
                value => {
                    tokens.push(Token::Literal(value));
                    position += 1;
                }
            }
        }
        Self { tokens }
    }

    /// Deterministic wildcard matching with bounded O(pattern) work memory.
    ///
    /// The active-state simulation avoids exponential star backtracking while
    /// retaining Python `fnmatchcase` semantics for `*`, `?`, and classes.
    pub(super) fn matches(
        &self,
        value: &str,
        work: &mut GlobWorkBudget,
    ) -> Result<bool, SourceBundleError> {
        let character_count = value.chars().count();
        work.reserve(self.required_work(character_count)?)?;
        let mut active = vec![false; self.tokens.len() + 1];
        active[0] = true;
        close_stars(&self.tokens, &mut active);
        for character in value.chars() {
            let mut next = vec![false; active.len()];
            for (position, token) in self.tokens.iter().enumerate() {
                if !active[position] {
                    continue;
                }
                match token {
                    Token::AnySequence => next[position] = true,
                    Token::AnyChar => next[position + 1] = true,
                    Token::Literal(expected) if *expected == character => {
                        next[position + 1] = true;
                    }
                    Token::Class(class) if class.matches(character) => {
                        next[position + 1] = true;
                    }
                    Token::Literal(_) | Token::Class(_) => {}
                }
            }
            close_stars(&self.tokens, &mut next);
            active = next;
        }
        close_stars(&self.tokens, &mut active);
        Ok(active[self.tokens.len()])
    }

    fn required_work(&self, character_count: usize) -> Result<usize, SourceBundleError> {
        let class_membership_work = self.tokens.iter().try_fold(0usize, |total, token| {
            let Token::Class(class) = token else {
                return Ok(total);
            };
            total
                .checked_add(class.literals.len())
                .and_then(|total| total.checked_add(class.ranges.len()))
                .ok_or_else(|| limit_error("KiCad netlist wildcard match work overflows"))
        })?;
        let per_character = self
            .tokens
            .len()
            .checked_add(class_membership_work)
            .and_then(|work| work.checked_add(1))
            .ok_or_else(|| limit_error("KiCad netlist wildcard match work overflows"))?;
        per_character
            .checked_mul(character_count)
            .and_then(|work| work.checked_add(self.tokens.len().checked_mul(2)?))
            .ok_or_else(|| limit_error("KiCad netlist wildcard match work overflows"))
    }
}

impl CharacterClass {
    fn matches(&self, value: char) -> bool {
        let contained = self.literals.contains(&value)
            || self
                .ranges
                .iter()
                .any(|(start, end)| *start <= value && value <= *end);
        contained != self.negated
    }
}

fn close_stars(tokens: &[Token], active: &mut [bool]) {
    for position in 0..tokens.len() {
        if active[position] && matches!(tokens[position], Token::AnySequence) {
            active[position + 1] = true;
        }
    }
}

fn parse_class(characters: &[char], start: usize) -> Option<(CharacterClass, usize)> {
    let mut content_start = start.checked_add(1)?;
    let negated = characters.get(content_start) == Some(&'!');
    if negated {
        content_start += 1;
    }
    let mut close = content_start;
    if characters.get(close) == Some(&']') {
        close += 1;
    }
    while characters.get(close).is_some_and(|value| *value != ']') {
        close += 1;
    }
    if close >= characters.len() || close == content_start {
        return None;
    }
    let content = &characters[content_start..close];
    let mut literals = Vec::new();
    let mut ranges = Vec::new();
    let mut position = 0;
    while position < content.len() {
        if position + 2 < content.len() && content[position + 1] == '-' {
            let start = content[position];
            let end = content[position + 2];
            if start <= end {
                ranges.push((start, end));
            }
            position += 3;
        } else {
            literals.push(content[position]);
            position += 1;
        }
    }
    Some((
        CharacterClass {
            negated,
            literals,
            ranges,
        },
        close + 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::{GlobPattern, GlobWorkBudget};
    use crate::SourceBundleErrorKind;

    #[test]
    fn matches_python_style_wildcards_without_backtracking() {
        for (pattern, value, expected) in [
            ("+9V*", "+9V_RAW", true),
            ("DAC?", "DAC1", true),
            ("DAC?", "DAC12", false),
            ("BUS[0-3]", "BUS2", true),
            ("BUS[!0-3]", "BUS8", true),
            ("BUS[!0-3]", "BUS2", false),
            ("name[", "name[", true),
            ("[*]", "*", true),
            ("[]a]", "]", true),
        ] {
            let mut work = GlobWorkBudget::new(10_000);
            assert_eq!(
                GlobPattern::compile(pattern)
                    .matches(value, &mut work)
                    .expect("work fits"),
                expected
            );
        }
    }

    #[test]
    fn aggregate_match_work_accepts_exact_limit_and_rejects_one_under() {
        let pattern = GlobPattern::compile("BUS[0-7]*");
        let required = pattern
            .required_work("BUS0123".chars().count())
            .expect("work size");
        let mut exact = GlobWorkBudget::new(required);
        assert!(pattern.matches("BUS0123", &mut exact).expect("exact limit"));

        let mut one_under = GlobWorkBudget::new(required - 1);
        assert_eq!(
            pattern
                .matches("BUS0123", &mut one_under)
                .expect_err("one under")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }

    #[test]
    fn starred_large_class_membership_is_fully_charged_before_matching() {
        let source = format!("*[{}]", "a".repeat(2_048));
        let value = "z".repeat(1_024);
        let pattern = GlobPattern::compile(&source);
        let required = pattern
            .required_work(value.chars().count())
            .expect("work size");
        assert!(required > 2_000_000);

        let mut exact = GlobWorkBudget::new(required);
        assert!(!pattern.matches(&value, &mut exact).expect("exact limit"));

        let mut one_under = GlobWorkBudget::new(required - 1);
        assert_eq!(
            pattern
                .matches(&value, &mut one_under)
                .expect_err("one under")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }
}
