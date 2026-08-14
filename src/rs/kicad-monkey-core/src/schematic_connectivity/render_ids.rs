use crate::{SchematicSheet, SchematicSheetPin};

pub(crate) fn schematic_sheet_pin_group_id(
    sheet: &SchematicSheet,
    pin: &SchematicSheetPin,
) -> String {
    if !pin.uuid.is_empty() {
        return pin.uuid.clone();
    }
    if sheet.uuid.is_empty() || pin.name.is_empty() {
        return sheet.uuid.clone();
    }
    let token = sanitized_id_token(&pin.name);
    format!("{}__sheet_pin__{token}", sheet.uuid)
}

fn sanitized_id_token(value: &str) -> String {
    let mut output = String::new();
    let mut replacing = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':' | '-') {
            output.push(character);
            replacing = false;
        } else if !replacing {
            output.push('_');
            replacing = true;
        }
    }
    let token = output.trim_matches('_');
    if token.is_empty() {
        "pin".to_owned()
    } else {
        token.to_owned()
    }
}
