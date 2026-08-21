use crate::{
    SchematicBundleLimits, SchematicDefinition, SchematicEffectiveSymbol, SchematicLibraryPin,
    SchematicOccurrence, SchematicPlacedSymbol, SchematicPoint, SourceBundleError,
    SourceBundleErrorKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolTerminal {
    pub symbol_index: usize,
    pub symbol_uuid: String,
    pub reference: String,
    pub pin_number: String,
    pub pin_name: String,
    pub electrical_type: String,
    pub graphic_style: String,
    pub hidden: bool,
    pub has_drawing: bool,
    pub library_at: SchematicPoint,
    pub at: SchematicPoint,
}

pub(crate) fn resolve_symbol_terminals(
    definition: &SchematicDefinition,
    occurrence: &SchematicOccurrence,
    effective_symbols: &[SchematicEffectiveSymbol],
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicSymbolTerminal>, SourceBundleError> {
    let mut terminals = Vec::new();
    let mut retained_string_bytes = 0_usize;
    for effective in effective_symbols {
        let Some(placed) = definition.symbols.get(effective.symbol_index) else {
            continue;
        };
        let Some(library) = definition.library_pin_symbol_for_placement(placed) else {
            continue;
        };
        for subsymbol in &library.subsymbols {
            if !active_subsymbol(
                subsymbol.unit,
                subsymbol.style,
                effective.unit,
                placed.convert,
            ) {
                continue;
            }
            for pin in &subsymbol.pins {
                if terminals.len() >= limits.max_symbol_terminals_per_occurrence {
                    return Err(limit_error(
                        occurrence,
                        "symbol terminal count exceeds its limit",
                    ));
                }
                let terminal_bytes =
                    terminal_string_bytes(placed, effective, pin).ok_or_else(|| {
                        limit_error(occurrence, "symbol terminal retained bytes overflow")
                    })?;
                retained_string_bytes = retained_string_bytes
                    .checked_add(terminal_bytes)
                    .ok_or_else(|| {
                        limit_error(occurrence, "symbol terminal retained bytes overflow")
                    })?;
                if retained_string_bytes > limits.max_symbol_terminal_retained_bytes_per_occurrence
                {
                    return Err(limit_error(
                        occurrence,
                        "symbol terminal retained bytes exceed their limit",
                    ));
                }
                terminals.push(SchematicSymbolTerminal {
                    symbol_index: effective.symbol_index,
                    symbol_uuid: placed.uuid.clone(),
                    reference: effective.reference.clone(),
                    pin_number: pin.number.clone(),
                    pin_name: pin.name.clone(),
                    electrical_type: pin.electrical_type.clone(),
                    graphic_style: pin.graphic_style.clone(),
                    hidden: pin.hidden,
                    has_drawing: placed_pin_has_drawing(library, placed, pin),
                    library_at: pin.at,
                    at: transform_pin(placed, pin, occurrence)?,
                });
            }
        }
    }
    Ok(terminals)
}

fn terminal_string_bytes(
    placed: &SchematicPlacedSymbol,
    effective: &SchematicEffectiveSymbol,
    pin: &SchematicLibraryPin,
) -> Option<usize> {
    [
        placed.uuid.len(),
        effective.reference.len(),
        pin.number.len(),
        pin.name.len(),
        pin.electrical_type.len(),
        pin.graphic_style.len(),
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
}

fn placed_pin_has_drawing(
    library: &crate::SchematicLibrarySymbol,
    placed: &SchematicPlacedSymbol,
    pin: &SchematicLibraryPin,
) -> bool {
    if pin.hidden {
        return false;
    }
    // A selected alternate can replace both the pin name and graphic style.
    // The compact connectivity model does not retain alternate definitions, so
    // preserve the selector conservatively whenever an alternate is selected.
    if placed
        .pins
        .iter()
        .any(|placed_pin| placed_pin.number == pin.number && placed_pin.alternate.is_some())
    {
        return true;
    }
    let draws_geometry = pin.graphic_style != "line" || !pin.length_is_zero;
    let draws_name = !library.pin_names_hide
        && !matches!(pin.name.as_str(), "" | "~")
        && pin.name_has_visible_size;
    let draws_number =
        !library.pin_numbers_hide && !pin.number.is_empty() && pin.number_has_visible_size;
    draws_geometry || draws_name || draws_number
}

fn active_subsymbol(unit: i64, style: i64, placed_unit: i64, convert: i64) -> bool {
    (unit == 0 || unit == placed_unit) && (style == 0 || style == convert)
}

fn transform_pin(
    symbol: &SchematicPlacedSymbol,
    pin: &SchematicLibraryPin,
    occurrence: &SchematicOccurrence,
) -> Result<SchematicPoint, SourceBundleError> {
    let x = pin.at.x_iu;
    let y = pin
        .at
        .y_iu
        .checked_neg()
        .ok_or_else(|| error(occurrence, "library pin Y transform overflows"))?;
    let angle = symbol.angle_degrees.rem_euclid(360.0);
    let (mut x, mut y) = if angle == 0.0 {
        (x, y)
    } else if angle == 90.0 {
        (y, checked_neg(x, occurrence)?)
    } else if angle == 180.0 {
        (checked_neg(x, occurrence)?, checked_neg(y, occurrence)?)
    } else if angle == 270.0 {
        (checked_neg(y, occurrence)?, x)
    } else {
        arbitrary_rotation(x, y, angle, occurrence)?
    };
    match symbol.mirror.as_deref() {
        Some("x") => y = checked_neg(y, occurrence)?,
        Some("y") => x = checked_neg(x, occurrence)?,
        _ => {}
    }
    Ok(SchematicPoint {
        x_iu: symbol
            .at
            .x_iu
            .checked_add(x)
            .ok_or_else(|| error(occurrence, "library pin X translation overflows"))?,
        y_iu: symbol
            .at
            .y_iu
            .checked_add(y)
            .ok_or_else(|| error(occurrence, "library pin Y translation overflows"))?,
    })
}

fn arbitrary_rotation(
    x: i64,
    y: i64,
    angle: f64,
    occurrence: &SchematicOccurrence,
) -> Result<(i64, i64), SourceBundleError> {
    let radians = angle.to_radians();
    let cosine = radians.cos();
    let sine = radians.sin();
    let rotated_x = (x as f64).mul_add(cosine, y as f64 * sine);
    let rotated_y = (-(x as f64)).mul_add(sine, y as f64 * cosine);
    Ok((
        rounded_i64(rotated_x, occurrence)?,
        rounded_i64(rotated_y, occurrence)?,
    ))
}

fn rounded_i64(value: f64, occurrence: &SchematicOccurrence) -> Result<i64, SourceBundleError> {
    let rounded = value.round_ties_even();
    if !rounded.is_finite() || rounded < i64::MIN as f64 || rounded >= -(i64::MIN as f64) {
        return Err(error(occurrence, "library pin rotation exceeds i64"));
    }
    Ok(rounded as i64)
}

fn checked_neg(value: i64, occurrence: &SchematicOccurrence) -> Result<i64, SourceBundleError> {
    value
        .checked_neg()
        .ok_or_else(|| error(occurrence, "library pin transform overflows"))
}

fn error(occurrence: &SchematicOccurrence, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Schematic,
        Some(&occurrence.source_path),
        message,
    )
}

fn limit_error(occurrence: &SchematicOccurrence, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        Some(&occurrence.source_path),
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::rounded_i64;
    use crate::SchematicOccurrence;

    #[test]
    fn half_grid_rounding_is_ties_to_even() {
        let occurrence = occurrence();
        assert_eq!(rounded_i64(2.5, &occurrence), Ok(2));
        assert_eq!(rounded_i64(3.5, &occurrence), Ok(4));
        assert_eq!(rounded_i64(-2.5, &occurrence), Ok(-2));
        assert_eq!(rounded_i64(-3.5, &occurrence), Ok(-4));
    }

    #[test]
    fn arbitrary_rotation_rejects_non_finite_and_out_of_range_values() {
        let occurrence = occurrence();
        assert!(rounded_i64(f64::INFINITY, &occurrence).is_err());
        assert!(rounded_i64(i64::MAX as f64, &occurrence).is_err());
    }

    fn occurrence() -> SchematicOccurrence {
        SchematicOccurrence {
            index: 1,
            source_path: "test.kicad_sch".to_owned(),
            parent_index: None,
            parent_sheet_index: None,
            sheet_uuid: None,
            sheet_name: String::new(),
            sheet_file: "test.kicad_sch".to_owned(),
            occurrence_address: "/".to_owned(),
            legacy_address: "/".to_owned(),
            human_address: "/".to_owned(),
            effective_in_bom: true,
            effective_on_board: true,
            effective_dnp: false,
            effective_exclude_from_sim: false,
        }
    }
}
