//! KiCad net-class pattern matching (`EDA_COMBINED_MATCHER`, net-class
//! context, KiCad 10.0.6).
//!
//! A pattern matches a net name when it matches as an anchored regular
//! expression or as an anchored wildcard (`*`, `?`), both case-sensitive
//! and over the whole name. KiCad compiles the regex as a Tcl advanced
//! regular expression and drops it when compilation fails. This module
//! implements the common subset: literals, `.`, bracket expressions with
//! ranges and negation, the `\d \w \s` classes and their negations,
//! escaped literals, groups (also `(?:`), alternation, the `* + ?` and
//! `{m,n}` quantifiers (greedy or not, which cannot change a whole-name
//! match), and `^ $` anchors. Anything else (back-references, word
//! boundaries, lookahead, POSIX classes, embedded options) is reported as
//! unsupported rather than guessed.

/// Steps a single regex match may take before it is abandoned.
const MAX_STEPS: usize = 200_000;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    Char(char),
    Any,
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
    Group(Box<Node>),
    Alternation(Vec<Node>),
    Sequence(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: Option<usize>,
    },
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ClassItem {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match self {
            Self::Range(low, high) => (*low..=*high).contains(&c),
            Self::Digit(positive) => c.is_ascii_digit() == *positive,
            Self::Word(positive) => (c.is_alphanumeric() || c == '_') == *positive,
            Self::Space(positive) => c.is_whitespace() == *positive,
        }
    }
}

/// How a pattern's regex leg compiled.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Regex {
    Compiled(Node),
    /// KiCad cannot compile it either, so only the wildcard leg applies.
    Invalid,
    /// Valid in KiCad but beyond this subset.
    Unsupported(String),
}

#[derive(Debug)]
enum ParseError {
    Invalid,
    Unsupported(String),
}

struct Parser<'a> {
    chars: Vec<char>,
    at: usize,
    pattern: &'a str,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        self.at += usize::from(c.is_some());
        c
    }

    fn unsupported(&self, what: &str) -> ParseError {
        ParseError::Unsupported(format!("{what} in net-class pattern {:?}", self.pattern))
    }

    fn alternation(&mut self) -> Result<Node, ParseError> {
        let mut branches = vec![self.sequence()?];
        while self.peek() == Some('|') {
            self.at += 1;
            branches.push(self.sequence()?);
        }
        Ok(if branches.len() == 1 {
            branches.remove(0)
        } else {
            Node::Alternation(branches)
        })
    }

    fn sequence(&mut self) -> Result<Node, ParseError> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            items.push(self.quantified(atom)?);
        }
        Ok(Node::Sequence(items))
    }

    fn quantified(&mut self, atom: Node) -> Result<Node, ParseError> {
        let (min, max) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => return self.bounded(atom),
            _ => return Ok(atom),
        };
        self.at += 1;
        self.lazy_marker();
        self.repeat(atom, min, max)
    }

    fn lazy_marker(&mut self) {
        if self.peek() == Some('?') {
            self.at += 1;
        }
    }

    fn repeat(&self, atom: Node, min: usize, max: Option<usize>) -> Result<Node, ParseError> {
        if matches!(atom, Node::Start | Node::End | Node::Repeat { .. }) {
            return Err(ParseError::Invalid);
        }
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
        })
    }

    fn number(&mut self) -> Option<usize> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        self.chars[start..self.at]
            .iter()
            .collect::<String>()
            .parse()
            .ok()
    }

    fn bounded(&mut self, atom: Node) -> Result<Node, ParseError> {
        self.at += 1;
        let min = self.number().ok_or(ParseError::Invalid)?;
        let max = if self.peek() == Some(',') {
            self.at += 1;
            self.number()
        } else {
            Some(min)
        };
        if self.next() != Some('}') || max.is_some_and(|max| max < min) || min > 255 {
            return Err(ParseError::Invalid);
        }
        self.lazy_marker();
        self.repeat(atom, min, max)
    }

    fn atom(&mut self) -> Result<Node, ParseError> {
        let c = self.next().ok_or(ParseError::Invalid)?;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => self.group()?,
            '[' => self.class()?,
            '\\' => self.escape()?,
            '*' | '+' | '?' | '{' => return Err(ParseError::Invalid),
            other => Node::Char(other),
        })
    }

    fn group(&mut self) -> Result<Node, ParseError> {
        if self.peek() == Some('?') {
            self.at += 1;
            if self.next() != Some(':') {
                return Err(self.unsupported("a lookahead or embedded option"));
            }
        }
        let inner = self.alternation()?;
        if self.next() != Some(')') {
            return Err(ParseError::Invalid);
        }
        Ok(Node::Group(Box::new(inner)))
    }

    fn escape(&mut self) -> Result<Node, ParseError> {
        let c = self.next().ok_or(ParseError::Invalid)?;
        let class = |item| Node::Class {
            negated: false,
            items: vec![item],
        };
        Ok(match c {
            'd' => class(ClassItem::Digit(true)),
            'D' => class(ClassItem::Digit(false)),
            'w' => class(ClassItem::Word(true)),
            'W' => class(ClassItem::Word(false)),
            's' => class(ClassItem::Space(true)),
            'S' => class(ClassItem::Space(false)),
            c if c.is_ascii_alphanumeric() => {
                return Err(self.unsupported(&format!("escape \\{c}")));
            }
            other => Node::Char(other),
        })
    }

    fn class(&mut self) -> Result<Node, ParseError> {
        let negated = self.peek() == Some('^');
        self.at += usize::from(negated);
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.next().ok_or(ParseError::Invalid)?;
            match c {
                ']' if !first => break,
                '[' if matches!(self.peek(), Some(':' | '.' | '=')) => {
                    return Err(self.unsupported("a POSIX bracket class"));
                }
                '\\' => items.push(self.class_escape()?),
                low => items.push(self.class_range(low)?),
            }
            first = false;
        }
        Ok(Node::Class { negated, items })
    }
}

