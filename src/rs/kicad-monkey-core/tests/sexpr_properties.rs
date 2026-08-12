use kicad_monkey_core::{FormatOptions, Sexp, build, format, parse};

struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn bounded(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(bound).expect("small bound"))
            .expect("bounded value fits usize")
    }

    fn tree(&mut self, depth: usize) -> Sexp {
        if depth == 0 || self.bounded(4) != 0 {
            return match self.bounded(4) {
                0 => Sexp::Atom(format!("atom_{}", self.bounded(10_000))),
                1 => Sexp::Quoted(format!(
                    "quoted {} \\\" line\\n{}",
                    self.bounded(100),
                    self.bounded(100)
                )),
                2 => Sexp::Integer(i64::try_from(self.bounded(100_000)).expect("small integer")),
                _ => Sexp::Float((self.bounded(100_000) as f64) / 37.0),
            };
        }
        let count = 1 + self.bounded(6);
        let mut values = Vec::with_capacity(count);
        values.push(Sexp::Atom(format!("form_{}", self.bounded(32))));
        for _ in 1..count {
            values.push(self.tree(depth - 1));
        }
        Sexp::List(values)
    }
}

#[test]
fn generated_trees_preserve_semantics_and_stable_second_writes() {
    let mut generator = Generator(0x5eed_c0de_2026_0812);
    for case in 0..2_000 {
        let mut tree = generator.tree(5);
        if !matches!(tree, Sexp::List(_)) {
            tree = Sexp::List(vec![Sexp::Atom("root".to_owned()), tree]);
        }
        let first = build(&tree).unwrap_or_else(|error| panic!("case {case} build: {error}"));
        let reparsed = parse(&first).unwrap_or_else(|error| panic!("case {case} parse: {error}"));
        let second =
            build(&reparsed).unwrap_or_else(|error| panic!("case {case} rebuild: {error}"));
        assert_eq!(reparsed, tree, "case {case}: {first}");
        assert_eq!(second, first, "case {case}");

        let formatted = format(&first, FormatOptions::default())
            .unwrap_or_else(|error| panic!("case {case} format: {error}"));
        assert_eq!(
            parse(&formatted).unwrap_or_else(|error| panic!("case {case} formatted: {error}")),
            tree,
            "case {case}"
        );
    }
}

#[test]
fn generated_malformed_variants_never_parse_as_the_original_tree() {
    let mut generator = Generator(0xa11c_e5ed_5eed_1234);
    for case in 0..1_000 {
        let tree = Sexp::List(vec![
            Sexp::Atom("root".to_owned()),
            generator.tree(4),
            Sexp::Quoted(format!("value {}", generator.next())),
        ]);
        let valid = build(&tree).expect("generated tree should build");
        let malformed = match case % 3 {
            0 => valid[..valid.len() - 1].to_owned(),
            1 => format!("{valid})"),
            _ => format!("{}\"unterminated", &valid[..valid.len() - 1]),
        };
        assert!(parse(&malformed).is_err(), "case {case}: {malformed}");
    }
}
