//! Net-tie and jumper pad groups, and the nets a net-tie bridge joins.
//!
//! KiCad gives footprint copper no net: it never enters connectivity. A
//! net-tie footprint's copper may still short the pads of one group, and
//! DRC allows it (`FOOTPRINT::BuildNetTieCache`): a copper shape touching a
//! grouped pad may join the nets of every pad in that pad's group. The
//! materializer keeps the shape's own net and lists those nets on its
//! binding entry. Touching is tested on bounds, a superset of KiCad's exact
//! collision test.

use super::emit::{Emitter, Origin};
use super::geometry::board_bounds;
use super::nets::net_key;
use super::pads::{pad_shapes, place_pad};
use super::stack::CopperStack;
use crate::pcb::{PcbFootprint, PcbPad, resolve_pad_copper_layer};
use kicad_monkey_contracts::generated::materialization_document::{NmBounds, PadGroup};
use std::collections::{BTreeSet, HashMap};

/// Last board format whose net ties are footprints tagged "net tie".
const LEGACY_NET_TIES: i64 = 20_220_815;

/// The value of a quoted S-expression string token, backslash escapes
/// resolved.
fn token_value(raw: &str) -> String {
    let inner = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(raw);
    let mut value = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        value.push(if c == '\\' {
            chars.next().unwrap_or(c)
        } else {
            c
        });
    }
    value
}

/// Pad numbers of one net-tie group string, split as
/// `FOOTPRINT::MapPadNumbersToNetTieGroups` does: on commas, a backslash
/// escaping the next character, each number trimmed, empty ones dropped.
fn split_group(group: &str) -> Vec<String> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    let mut chars = group.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => current.extend(chars.next()),
            ',' => numbers.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    numbers.push(current);
    distinct(numbers.iter().map(|number| number.trim().to_owned()))
}

fn distinct(numbers: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    numbers
        .filter(|number| !number.is_empty() && seen.insert(number.clone()))
        .collect()
}

/// Net-tie pad groups of `footprint`. Boards up to format 20220815 mark a
/// net tie by keywords starting with "net tie"; KiCad then ties every pad.
pub(crate) fn net_tie_groups(
    footprint: &PcbFootprint,
    pads: &[&PcbPad],
    version: i64,
) -> Vec<PadGroup> {
    let mut groups: Vec<Vec<String>> = footprint
        .net_tie_pad_groups
        .iter()
        .map(|group| match &group.raw_token {
            Some(raw) => split_group(&token_value(raw)),
            None => distinct(group.pad_names.iter().cloned()),
        })
        .collect();
    if version <= LEGACY_NET_TIES && footprint.tags.starts_with("net tie") {
        groups.push(distinct(pads.iter().map(|pad| pad.number.clone())));
    }
    groups
        .into_iter()
        .filter(|numbers| !numbers.is_empty())
        .map(|pad_numbers| PadGroup { pad_numbers })
        .collect()
}

/// Jumper pad groups of `footprint`: every pad whose number a group lists
/// is connected to the others. Duplicate pad numbers form a group of their
/// own when the footprint declares them jumpers.
pub(crate) fn jumper_groups(footprint: &PcbFootprint, pads: &[&PcbPad]) -> Vec<PadGroup> {
    let mut groups: Vec<Vec<String>> = footprint
        .jumper_pad_groups
        .iter()
        .map(|group| distinct(group.pad_names.iter().cloned()))
        .filter(|numbers| !numbers.is_empty())
        .collect();
    if footprint.duplicate_pad_numbers_are_jumpers == Some(true) {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for pad in pads.iter().filter(|pad| !pad.number.is_empty()) {
            *counts.entry(&pad.number).or_default() += 1;
        }
        let repeated = distinct(
            pads.iter()
                .filter(|pad| counts.get(pad.number.as_str()).is_some_and(|n| *n > 1))
                .map(|pad| pad.number.clone()),
        );
        groups.extend(repeated.into_iter().map(|number| vec![number]));
    }
    groups
        .into_iter()
        .map(|pad_numbers| PadGroup { pad_numbers })
        .collect()
}

