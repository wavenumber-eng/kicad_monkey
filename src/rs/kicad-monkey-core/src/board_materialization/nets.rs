//! Deterministic net table and net-key resolution.

use crate::pcb::{PcbNet, PcbNetRef};
use kicad_monkey_contracts::generated::materialization_document::Net;
use std::collections::{BTreeMap, BTreeSet};

/// Resolved net key of a source reference: the net name, a `#ordinal` key
/// for legacy unnamed ordinals, or the empty key for no net.
pub(crate) fn net_key(net: &PcbNetRef) -> String {
    if let Some(name) = net.name.as_ref().filter(|name| !name.is_empty()) {
        return name.clone();
    }
    net.ordinal
        .filter(|ordinal| *ordinal != 0)
        .map(|ordinal| format!("#{ordinal}"))
        .unwrap_or_default()
}

/// Every declared and referenced net, sorted by key, always including the
/// empty no-net key.
pub(crate) fn net_table<'a>(
    declared: &[PcbNet],
    references: impl IntoIterator<Item = &'a PcbNetRef>,
) -> Vec<Net> {
    let mut keys: BTreeSet<String> = BTreeSet::from([String::new()]);
    keys.extend(
        declared
            .iter()
            .map(|net| net.name.clone())
            .filter(|name| !name.is_empty()),
    );
    keys.extend(references.into_iter().map(net_key));
    keys.into_iter()
        .map(|key| Net {
            name: key.clone(),
            key,
            net_class: None,
        })
        .collect()
}

/// Resolves graphic net references, which core leaves as ordinals, against
/// the declared nets and the net table.
pub(crate) struct NetNames {
    by_code: BTreeMap<i64, String>,
    known: BTreeSet<String>,
}

impl NetNames {
    pub(crate) fn new(declared: &[PcbNet], table: &[Net]) -> Self {
        Self {
            by_code: declared
                .iter()
                .map(|net| (net.code, net.name.clone()))
                .collect(),
            known: table.iter().map(|net| net.key.clone()).collect(),
        }
    }

    /// The net key of `net`, or `None` when it names no net in the table.
    pub(crate) fn resolve(&self, net: Option<&PcbNetRef>) -> Option<String> {
        let Some(net) = net else {
            return Some(String::new());
        };
        let key = match (&net.name, net.ordinal) {
            (Some(name), _) if !name.is_empty() => name.clone(),
            (_, Some(0) | None) => String::new(),
            (_, Some(code)) => self.by_code.get(&code).cloned()?,
        };
        self.known.contains(&key).then_some(key)
    }
}
