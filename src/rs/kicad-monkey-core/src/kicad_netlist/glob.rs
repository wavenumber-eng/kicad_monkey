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
    pub(super) fn matches(&self, value: &str) -> bool {
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
        active[self.tokens.len()]
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
    use super::GlobPattern;

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
            assert_eq!(GlobPattern::compile(pattern).matches(value), expected);
        }
    }
}