impl Parser<'_> {
    fn class_escape(&mut self) -> Result<ClassItem, ParseError> {
        Ok(match self.next().ok_or(ParseError::Invalid)? {
            'd' => ClassItem::Digit(true),
            'w' => ClassItem::Word(true),
            's' => ClassItem::Space(true),
            c if c.is_ascii_alphanumeric() => {
                return Err(self.unsupported(&format!("escape \\{c}")));
            }
            other => ClassItem::Range(other, other),
        })
    }

    /// `low` alone, or the range `low-high`.
    fn class_range(&mut self, low: char) -> Result<ClassItem, ParseError> {
        let ranged = self.peek() == Some('-')
            && self.chars.get(self.at + 1).is_some_and(|next| *next != ']');
        if !ranged {
            return Ok(ClassItem::Range(low, low));
        }
        self.at += 1;
        let high = self.next().ok_or(ParseError::Invalid)?;
        if high < low {
            return Err(ParseError::Invalid);
        }
        Ok(ClassItem::Range(low, high))
    }
}

/// Compile `pattern` as KiCad's anchored regex leg.
pub(crate) fn compile_regex(pattern: &str) -> Regex {
    let mut parser = Parser {
        chars: pattern.chars().collect(),
        at: 0,
        pattern,
    };
    match parser.alternation() {
        Ok(node) if parser.at == parser.chars.len() => Regex::Compiled(node),
        Ok(_) | Err(ParseError::Invalid) => Regex::Invalid,
        Err(ParseError::Unsupported(what)) => Regex::Unsupported(what),
    }
}

struct Matcher<'a> {
    text: &'a [char],
    steps: usize,
}

