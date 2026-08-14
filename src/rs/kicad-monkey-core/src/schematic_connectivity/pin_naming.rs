use crate::{
    SchematicDefinition, SchematicEffectiveSymbol, SchematicSymbolTerminal, SourceBundleError,
    SourceBundleErrorKind,
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicSubpartSettings {
    pub first_id: u32,
    pub separator: u32,
}

impl Default for SchematicSubpartSettings {
    fn default() -> Self {
        Self {
            first_id: u32::from(b'A'),
            separator: 0,
        }
    }
}

pub(super) struct PinNaming {
    pub has_multiple: bool,
    pub designator_with_unit: String,
    pub parent_pin_count: usize,
    pub source_pin_uuid: String,
    pub pin_svg_id: String,
}

pub(super) fn build_pin_namings(
    definition: &SchematicDefinition,
    terminals: &[&SchematicSymbolTerminal],
    effective: &[SchematicEffectiveSymbol],
    settings: SchematicSubpartSettings,
    max_string_bytes: usize,
) -> Result<Vec<PinNaming>, SourceBundleError> {
    let mut parent_pin_counts = HashMap::<usize, usize>::new();
    let mut name_groups = HashMap::<(usize, &str, bool), (&str, bool)>::new();
    for &terminal in terminals {
        *parent_pin_counts.entry(terminal.symbol_index).or_default() += 1;
        name_groups
            .entry(pin_name_key(terminal))
            .and_modify(|(first_number, multiple)| {
                if *first_number != terminal.pin_number {
                    *multiple = true;
                }
            })
            .or_insert((terminal.pin_number.as_str(), false));
    }
    let active_symbols = terminals
        .iter()
        .map(|terminal| terminal.symbol_index)
        .collect::<HashSet<_>>();
    let mut source_pin_uuids = HashMap::new();
    for symbol_index in active_symbols {
        let Some(symbol) = definition.symbols.get(symbol_index) else {
            continue;
        };
        for pin in &symbol.pins {
            source_pin_uuids.insert((symbol_index, pin.number.as_str()), pin.uuid.as_str());
        }
    }
    let mut retained_string_bytes = 0_usize;
    let mut namings = Vec::with_capacity(terminals.len());
    for &terminal in terminals {
        let symbol = effective.get(terminal.symbol_index).ok_or_else(|| {
            error(
                definition,
                SourceBundleErrorKind::Schematic,
                "terminal effective symbol is missing",
            )
        })?;
        let source_pin_uuid = source_pin_uuids
            .get(&(terminal.symbol_index, terminal.pin_number.as_str()))
            .copied()
            .unwrap_or_default();
        let designator_with_unit =
            designator_with_unit(definition, terminal, symbol, settings, max_string_bytes)?;
        let pin_svg_id =
            schematic_pin_svg_id(definition, terminal, source_pin_uuid, max_string_bytes)?;
        let added_bytes = designator_with_unit
            .len()
            .checked_add(source_pin_uuid.len())
            .and_then(|bytes| bytes.checked_add(pin_svg_id.len()))
            .ok_or_else(|| limit_error(definition, "pin naming retained string bytes overflow"))?;
        retained_string_bytes = retained_string_bytes
            .checked_add(added_bytes)
            .ok_or_else(|| limit_error(definition, "pin naming retained string bytes overflow"))?;
        ensure_bytes(definition, retained_string_bytes, max_string_bytes)?;
        namings.push(PinNaming {
            has_multiple: name_groups
                .get(&pin_name_key(terminal))
                .is_some_and(|value| value.1),
            designator_with_unit,
            parent_pin_count: parent_pin_counts
                .get(&terminal.symbol_index)
                .copied()
                .unwrap_or_default(),
            source_pin_uuid: source_pin_uuid.to_owned(),
            pin_svg_id,
        });
    }
    Ok(namings)
}

fn pin_name_key(terminal: &SchematicSymbolTerminal) -> (usize, &str, bool) {
    (
        terminal.symbol_index,
        terminal.pin_name.as_str(),
        terminal.electrical_type == "no_connect",
    )
}

fn designator_with_unit(
    definition: &SchematicDefinition,
    terminal: &SchematicSymbolTerminal,
    symbol: &SchematicEffectiveSymbol,
    settings: SchematicSubpartSettings,
    max_string_bytes: usize,
) -> Result<String, SourceBundleError> {
    let placed = &definition.symbols[terminal.symbol_index];
    let unit_count = definition
        .library_pin_symbol_for_placement(placed)
        .map_or(1, |library| {
            library
                .subsymbols
                .iter()
                .map(|subsymbol| subsymbol.unit)
                .max()
                .unwrap_or(1)
                .max(1)
        });
    if unit_count <= 1 {
        ensure_bytes(definition, terminal.reference.len(), max_string_bytes)?;
        return Ok(terminal.reference.clone());
    }
    let suffix = subpart_reference(symbol.unit, settings)
        .map_err(|message| error(definition, SourceBundleErrorKind::Schematic, message))?;
    let output_bytes = terminal
        .reference
        .len()
        .checked_add(suffix.len())
        .ok_or_else(|| limit_error(definition, "unit-suffixed reference bytes overflow"))?;
    ensure_bytes(definition, output_bytes, max_string_bytes)?;
    let mut value = String::with_capacity(output_bytes);
    value.push_str(&terminal.reference);
    value.push_str(&suffix);
    Ok(value)
}

fn subpart_reference(
    unit: i64,
    settings: SchematicSubpartSettings,
) -> Result<String, &'static str> {
    if unit < 1 {
        return Ok(String::new());
    }
    let mut output = String::new();
    if settings.separator != 0 {
        output.push(
            char::from_u32(settings.separator)
                .ok_or("schematic subpart separator is not a Unicode scalar")?,
        );
    }
    if (u32::from(b'0')..=u32::from(b'9')).contains(&settings.first_id) {
        output.push_str(&unit.to_string());
        return Ok(output);
    }
    let mut unit = u64::try_from(unit).map_err(|_| "schematic subpart unit is invalid")?;
    let mut letters = Vec::new();
    while unit > 0 {
        let offset = u32::try_from((unit - 1) % 26)
            .map_err(|_| "schematic subpart letter offset overflows")?;
        let codepoint = settings
            .first_id
            .checked_add(offset)
            .ok_or("schematic subpart letter overflows")?;
        letters.push(
            char::from_u32(codepoint).ok_or("schematic subpart letter is not a Unicode scalar")?,
        );
        unit = (unit - u64::from(offset)) / 26;
    }
    output.extend(letters.into_iter().rev());
    Ok(output)
}

