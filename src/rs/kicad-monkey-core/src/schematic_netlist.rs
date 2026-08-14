use crate::{
    SchematicBundleIndex, SchematicDriverPriority, SchematicOccurrenceConnectivityLimits,
    SchematicPinDriver, SchematicSubpartSettings, SchematicWireDriverKind, SchematicWireSubgraph,
    SourceBundleError, SourceBundleErrorKind, build_schematic_occurrence_subgraphs_with_settings,
};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLocalNetTerminal {
    pub symbol_index: usize,
    pub designator: String,
    pub pin: String,
    pub pin_name: String,
    pub pin_type: String,
    pub sheet_path: String,
    pub source_pin_id: String,
    pub svg_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLocalNet {
    pub name: String,
    pub code: u64,
    pub driver_priority: SchematicDriverPriority,
    pub driver_kind: Option<SchematicWireDriverKind>,
    pub auto_named: bool,
    pub terminals: Vec<SchematicLocalNetTerminal>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicLocalNetLimits {
    pub connectivity: SchematicOccurrenceConnectivityLimits,
    pub max_nets: usize,
    pub max_terminals: usize,
    pub max_name_bytes: usize,
    pub max_retained_string_bytes: usize,
}

impl Default for SchematicLocalNetLimits {
    fn default() -> Self {
        Self {
            connectivity: SchematicOccurrenceConnectivityLimits::default(),
            max_nets: 8_000_000,
            max_terminals: 16_000_000,
            max_name_bytes: 512 * 1024 * 1024,
            max_retained_string_bytes: 1024 * 1024 * 1024,
        }
    }
}

pub fn build_schematic_occurrence_nets(
    index: &SchematicBundleIndex,
    occurrence_index: usize,
    code_offset: u64,
    limits: SchematicLocalNetLimits,
) -> Result<Vec<SchematicLocalNet>, SourceBundleError> {
    build_schematic_occurrence_nets_with_settings(
        index,
        occurrence_index,
        code_offset,
        index.subpart_settings(),
        limits,
    )
}

pub fn build_schematic_occurrence_nets_with_settings(
    index: &SchematicBundleIndex,
    occurrence_index: usize,
    code_offset: u64,
    subparts: SchematicSubpartSettings,
    limits: SchematicLocalNetLimits,
) -> Result<Vec<SchematicLocalNet>, SourceBundleError> {
    let occurrence = index.occurrence(occurrence_index).ok_or_else(|| {
        SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            None,
            "schematic occurrence index is out of range",
        )
    })?;
    let mut builder = NetBuilder {
        source_path: &occurrence.source_path,
        sheet_path: &occurrence.legacy_address,
        limits,
        retained_string_bytes: 0,
        terminal_count: 0,
    };
    let subgraphs = build_schematic_occurrence_subgraphs_with_settings(
        index,
        occurrence_index,
        subparts,
        limits.connectivity,
    )?;
    builder.preflight_output_shape(&subgraphs, code_offset)?;
    let mut nets = Vec::new();
    for mut subgraph in subgraphs {
        if subgraph.pin_drivers.is_empty() && subgraph.label_drivers.is_empty() {
            continue;
        }
        if nets.len() >= limits.max_nets {
            return Err(builder.limit_error("local net count exceeds its limit"));
        }
        let code = code_offset
            .checked_add(
                u64::try_from(nets.len())
                    .map_err(|_| builder.limit_error("local net code exceeds the platform size"))?,
            )
            .ok_or_else(|| builder.limit_error("local net code overflows"))?;
        let (name, auto_named) = builder.name_net(&subgraph)?;
        builder.retain_string_bytes(name.len())?;
        subgraph.pin_drivers.sort_by(|left, right| {
            left.reference
                .cmp(&right.reference)
                .then_with(|| left.pin_number.cmp(&right.pin_number))
        });
        let terminals = builder.materialize_terminals(&subgraph.pin_drivers)?;
        nets.push(SchematicLocalNet {
            name,
            code,
            driver_priority: subgraph.chosen_priority,
            driver_kind: subgraph.chosen_kind,
            auto_named,
            terminals,
        });
    }
    builder.uniquify_sheet_pin_names(&mut nets)?;
    Ok(nets)
}

pub(crate) fn name_schematic_subgraph(
    source_path: &str,
    sheet_path: &str,
    subgraph: &SchematicWireSubgraph,
    max_name_bytes: usize,
) -> Result<(String, bool), SourceBundleError> {
    NetBuilder {
        source_path,
        sheet_path,
        limits: SchematicLocalNetLimits {
            max_name_bytes,
            max_retained_string_bytes: usize::MAX,
            ..SchematicLocalNetLimits::default()
        },
        retained_string_bytes: 0,
        terminal_count: 0,
    }
    .name_net(subgraph)
}

struct NetBuilder<'a> {
    source_path: &'a str,
    sheet_path: &'a str,
    limits: SchematicLocalNetLimits,
    retained_string_bytes: usize,
    terminal_count: usize,
}

