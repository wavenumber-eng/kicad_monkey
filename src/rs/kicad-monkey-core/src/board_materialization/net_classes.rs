//! Net classes and differential pairs, resolved as KiCad 10.0.6 does.
//!
//! Classes come from the project's `net_settings`, or from `(net_class)`
//! forms of an old board, which KiCad copies over the project's classes,
//! default class, and patterns (`BOARD::SetProject`). A net's effective
//! class follows `NET_SETTINGS::GetEffectiveNetClass`:
//!
//! - every class assigned to the net by name, and every class whose pattern
//!   matches it, takes part; an unknown class name is an implicit class
//!   with no rules;
//! - with none, the net is in `Default`;
//! - otherwise the classes sort by priority, then name, each rule comes from
//!   the first class that sets it, and rules still unset come from
//!   `Default`, which then joins the list;
//! - a single class keeps its own name; several make a composite named by
//!   joining theirs with commas.

use super::emit::Emitter;
use super::geometry::nm;
use super::net_patterns::{Regex, compile_regex, regex_matches, wildcard_matches};
use super::{MaterializationError, error};
use crate::pcb::PcbView;
use crate::project::{ProjectDocument, ProjectLimits};
use crate::schematic_bus::{SchematicBusExpansionLimits, expand_schematic_bus_label};
use crate::sexpr::{Sexp, parse};
use kicad_monkey_contracts::generated::materialization_document::{
    DiffPair, Net, NetClass, NonNegativeNm,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

const DEFAULT: &str = "Default";

/// Rules in KiCad's field order: clearance, track width, via diameter and
/// drill, micro-via diameter and drill, pair width, gap, and via gap, then
/// the schematic wire and bus widths and the tuning profile, which take
/// part only in deciding whether `Default` fills a rule.
const RULES: usize = 12;
/// Rules `Default` fills whenever they are unset: every one but the pair
/// via gap (index 8) and the tuning profile (index 11), which it fills only
/// when it has one.
const FILLED_BY_DEFAULT: [usize; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 9, 10];
const TUNING_PROFILE: usize = 11;
/// KiCad's built-in `Default` class (mm; the wire and bus widths in mils).
const BUILT_IN: [f64; RULES] = [
    0.2, 0.2, 0.6, 0.3, 0.3, 0.1, 0.2, 0.25, 0.25, 6.0, 12.0, 0.0,
];

#[derive(Clone, Debug)]
struct Class {
    name: String,
    priority: i64,
    rules: [Option<f64>; RULES],
}

impl Class {
    fn built_in_default() -> Self {
        let mut rules = BUILT_IN.map(Some);
        rules[11] = None;
        Self {
            name: DEFAULT.to_owned(),
            priority: i64::from(i32::MAX),
            rules,
        }
    }

    fn implicit(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            priority: i64::from(i32::MAX) - 1,
            rules: [None; RULES],
        }
    }
}

/// One pattern: its bus members, each tried as regex or wildcard.
struct Pattern {
    members: Vec<(String, Regex)>,
    class: String,
}

impl Pattern {
    fn new(pattern: &str, class: &str) -> Self {
        let members = if pattern.contains('[') || pattern.contains('{') {
            expand_schematic_bus_label(
                pattern,
                &HashMap::new(),
                SchematicBusExpansionLimits::default(),
            )
            .ok()
            .filter(|members| !members.is_empty())
            .unwrap_or_else(|| vec![pattern.to_owned()])
        } else {
            vec![pattern.to_owned()]
        };
        Self {
            members: members
                .into_iter()
                .map(|member| {
                    let regex = compile_regex(&member);
                    (member, regex)
                })
                .collect(),
            class: class.to_owned(),
        }
    }

    /// Whether the pattern matches `name`; patterns beyond the regex subset
    /// are noted in `unsupported`.
    fn matches(&self, name: &str, unsupported: &mut BTreeSet<String>) -> bool {
        self.members.iter().any(|(member, regex)| {
            let by_regex = match regex {
                Regex::Compiled(node) => regex_matches(node, name).unwrap_or_else(|| {
                    unsupported.insert(format!(
                        "net-class pattern {member:?} is too costly to match"
                    ));
                    false
                }),
                Regex::Invalid => false,
                Regex::Unsupported(what) => {
                    unsupported.insert(what.clone());
                    false
                }
            };
            by_regex || wildcard_matches(member, name)
        })
    }
}

/// Net-class settings of one project and board.
pub(crate) struct NetClassSettings {
    default: Class,
    classes: BTreeMap<String, Class>,
    assignments: HashMap<String, Vec<String>>,
    patterns: Vec<Pattern>,
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64)
}