fn schematic_pin_svg_id(
    definition: &SchematicDefinition,
    terminal: &SchematicSymbolTerminal,
    source_pin_uuid: &str,
    max_string_bytes: usize,
) -> Result<String, SourceBundleError> {
    if terminal.hidden {
        return Ok(String::new());
    }
    if !source_pin_uuid.is_empty() {
        ensure_bytes(definition, source_pin_uuid.len(), max_string_bytes)?;
        return Ok(source_pin_uuid.to_owned());
    }
    if terminal.symbol_uuid.is_empty() || terminal.pin_number.is_empty() {
        ensure_bytes(definition, terminal.symbol_uuid.len(), max_string_bytes)?;
        return Ok(terminal.symbol_uuid.clone());
    }
    let token = sanitized_pin_token(&terminal.pin_number);
    let output_bytes = terminal
        .symbol_uuid
        .len()
        .checked_add("__pin__".len())
        .and_then(|bytes| bytes.checked_add(token.len()))
        .ok_or_else(|| limit_error(definition, "pin SVG identifier bytes overflow"))?;
    ensure_bytes(definition, output_bytes, max_string_bytes)?;
    let mut output = String::with_capacity(output_bytes);
    output.push_str(&terminal.symbol_uuid);
    output.push_str("__pin__");
    output.push_str(&token);
    Ok(output)
}

fn sanitized_pin_token(pin_number: &str) -> String {
    let mut token = String::with_capacity(pin_number.len());
    let mut replacing = false;
    for character in pin_number.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':' | '-') {
            token.push(character);
            replacing = false;
        } else if !replacing {
            token.push('_');
            replacing = true;
        }
    }
    let Some(start) = token.bytes().position(|value| value != b'_') else {
        return "pin".to_owned();
    };
    let end = token
        .bytes()
        .rposition(|value| value != b'_')
        .map_or(start, |index| index + 1);
    token.truncate(end);
    if start != 0 {
        token.drain(..start);
    }
    token
}

fn ensure_bytes(
    definition: &SchematicDefinition,
    bytes: usize,
    maximum: usize,
) -> Result<(), SourceBundleError> {
    if bytes > maximum {
        Err(limit_error(
            definition,
            "pin naming string bytes exceed their limit",
        ))
    } else {
        Ok(())
    }
}

fn limit_error(definition: &SchematicDefinition, message: &str) -> SourceBundleError {
    error(definition, SourceBundleErrorKind::ResourceLimit, message)
}

fn error(
    definition: &SchematicDefinition,
    kind: SourceBundleErrorKind,
    message: &str,
) -> SourceBundleError {
    SourceBundleError::new(kind, Some(&definition.source_path), message)
}

#[cfg(test)]
mod tests {
    use super::{SchematicSubpartSettings, subpart_reference};

    #[test]
    fn subpart_references_match_kicad_letter_digit_and_separator_rules() {
        let upper = SchematicSubpartSettings::default();
        assert_eq!(subpart_reference(0, upper), Ok(String::new()));
        assert_eq!(subpart_reference(1, upper), Ok("A".to_owned()));
        assert_eq!(subpart_reference(26, upper), Ok("Z".to_owned()));
        assert_eq!(subpart_reference(27, upper), Ok("AA".to_owned()));
        assert_eq!(
            subpart_reference(
                2,
                SchematicSubpartSettings {
                    first_id: u32::from(b'1'),
                    separator: u32::from(b'.'),
                },
            ),
            Ok(".2".to_owned())
        );
        assert!(
            subpart_reference(
                1,
                SchematicSubpartSettings {
                    first_id: u32::from(char::MAX),
                    separator: 0,
                },
            )
            .is_ok()
        );
        assert!(
            subpart_reference(
                2,
                SchematicSubpartSettings {
                    first_id: u32::from(char::MAX),
                    separator: 0,
                },
            )
            .is_err()
        );
    }
}
