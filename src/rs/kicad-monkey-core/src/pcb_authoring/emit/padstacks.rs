use super::*;

pub(super) fn padstack(value: &AuthoredPadstack) -> Sexp {
    let mut fields = vec![form("mode", [atom(value.mode.as_str())])];
    for row in &value.layers {
        let mut layer = vec![quoted(row.layer().as_str())];
        if let AuthoredPadstackLayer::Land {
            shape,
            size_x_mm,
            size_y_mm,
            offset,
            clearance_mm,
            thermal_bridge_width_mm,
            thermal_gap_mm,
            zone_connect,
            ..
        } = row
        {
            layer.extend([
                form("shape", [atom(shape.as_str())]),
                form("size", [float(*size_x_mm), float(*size_y_mm)]),
            ]);
            if *offset != AuthoredPoint::default() {
                layer.push(point_form("offset", *offset));
            }
            append_shape(shape, &mut layer);
            for (name, value) in [
                ("clearance", clearance_mm),
                ("thermal_bridge_width", thermal_bridge_width_mm),
                ("thermal_gap", thermal_gap_mm),
            ] {
                if let Some(value) = value {
                    layer.push(form(name, [float(*value)]));
                }
            }
            if let Some(value) = zone_connect {
                layer.push(form("zone_connect", [integer(value.source_code())]));
            }
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