/// One copper land of a grouped pad in a net-tie footprint.
struct TiePad {
    slice: String,
    bounds: NmBounds,
    /// Nets of every pad in the pad's group.
    group_nets: Vec<String>,
}

/// Grouped pad lands of every net-tie footprint, by footprint index.
#[derive(Default)]
pub(crate) struct NetTies {
    lands: HashMap<usize, Vec<TiePad>>,
}

fn overlaps(a: &NmBounds, b: &NmBounds) -> bool {
    *a.min_x_nm <= *b.max_x_nm
        && *b.min_x_nm <= *a.max_x_nm
        && *a.min_y_nm <= *b.max_y_nm
        && *b.min_y_nm <= *a.max_y_nm
}

impl NetTies {
    pub(crate) fn new(
        footprints: &[PcbFootprint],
        pads: &[PcbPad],
        copper: &CopperStack,
        version: i64,
    ) -> Self {
        let mut by_footprint: HashMap<usize, Vec<&PcbPad>> = HashMap::new();
        for pad in pads {
            by_footprint
                .entry(pad.footprint_index)
                .or_default()
                .push(pad);
        }
        let mut ties = Self::default();
        for (index, footprint) in footprints.iter().enumerate() {
            let owned = by_footprint.get(&index).map_or(&[][..], Vec::as_slice);
            let groups = net_tie_groups(footprint, owned, version);
            let lands = tie_lands(&groups, owned, footprints, copper);
            if !lands.is_empty() {
                ties.lands.insert(index, lands);
            }
        }
        ties
    }

    /// Nets a footprint copper shape on `slice` with `bounds` may join,
    /// sorted; empty when it touches no grouped pad.
    pub(crate) fn tied_nets(
        &self,
        footprint_index: usize,
        slice: &str,
        bounds: &[NmBounds],
    ) -> Vec<String> {
        let Some(lands) = self.lands.get(&footprint_index) else {
            return Vec::new();
        };
        let nets: BTreeSet<&String> = lands
            .iter()
            .filter(|land| land.slice == slice)
            .filter(|land| bounds.iter().any(|shape| overlaps(shape, &land.bounds)))
            .flat_map(|land| &land.group_nets)
            .collect();
        nets.into_iter().cloned().collect()
    }
}

/// Copper lands of the pads in `groups` that share their group with
/// another pad.
fn tie_lands(
    groups: &[PadGroup],
    pads: &[&PcbPad],
    footprints: &[PcbFootprint],
    copper: &CopperStack,
) -> Vec<TiePad> {
    let mut lands = Vec::new();
    for group in groups {
        let members: Vec<&&PcbPad> = pads
            .iter()
            .filter(|pad| group.pad_numbers.contains(&pad.number))
            .collect();
        if members.len() < 2 {
            continue;
        }
        let nets: BTreeSet<String> = members
            .iter()
            .map(|pad| net_key(&pad.net))
            .filter(|net| !net.is_empty())
            .collect();
        let group_nets: Vec<String> = nets.into_iter().collect();
        for pad in members {
            lands.extend(pad_lands(pad, footprints, copper, &group_nets));
        }
    }
    lands
}

fn pad_lands(
    pad: &PcbPad,
    footprints: &[PcbFootprint],
    copper: &CopperStack,
    group_nets: &[String],
) -> Vec<TiePad> {
    // Placement and shape problems are reported where the pad is emitted.
    let mut scratch = Emitter::default();
    let Some(placed) = place_pad(pad, footprints, &mut scratch) else {
        return Vec::new();
    };
    let origin: &Origin = &placed.origin;
    let mut lands = Vec::new();
    for layer in copper.expand(&pad.layers) {
        let (Some(slice), Ok(resolved)) =
            (copper.key(&layer), resolve_pad_copper_layer(pad, &layer))
        else {
            continue;
        };
        let affine = placed.frame.affine(resolved.offset.x, resolved.offset.y);
        let shapes = pad_shapes(&resolved, origin, slice, &mut scratch);
        for shape in shapes.base.into_iter().chain(shapes.primitives) {
            lands.push(TiePad {
                slice: slice.to_owned(),
                bounds: board_bounds(&shape, &affine),
                group_nets: group_nets.to_vec(),
            });
        }
    }
    lands
}
