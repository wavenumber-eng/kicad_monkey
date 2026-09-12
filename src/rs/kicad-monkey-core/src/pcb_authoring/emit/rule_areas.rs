use super::*;

pub(super) fn rule_area(value: &AuthoredRuleArea) -> Sexp {
    let mut children = vec![
        atom("zone"),
        form("net", [integer(0)]),
        form("net_name", [quoted("")]),
        form("uuid", [quoted(&value.uuid)]),
    ];
    if value.layers.len() == 1 {
        children.push(form("layer", [quoted(&value.layers[0])]));
    } else {
        children.push(form(
            "layers",
            value.layers.iter().map(|layer| quoted(layer)),
        ));
    }
    if value.locked {
        children.push(form("locked", [yes_no(true)]));
    }
    if let Some(name) = &value.name {
        children.push(form("name", [quoted(name)]));
    }
    children.push(form(
        "hatch",
        [atom(value.hatch.as_str()), float(value.hatch_pitch_mm)],
    ));
    children.push(form(
        "keepout",
        [
            ("tracks", value.keepout.tracks),
            ("vias", value.keepout.vias),
            ("pads", value.keepout.pads),
            ("copperpour", value.keepout.copperpour),
            ("footprints", value.keepout.footprints),
        ]
        .map(|(name, setting)| form(name, [atom(setting.as_str())])),
    ));
    if let Some(placement) = &value.placement {
        let (kind, name) = placement.source.source_pair();
        children.push(form(
            "placement",
            [
                form("enabled", [yes_no(placement.enabled)]),
                form(kind, [quoted(name)]),
            ],
        ));
    }
    children.extend(value.outlines.iter().map(zone_polygon));
    Sexp::List(children)
}
