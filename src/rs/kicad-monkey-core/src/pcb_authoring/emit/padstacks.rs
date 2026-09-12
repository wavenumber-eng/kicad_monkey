use super::*;

pub(super) fn padstack(value: &AuthoredPadstack) -> Sexp {
    let mut fields = vec![form("mode", [atom(value.mode.as_str())])];
    for row in &value.layers {
        let mut layer = vec![
            quoted(row.layer.as_str()),
            form("shape", [atom(row.shape.as_str())]),
            form("size", [float(row.size_x_mm), float(row.size_y_mm)]),
        ];
        if row.offset != AuthoredPoint::default() {
            layer.push(point_form("offset", row.offset));
        }
        append_shape(&row.shape, &mut layer);
        for (name, value) in [
            ("clearance", row.clearance_mm),
            ("thermal_bridge_width", row.thermal_bridge_width_mm),
            ("thermal_gap", row.thermal_gap_mm),
        ] {
            if let Some(value) = value {
                layer.push(form(name, [float(value)]));
            }
        }
        if let Some(value) = row.zone_connect {
            layer.push(form("zone_connect", [integer(value.source_code())]));
        }
        fields.push(form("layer", layer));
    }
    form("padstack", fields)
}

pub(super) fn viastack(value: &AuthoredViaStack) -> Sexp {
    let mut fields = vec![form("mode", [atom(value.mode.as_str())])];
    fields.extend(value.layers.iter().map(|row| {
        form(
            "layer",
            [
                quoted(row.layer.as_str()),
                form("size", [float(row.size_mm)]),
            ],
        )
    }));
    form("padstack", fields)
}