fn project_class(value: &serde_json::Map<String, Value>) -> Class {
    let mut rules = [None; RULES];
    for (index, key) in [
        "clearance",
        "track_width",
        "via_diameter",
        "via_drill",
        "microvia_diameter",
        "microvia_drill",
        "diff_pair_width",
        "diff_pair_gap",
        "diff_pair_via_gap",
        "wire_width",
        "bus_width",
    ]
    .into_iter()
    .enumerate()
    {
        rules[index] = number(value.get(key));
    }
    // A tuning profile is a rule only for deciding whether Default fills.
    rules[11] = value
        .get("tuning_profile")
        .and_then(Value::as_str)
        .filter(|profile| !profile.is_empty())
        .map(|_| 1.0);
    Class {
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        priority: value.get("priority").and_then(Value::as_i64).unwrap_or(-1),
        rules,
    }
}

impl NetClassSettings {
    /// Settings of the project file, migrated from older `net_settings`
    /// versions as KiCad does on load.
    pub(crate) fn from_project(source: &str) -> Result<Self, MaterializationError> {
        let document = ProjectDocument::parse(source.to_owned(), ProjectLimits::default())
            .map_err(|cause| error("project_parse", cause.to_string()))?;
        let view = document.view();
        // Typed decoding enforces the project limits.
        view.net_settings()
            .map_err(|cause| error("project_parse", cause.to_string()))?;
        let settings = view.get_path("net_settings").and_then(Value::as_object);
        let version = settings
            .and_then(|value| value.get("meta"))
            .and_then(|meta| meta.get("version"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let mut result = Self {
            default: Class::built_in_default(),
            classes: BTreeMap::new(),
            assignments: HashMap::new(),
            patterns: Vec::new(),
        };
        let classes: Vec<&serde_json::Map<String, Value>> = settings
            .and_then(|value| value.get("classes"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .filter(|class| class.get("name").is_some_and(Value::is_string))
            .collect();
        result.read_classes(&classes, version);
        result.read_assignments(settings, version);
        result.read_patterns(settings, &classes, version);
        Ok(result)
    }

    fn read_classes(&mut self, classes: &[&serde_json::Map<String, Value>], version: i64) {
        let mut order = 0;
        for value in classes {
            let mut class = project_class(value);
            // Version 4 gave classes priorities in array order.
            if version < 4 {
                class.priority = if class.name == DEFAULT {
                    i64::from(i32::MAX)
                } else {
                    order += 1;
                    order - 1
                };
            }
            if class.name == DEFAULT {
                self.default = class;
            } else {
                self.classes.insert(class.name.clone(), class);
            }
        }
    }

    fn read_assignments(
        &mut self,
        settings: Option<&serde_json::Map<String, Value>>,
        version: i64,
    ) {
        let assigned = settings
            .and_then(|value| value.get("netclass_assignments"))
            .and_then(Value::as_object);
        for (net, classes) in assigned.into_iter().flatten() {
            let names: Vec<String> = match classes {
                // Before version 4 a net named one class.
                Value::String(name) if version < 4 && !name.is_empty() => vec![name.clone()],
                Value::Array(names) => names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                _ => Vec::new(),
            };
            self.assignments.insert(net.clone(), names);
        }
    }

    fn read_patterns(
        &mut self,
        settings: Option<&serde_json::Map<String, Value>>,
        classes: &[&serde_json::Map<String, Value>],
        version: i64,
    ) {
        let mut seen = BTreeSet::new();
        let mut add = |pattern: &str, class: &str, patterns: &mut Vec<Pattern>| {
            if seen.insert((pattern.to_owned(), class.to_owned())) {
                patterns.push(Pattern::new(pattern, class));
            }
        };
        // Before version 3, classes listed their nets, which became patterns.
        if version < 3 {
            for class in classes {
                let name = class
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                for net in class
                    .get("nets")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(net) = net.as_str() {
                        add(net, name, &mut self.patterns);
                    }
                }
            }
        }
        let listed = settings
            .and_then(|value| value.get("netclass_patterns"))
            .and_then(Value::as_array);
        for entry in listed.into_iter().flatten() {
            if let (Some(pattern), Some(class)) = (
                entry.get("pattern").and_then(Value::as_str),
                entry.get("netclass").and_then(Value::as_str),
            ) {
                add(pattern, class, &mut self.patterns);
            }
        }
    }
}

/// `(net_class NAME DESCRIPTION (clearance mm) ... (add_net NET) ...)` of
/// an old board. Unlisted rules keep KiCad's built-in values.
/// The text of a scalar.
fn scalar(value: &Sexp) -> Option<String> {
    match value {
        Sexp::Atom(text) | Sexp::Quoted(text) => Some(text.clone()),
        Sexp::Integer(value) => Some(value.to_string()),
        Sexp::Float(value) => Some(value.to_string()),
        Sexp::List(_) => None,
    }
}

/// Rule index of a legacy `(net_class)` child.
fn legacy_rule(head: &str) -> Option<usize> {
    [
        "clearance",
        "trace_width",
        "via_dia",
        "via_drill",
        "uvia_dia",
        "uvia_drill",
        "diff_pair_width",
        "diff_pair_gap",
    ]
    .iter()
    .position(|name| *name == head)
}

fn legacy_class(form: &[Sexp]) -> Option<(Class, Vec<String>)> {
    let name = scalar(form.get(1)?)?;
    let mut rules = BUILT_IN.map(Some);
    rules[11] = None;
    let mut nets = Vec::new();
    for item in form.iter().skip(2) {
        let Sexp::List(child) = item else { continue };
        let (Some(head), Some(value)) = (
            child.first().and_then(scalar),
            child.get(1).and_then(scalar),
        ) else {
            continue;
        };
        if head == "add_net" {
            nets.push(value);
        } else if let Some(index) = legacy_rule(&head) {
            rules[index] = value.parse().ok();
        }
    }
    Some((
        Class {
            name,
            priority: -1,
            rules,
        },
        nets,
    ))
}

impl NetClassSettings {
    /// Replace classes, default class, and patterns with an old board's
    /// `(net_class)` forms, when it has any.
    pub(crate) fn with_board_classes(mut self, view: &PcbView<'_>) -> Self {
        let Some(legacy) = board_classes(view) else {
            return self;
        };
        self.default = Class::built_in_default();
        self.classes.clear();
        self.patterns.clear();
        for (class, nets) in legacy {
            for net in &nets {
                self.patterns.push(Pattern::new(net, &class.name));
            }
            if class.name == DEFAULT {
                self.default = class;
            } else {
                self.classes.insert(class.name.clone(), class);
            }
        }
        self
    }

    /// Settings from an old board's `(net_class)` forms alone.
    pub(crate) fn from_board(view: &PcbView<'_>) -> Option<Self> {
        board_classes(view)?;
        let empty = Self {
            default: Class::built_in_default(),
            classes: BTreeMap::new(),
            assignments: HashMap::new(),
            patterns: Vec::new(),
        };
        Some(empty.with_board_classes(view))
    }
}

fn board_classes(view: &PcbView<'_>) -> Option<Vec<(Class, Vec<String>)>> {
    let classes: Vec<(Class, Vec<String>)> = view
        .top_level_forms()
        .filter(|span| span.head.as_deref() == Some("net_class"))
        .filter_map(|span| {
            let text = span.text(view.source()).ok()?;
            match parse(text).ok()? {
                Sexp::List(form) => legacy_class(&form),
                _ => None,
            }
        })
        .collect();
    (!classes.is_empty()).then_some(classes)
}

/// A net's effective class before conversion to the contract.
struct Effective {
    name: String,
    constituents: Vec<String>,
    rules: [Option<f64>; RULES],
}

impl NetClassSettings {
    fn class(&self, name: &str) -> Class {
        if name == DEFAULT {
            return self.default.clone();
        }
        self.classes
            .get(name)
            .cloned()
            .unwrap_or_else(|| Class::implicit(name))
    }

    /// Classes assigned to `net` by name or matching it by pattern.
    fn matched(&self, net: &str, unsupported: &mut BTreeSet<String>) -> BTreeSet<&str> {
        let mut names: BTreeSet<&str> = BTreeSet::new();
        if net.is_empty() {
            return names;
        }
        if let Some(assigned) = self.assignments.get(net) {
            names.extend(assigned.iter().map(String::as_str));
        }
        for pattern in &self.patterns {
            if pattern.matches(net, unsupported) {
                names.insert(&pattern.class);
            }
        }
        names
    }

    fn effective(&self, net: &str, unsupported: &mut BTreeSet<String>) -> Effective {
        let names = self.matched(net, unsupported);
        if names.is_empty() {
            return Effective {
                name: DEFAULT.to_owned(),
                constituents: vec![DEFAULT.to_owned()],
                rules: self.default.rules,
            };
        }
        let mut classes: Vec<Class> = names.into_iter().map(|name| self.class(name)).collect();
        classes.sort_by(|a, b| (a.priority, &a.name).cmp(&(b.priority, &b.name)));
        let mut rules = [None; RULES];
        for (index, rule) in rules.iter_mut().enumerate() {
            *rule = classes.iter().find_map(|class| class.rules[index]);
        }
        let mut constituents: Vec<String> = classes.into_iter().map(|class| class.name).collect();
        let profile =
            rules[TUNING_PROFILE].is_none() && self.default.rules[TUNING_PROFILE].is_some();
        if profile
            || FILLED_BY_DEFAULT
                .iter()
                .any(|index| rules[*index].is_none())
        {
            for index in FILLED_BY_DEFAULT.into_iter().chain([TUNING_PROFILE]) {
                rules[index] = rules[index].or(self.default.rules[index]);
            }
            constituents.push(DEFAULT.to_owned());
        }
        Effective {
            name: constituents.join(","),
            constituents,
            rules,
        }
    }
}

fn length(value: Option<f64>) -> Option<NonNegativeNm> {
    value
        .filter(|value| *value >= 0.0)
        .map(|value| NonNegativeNm(i64::from(nm(value))))
}

fn contract_class(effective: &Effective) -> NetClass {
    let rule = |index: usize| length(effective.rules[index]);
    NetClass {
        name: effective.name.clone(),
        constituents: effective.constituents.clone(),
        clearance_nm: rule(0),
        track_width_nm: rule(1),
        via_diameter_nm: rule(2),
        via_drill_nm: rule(3),
        microvia_diameter_nm: rule(4),
        microvia_drill_nm: rule(5),
        diff_pair_width_nm: rule(6),
        diff_pair_gap_nm: rule(7),
        diff_pair_via_gap_nm: rule(8),
    }
}

/// Give every net its effective class and return the distinct classes,
/// sorted by name. Patterns beyond the supported regex subset are
/// reported once each.
pub(crate) fn assign_classes(
    settings: &NetClassSettings,
    nets: &mut [Net],
    emitter: &mut Emitter,
) -> Vec<NetClass> {
    let mut unsupported = BTreeSet::new();
    let mut classes = BTreeMap::new();
    for net in nets.iter_mut() {
        let effective = settings.effective(&net.name, &mut unsupported);
        net.net_class = Some(effective.name.clone());
        classes
            .entry(effective.name.clone())
            .or_insert_with(|| contract_class(&effective));
    }
    for message in unsupported {
        emitter.warn(
            "net_class_pattern_unsupported",
            format!("{message}; only its wildcard reading is used"),
            None,
        );
    }
    classes.into_values().collect()
}

/// KiCad's `MatchDpSuffix`: after any trailing digits and underscores, `+`
/// or `P` is positive and `-` or `N` negative. Returns the polarity, the
/// complementary net name, and the base name.
fn dp_suffix(name: &str) -> Option<(bool, String, String)> {
    let chars: Vec<char> = name.chars().collect();
    let mut count = 0;
    for c in chars.iter().rev() {
        count += 1;
        let (positive, complement) = match c {
            '0'..='9' | '_' => continue,
            '+' => (true, '-'),
            '-' => (false, '+'),
            'P' => (true, 'N'),
            'N' => (false, 'P'),
            _ => return None,
        };
        let base: String = chars[..chars.len() - count].iter().collect();
        let tail: String = chars[chars.len() - count + 1..].iter().collect();
        return Some((positive, format!("{base}{complement}{tail}"), base));
    }
    None
}

/// Differential pairs among `nets`: each positive net whose complement
/// exists, sorted by positive net.
pub(crate) fn diff_pairs(nets: &[Net]) -> Vec<DiffPair> {
    let keys: HashMap<&str, &str> = nets
        .iter()
        .filter(|net| !net.name.is_empty())
        .map(|net| (net.name.as_str(), net.key.as_str()))
        .collect();
    let mut pairs: Vec<DiffPair> = nets
        .iter()
        .filter_map(|net| {
            let (true, complement, base) = dp_suffix(&net.name)? else {
                return None;
            };
            let negative = keys.get(complement.as_str())?;
            Some(DiffPair {
                base_name: base,
                positive_net_key: net.key.clone(),
                negative_net_key: (*negative).to_owned(),
            })
        })
        .collect();
    pairs.sort_by(|a, b| a.positive_net_key.cmp(&b.positive_net_key));
    pairs
}

#[cfg(test)]
mod tests {
    use super::dp_suffix;

    #[test]
    fn pairs_follow_kicad_suffix_rules() {
        let pair = dp_suffix;
        assert_eq!(
            pair("USB_D+"),
            Some((true, "USB_D-".into(), "USB_D".into()))
        );
        assert_eq!(
            pair("CLK_P_1"),
            Some((true, "CLK_N_1".into(), "CLK_".into()))
        );
        assert_eq!(pair("DN"), Some((false, "DP".into(), "D".into())));
        assert_eq!(
            pair("LVDS_N12"),
            Some((false, "LVDS_P12".into(), "LVDS_".into()))
        );
        assert_eq!(pair("GND"), None);
        assert_eq!(pair("+5V"), None);
        assert_eq!(pair("clk_p"), None);
        assert_eq!(pair("1_2"), None);
    }
}