impl NetBuilder<'_> {
    fn preflight_output_shape(
        &self,
        subgraphs: &[SchematicWireSubgraph],
        code_offset: u64,
    ) -> Result<(), SourceBundleError> {
        let mut net_count = 0_usize;
        let mut terminal_count = 0_usize;
        for subgraph in subgraphs {
            if subgraph.pin_drivers.is_empty() && subgraph.label_drivers.is_empty() {
                continue;
            }
            net_count = net_count
                .checked_add(1)
                .ok_or_else(|| self.limit_error("local net count overflows"))?;
            terminal_count = terminal_count
                .checked_add(
                    subgraph
                        .pin_drivers
                        .iter()
                        .filter(|pin| !pin.reference.is_empty())
                        .count(),
                )
                .ok_or_else(|| self.limit_error("local net terminal count overflows"))?;
        }
        if net_count > self.limits.max_nets {
            return Err(self.limit_error("local net count exceeds its limit"));
        }
        if terminal_count > self.limits.max_terminals {
            return Err(self.limit_error("local net terminal count exceeds its limit"));
        }
        if net_count != 0 {
            let final_offset = u64::try_from(net_count - 1)
                .map_err(|_| self.limit_error("local net code exceeds the platform size"))?;
            code_offset
                .checked_add(final_offset)
                .ok_or_else(|| self.limit_error("local net code overflows"))?;
        }
        Ok(())
    }

    fn name_net(
        &self,
        subgraph: &SchematicWireSubgraph,
    ) -> Result<(String, bool), SourceBundleError> {
        match subgraph.chosen_kind {
            Some(SchematicWireDriverKind::GlobalLabel)
            | Some(SchematicWireDriverKind::GlobalPowerPin)
            | Some(SchematicWireDriverKind::LocalPowerPin) => {
                Ok((self.escaped_name(&subgraph.chosen_name)?, false))
            }
            Some(SchematicWireDriverKind::LocalLabel)
            | Some(SchematicWireDriverKind::HierarchicalLabel)
            | Some(SchematicWireDriverKind::SheetPin) => {
                let escaped_bytes = self.escaped_name_bytes(&subgraph.chosen_name)?;
                let separator = usize::from(!self.sheet_path.ends_with('/'));
                let output_bytes = self
                    .sheet_path
                    .len()
                    .checked_add(separator)
                    .and_then(|bytes| bytes.checked_add(escaped_bytes))
                    .ok_or_else(|| self.limit_error("local net name bytes overflow"))?;
                self.ensure_name_bytes(output_bytes)?;
                let mut output = String::with_capacity(output_bytes);
                output.push_str(self.sheet_path);
                if separator != 0 {
                    output.push('/');
                }
                push_escaped_name(&mut output, &subgraph.chosen_name);
                Ok((output, false))
            }
            _ if !subgraph.pin_drivers.is_empty() => Ok((self.auto_name(subgraph)?, true)),
            _ => Ok(("unconnected".to_owned(), true)),
        }
    }

    fn auto_name(&self, subgraph: &SchematicWireSubgraph) -> Result<String, SourceBundleError> {
        let isolated = (subgraph.pin_drivers.len() == 1).then(|| &subgraph.pin_drivers[0]);
        let weak_isolated = isolated.is_some_and(|pin| {
            matches!(pin.electrical_type.as_str(), "passive" | "unspecified")
                || pin.electrical_type.contains("no_connect")
                || pin.parent_pin_count == 1
        });
        let forced_unconnected = subgraph.no_connect || weak_isolated;
        let has_component_pin = subgraph
            .pin_drivers
            .iter()
            .any(|pin| !pin.reference.starts_with('#'));
        let mut best: Option<AutoNameCandidate<'_>> = None;
        for (order, pin) in subgraph
            .pin_drivers
            .iter()
            .filter(|pin| !has_component_pin || !pin.reference.starts_with('#'))
            .enumerate()
        {
            let candidate = self.pin_name_candidate(pin, forced_unconnected, order)?;
            if best
                .as_ref()
                .is_none_or(|current| candidate.precedes(current))
            {
                best = Some(candidate);
            }
        }
        let best = best.ok_or_else(|| self.error("auto-named net has no pin candidate"))?;
        let prefix = if best.unconnected {
            "unconnected-("
        } else {
            "Net-("
        };
        let output_bytes = prefix
            .len()
            .checked_add(best.full_segment.len())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or_else(|| self.limit_error("local net name bytes overflow"))?;
        self.ensure_name_bytes(output_bytes)?;
        let mut output = String::with_capacity(output_bytes);
        output.push_str(prefix);
        output.push_str(&best.full_segment);
        output.push(')');
        Ok(output)
    }

    fn pin_name_candidate<'a>(
        &self,
        pin: &'a SchematicPinDriver,
        forced_unconnected: bool,
        order: usize,
    ) -> Result<AutoNameCandidate<'a>, SourceBundleError> {
        let unconnected = forced_unconnected || pin.electrical_type == "no_connect";
        let bare_pad =
            pin.pin_name.is_empty() || pin.pin_name == "~" || pin.pin_name == pin.pin_number;
        let suffix = if bare_pad {
            self.compose_escaped("Pad", &pin.pin_number, "")?
        } else if unconnected || pin.has_multiple {
            self.compose_escaped(&pin.pin_name, &pin.pin_number, "-Pad")?
        } else {
            self.escaped_name(&pin.pin_name)?
        };
        let low_quality = suffix.contains("-Pad") || suffix.starts_with("Pad");
        let reference = if bare_pad || pin.designator_with_unit.is_empty() {
            &pin.reference
        } else {
            &pin.designator_with_unit
        };
        let full_bytes = reference
            .len()
            .checked_add(1)
            .and_then(|bytes| bytes.checked_add(suffix.len()))
            .ok_or_else(|| self.limit_error("auto-name candidate bytes overflow"))?;
        self.ensure_name_bytes(full_bytes)?;
        let mut full_segment = String::with_capacity(full_bytes);
        full_segment.push_str(reference);
        full_segment.push('-');
        full_segment.push_str(&suffix);
        Ok(AutoNameCandidate {
            low_quality,
            full_segment,
            reference,
            pin_number: &pin.pin_number,
            order,
            unconnected,
        })
    }

    fn compose_escaped(
        &self,
        name: &str,
        pin: &str,
        middle: &str,
    ) -> Result<String, SourceBundleError> {
        let name_bytes = self.escaped_name_bytes(name)?;
        let pin_bytes = self.escaped_name_bytes(pin)?;
        let output_bytes = name_bytes
            .checked_add(middle.len())
            .and_then(|bytes| bytes.checked_add(pin_bytes))
            .ok_or_else(|| self.limit_error("auto-name suffix bytes overflow"))?;
        self.ensure_name_bytes(output_bytes)?;
        let mut output = String::with_capacity(output_bytes);
        push_escaped_name(&mut output, name);
        output.push_str(middle);
        push_escaped_name(&mut output, pin);
        Ok(output)
    }

    fn escaped_name(&self, value: &str) -> Result<String, SourceBundleError> {
        let bytes = self.escaped_name_bytes(value)?;
        self.ensure_name_bytes(bytes)?;
        let mut output = String::with_capacity(bytes);
        push_escaped_name(&mut output, value);
        Ok(output)
    }

    fn materialize_terminals(
        &mut self,
        pins: &[SchematicPinDriver],
    ) -> Result<Vec<SchematicLocalNetTerminal>, SourceBundleError> {
        let emitted = pins.iter().filter(|pin| !pin.reference.is_empty()).count();
        let total = self
            .terminal_count
            .checked_add(emitted)
            .ok_or_else(|| self.limit_error("local net terminal count overflows"))?;
        if total > self.limits.max_terminals {
            return Err(self.limit_error("local net terminal count exceeds its limit"));
        }
        let mut terminals = Vec::with_capacity(emitted);
        for pin in pins.iter().filter(|pin| !pin.reference.is_empty()) {
            let svg_id = if pin.pin_svg_id.is_empty() {
                &pin.symbol_uuid
            } else {
                &pin.pin_svg_id
            };
            let bytes = [
                pin.reference.len(),
                pin.pin_number.len(),
                pin.pin_name.len(),
                pin.electrical_type.len(),
                self.sheet_path.len(),
                pin.source_pin_uuid.len(),
                svg_id.len(),
            ]
            .into_iter()
            .try_fold(0_usize, usize::checked_add)
            .ok_or_else(|| self.limit_error("local net retained string bytes overflow"))?;
            self.retain_string_bytes(bytes)?;
            terminals.push(SchematicLocalNetTerminal {
                symbol_index: pin.symbol_index,
                designator: pin.reference.clone(),
                pin: pin.pin_number.clone(),
                pin_name: pin.pin_name.clone(),
                pin_type: pin.electrical_type.clone(),
                sheet_path: self.sheet_path.to_owned(),
                source_pin_id: pin.source_pin_uuid.clone(),
                svg_id: svg_id.to_owned(),
            });
        }
        self.terminal_count = total;
        Ok(terminals)
    }

    fn uniquify_sheet_pin_names(
        &mut self,
        nets: &mut [SchematicLocalNet],
    ) -> Result<(), SourceBundleError> {
        let suffixes = {
            let mut seen = HashMap::<&str, usize>::new();
            nets.iter()
                .map(|net| {
                    if net.driver_kind != Some(SchematicWireDriverKind::SheetPin) {
                        return None;
                    }
                    let count = seen.entry(&net.name).or_default();
                    let suffix = (*count != 0).then_some(*count);
                    *count += 1;
                    suffix
                })
                .collect::<Vec<_>>()
        };
        for (net, suffix) in nets.iter_mut().zip(suffixes) {
            let Some(suffix) = suffix else {
                continue;
            };
            let suffix = format!("_{suffix}");
            let final_bytes = net
                .name
                .len()
                .checked_add(suffix.len())
                .ok_or_else(|| self.limit_error("local net name bytes overflow"))?;
            self.ensure_name_bytes(final_bytes)?;
            self.retain_string_bytes(suffix.len())?;
            net.name.push_str(&suffix);
        }
        Ok(())
    }

    fn ensure_name_bytes(&self, bytes: usize) -> Result<(), SourceBundleError> {
        if bytes > self.limits.max_name_bytes {
            return Err(self.limit_error("local net name bytes exceed their limit"));
        }
        Ok(())
    }

    fn escaped_name_bytes(&self, value: &str) -> Result<usize, SourceBundleError> {
        escaped_name_bytes(value)
            .ok_or_else(|| self.limit_error("escaped local net name bytes overflow"))
    }

    fn retain_string_bytes(&mut self, bytes: usize) -> Result<(), SourceBundleError> {
        let total = self
            .retained_string_bytes
            .checked_add(bytes)
            .ok_or_else(|| self.limit_error("local net retained string bytes overflow"))?;
        if total > self.limits.max_retained_string_bytes {
            return Err(self.limit_error("local net retained string bytes exceed their limit"));
        }
        self.retained_string_bytes = total;
        Ok(())
    }

    fn error(&self, message: &str) -> SourceBundleError {
        SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(self.source_path),
            message,
        )
    }

    fn limit_error(&self, message: &str) -> SourceBundleError {
        SourceBundleError::new(
            SourceBundleErrorKind::ResourceLimit,
            Some(self.source_path),
            message,
        )
    }
}

struct AutoNameCandidate<'a> {
    low_quality: bool,
    full_segment: String,
    reference: &'a str,
    pin_number: &'a str,
    order: usize,
    unconnected: bool,
}

impl AutoNameCandidate<'_> {
    fn precedes(&self, other: &Self) -> bool {
        (
            self.low_quality,
            &self.full_segment,
            &self.reference,
            &self.pin_number,
            self.order,
        ) < (
            other.low_quality,
            &other.full_segment,
            &other.reference,
            &other.pin_number,
            other.order,
        )
    }
}

fn escaped_name_bytes(value: &str) -> Option<usize> {
    value.bytes().try_fold(0_usize, |bytes, value| {
        let added = match value {
            b'/' => 7,
            b'\r' | b'\n' => 0,
            _ => 1,
        };
        bytes.checked_add(added)
    })
}

fn push_escaped_name(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '/' => output.push_str("{slash}"),
            '\r' | '\n' => {}
            _ => output.push(character),
        }
    }
}
