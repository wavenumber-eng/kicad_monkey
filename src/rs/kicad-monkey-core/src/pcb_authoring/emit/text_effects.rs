//! Source text font and justification emission.

use super::*;

pub(super) fn effects(value: &AuthoredTextEffects) -> Sexp {
    let mut children = vec![atom("effects"), font_effects(value)];
    let justification = text_justification(value);
    if !justification.is_empty() {
        children.push(form("justify", justification));
    }
    if let Some(href) = &value.href {
        children.push(form("href", [quoted(href)]));
    }
    Sexp::List(children)
}

fn font_effects(value: &AuthoredTextEffects) -> Sexp {
    let mut font = vec![atom("font")];
    if let Some(face) = &value.face {
        font.push(form("face", [quoted(face)]));
    }
    font.push(form(
        "size",
        [float(value.size_y_mm), float(value.size_x_mm)],
    ));
    if let Some(thickness) = value.thickness_mm {
        font.push(form("thickness", [float(thickness)]));
    }
    if let Some(line_spacing) = value.line_spacing {
        font.push(form("line_spacing", [float(line_spacing)]));
    }
    if value.bold {
        font.push(form("bold", [atom("yes")]));
    }
    if value.italic {
        font.push(atom("italic"));
    }
    if let Some(color) = value.color {
        font.push(form(
            "color",
            [
                integer(color.red),
                integer(color.green),
                integer(color.blue),
                float(color.alpha),
            ],
        ));
    }
    Sexp::List(font)
}

fn text_justification(value: &AuthoredTextEffects) -> Vec<Sexp> {
    let mut justification = Vec::with_capacity(3);
    match value.horizontal_justify {
        AuthoredTextHorizontalJustification::Left => justification.push(atom("left")),
        AuthoredTextHorizontalJustification::Center => {}
        AuthoredTextHorizontalJustification::Right => justification.push(atom("right")),
    }
    match value.vertical_justify {
        AuthoredTextVerticalJustification::Top => justification.push(atom("top")),
        AuthoredTextVerticalJustification::Center => {}
        AuthoredTextVerticalJustification::Bottom => justification.push(atom("bottom")),
    }
    if value.mirrored {
        justification.push(atom("mirror"));
    }
    justification
}