impl Matcher<'_> {
    /// Match `nodes` from `at`, then the continuation `rest`.
    fn run(
        &mut self,
        node: &Node,
        at: usize,
        rest: &mut dyn FnMut(&mut Self, usize) -> bool,
    ) -> bool {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return false;
        }
        if let Some(width) = self.single(node, at) {
            return width.is_some_and(|width| rest(self, at + width));
        }
        match node {
            Node::Group(inner) => self.run(inner, at, rest),
            Node::Alternation(branches) => branches.iter().any(|branch| self.run(branch, at, rest)),
            Node::Sequence(items) => self.sequence(items, at, rest),
            Node::Repeat { node, min, max } => self.repeat(node, *min, *max, at, rest),
            // Fixed-width nodes are matched by `single` above.
            Node::Char(_) | Node::Any | Node::Class { .. } | Node::Start | Node::End => false,
        }
    }

    /// For a node that matches a fixed width at `at` (a character, any
    /// character, a class, or an anchor): `Some(Some(width))` on a match,
    /// `Some(None)` on none; `None` for other nodes.
    fn single(&self, node: &Node, at: usize) -> Option<Option<usize>> {
        let next = self.text.get(at);
        let matched = match node {
            Node::Char(c) => (next == Some(c)).then_some(1),
            Node::Any => next.map(|_| 1),
            Node::Class { negated, items } => next
                .filter(|c| items.iter().any(|item| item.matches(**c)) != *negated)
                .map(|_| 1),
            Node::Start => (at == 0).then_some(0),
            Node::End => (at == self.text.len()).then_some(0),
            _ => return None,
        };
        Some(matched)
    }

    fn sequence(
        &mut self,
        items: &[Node],
        at: usize,
        rest: &mut dyn FnMut(&mut Self, usize) -> bool,
    ) -> bool {
        match items.split_first() {
            None => rest(self, at),
            Some((first, others)) => self.run(first, at, &mut |matcher, next| {
                matcher.sequence(others, next, rest)
            }),
        }
    }

    fn repeat(
        &mut self,
        node: &Node,
        min: usize,
        max: Option<usize>,
        at: usize,
        rest: &mut dyn FnMut(&mut Self, usize) -> bool,
    ) -> bool {
        if min == 0 && rest(self, at) {
            return true;
        }
        if max == Some(0) {
            return false;
        }
        self.run(node, at, &mut |matcher, next| {
            // An empty iteration past the minimum cannot progress.
            if next == at && min == 0 {
                return false;
            }
            matcher.repeat(
                node,
                min.saturating_sub(1),
                max.map(|max| max - 1),
                next,
                rest,
            )
        })
    }
}

/// Whether `regex` matches the whole of `name`, `None` when the match was
/// abandoned past its step budget.
pub(crate) fn regex_matches(node: &Node, name: &str) -> Option<bool> {
    let text: Vec<char> = name.chars().collect();
    let mut matcher = Matcher {
        text: &text,
        steps: 0,
    };
    let length = text.len();
    let matched = matcher.run(node, 0, &mut |_, end| end == length);
    (matcher.steps <= MAX_STEPS).then_some(matched)
}

/// KiCad's anchored wildcard: `*` any run, `?` one character, everything
/// else literal; whole name, case-sensitive.
pub(crate) fn wildcard_matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let (mut p, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, n));
                p += 1;
            }
            Some(c) if *c == '?' || *c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => match star {
                Some((star_p, star_n)) => {
                    p = star_p + 1;
                    n = star_n + 1;
                    star = Some((star_p, star_n + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::{Regex, compile_regex, regex_matches, wildcard_matches};

    fn regex(pattern: &str, name: &str) -> bool {
        match compile_regex(pattern) {
            Regex::Compiled(node) => regex_matches(&node, name).expect("within budget"),
            other => panic!("{pattern}: {other:?}"),
        }
    }

    #[test]
    fn regex_subset_matches_whole_names() {
        assert!(regex("USB.*", "USB_D+"));
        assert!(!regex("USB", "USB_D+"));
        assert!(regex("A+", "AAA"));
        assert!(regex("^/CPU/DDR_DQ[0-9]+$", "/CPU/DDR_DQ12"));
        assert!(!regex("DQ[^0-9]", "DQ1"));
        assert!(regex("(CLK|STB)_[PN]", "STB_N"));
        assert!(regex("(?:A|B){2,3}", "ABA"));
        assert!(!regex("(?:A|B){2,3}", "ABAB"));
        assert!(regex("\\d{2}\\.\\w", "12.x"));
        assert!(regex("x*?y", "xxy"));
        assert!(regex("(a*)*b", "aaab"));
    }

    #[test]
    fn invalid_and_unsupported_regexes_are_told_apart() {
        assert_eq!(compile_regex("+A"), Regex::Invalid);
        assert_eq!(compile_regex("(A"), Regex::Invalid);
        assert_eq!(compile_regex("A{3,1}"), Regex::Invalid);
        assert!(matches!(compile_regex("(?=A)A"), Regex::Unsupported(_)));
        assert!(matches!(compile_regex("A\\1"), Regex::Unsupported(_)));
        assert!(matches!(
            compile_regex("[[:alpha:]]"),
            Regex::Unsupported(_)
        ));
    }

    #[test]
    fn wildcards_match_whole_names() {
        assert!(wildcard_matches("USB*", "USB_D+"));
        assert!(wildcard_matches("D?", "D1"));
        assert!(!wildcard_matches("D?", "D12"));
        assert!(wildcard_matches("*", ""));
        assert!(wildcard_matches("Net-(U1-Pad1)", "Net-(U1-Pad1)"));
        assert!(!wildcard_matches("gnd", "GND"));
    }
}
