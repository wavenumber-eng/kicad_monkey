use super::resource::ResourceEncoder;
use super::*;
mod custom_pads;
mod text_effects;
use text_effects::effects;
mod padstacks;
mod rule_areas;
use crate::sexpr::{ErrorKind, Sexp, build_with_limit, parse_bytes};
use crate::text_render_cache::{
    TextRenderCacheErrorKind, TextRenderCacheLimits, write_text_render_cache_a0,
};

pub(super) fn board_text(board: &AuthoredPcb, limits: PcbAuthoringLimits) -> Result<String, Error> {
    validate::board(board, limits)?;
    let mut resources = ResourceEncoder::new(
        limits.max_resource_work_bytes,
        limits.max_resource_encoded_bytes,
    );
    let mut values = vec![
        atom("kicad_pcb"),
        form("version", [integer(board.version)]),
        form("generator", [quoted(&board.generator)]),
        form("generator_version", [quoted(&board.generator_version)]),
        form("general", [form("thickness", [float(board.thickness_mm)])]),
        form("paper", [quoted(&board.paper)]),
        layer_table(&board.layers),
        setup(&board.setup),
    ];
    values.extend(board.properties.iter().map(|property| {
        form(
            "property",
            [quoted(&property.name), quoted(&property.value)],
        )
    }));
    if !board.embedded_files.is_empty() {
        values.push(resources.files(&board.embedded_files)?);
    }
    values.extend(board.nets.iter().map(net));
    for occurrence in &board.footprints {
        values.push(footprint_form(
            &occurrence.footprint,
            FootprintEnvelope::Occurrence(occurrence),
            &mut resources,
            limits,
        )?);
    }
    values.extend(board.profile.iter().map(|value| graphic(value, "gr_")));
    values.extend(board.graphics.iter().map(|value| graphic(value, "gr_")));
    for value in &board.texts {
        values.push(board_text_value(value, limits)?);
    }
    for value in &board.text_boxes {
        values.push(text_box(value, "gr_", None, limits)?);
    }
    values.extend(board.vias.iter().map(via));
    values.extend(board.segments.iter().map(segment));
    values.extend(board.arcs.iter().map(routing_arc));
    values.extend(board.zones.iter().map(zone));
    values.extend(board.rule_areas.iter().map(rule_areas::rule_area));
    build_document(Sexp::List(values), limits.max_output_bytes)
}

pub(super) fn footprint_text(
    footprint: &AuthoredStandaloneFootprint,
    limits: PcbAuthoringLimits,
) -> Result<String, Error> {
    validate::footprint(footprint, limits)?;
    let mut resources = ResourceEncoder::new(
        limits.max_resource_work_bytes,
        limits.max_resource_encoded_bytes,
    );
    let value = footprint_form(
        &footprint.footprint,
        FootprintEnvelope::Standalone(footprint),
        &mut resources,
        limits,
    )?;
    build_document(value, limits.max_output_bytes)
}

fn build_document(value: Sexp, maximum: usize) -> Result<String, Error> {
    let mut output = build_with_limit(&value, maximum.saturating_sub(1))?;
    if output.len() == maximum {
        return Err(Error::build(
            ErrorKind::ResourceLimit,
            "authored source exceeds max_output_bytes",
        ));
    }
    output.push('\n');
    Ok(output)
}

fn layer_table(layers: &[AuthoredLayer]) -> Sexp {
    let mut children = Vec::with_capacity(layers.len().saturating_add(1));
    children.push(atom("layers"));
    for layer in layers {
        let mut values = vec![
            integer(layer.ordinal),
            quoted(&layer.name),
            atom(&layer.kind),
        ];
        if let Some(user_name) = &layer.user_name {
            values.push(quoted(user_name));
        }
        children.push(Sexp::List(values));
    }
    Sexp::List(children)
}

fn setup(value: &AuthoredSetup) -> Sexp {
    let mut children = vec![
        atom("setup"),
        point_form("aux_axis_origin", value.aux_axis_origin),
        point_form("grid_origin", value.grid_origin),
        form(
            "pad_to_mask_clearance",
            [float(value.pad_to_mask_clearance_mm)],
        ),
        form(
            "pad_to_paste_clearance",
            [float(value.pad_to_paste_clearance_mm)],
        ),
        form(
            "pad_to_paste_clearance_ratio",
            [float(value.pad_to_paste_clearance_ratio)],
        ),
    ];
    if value.allow_soldermask_bridges_in_footprints {
        children.push(form(
            "allow_soldermask_bridges_in_footprints",
            [atom("yes")],
        ));
    }
    // Emit even the empty clause: both false is an explicit board policy,
    // independent of per-via source overrides and old-version defaults.
    children.push(form(
        "tenting",
        [("front", value.tenting_front), ("back", value.tenting_back)]
            .into_iter()
            .filter(|(_, enabled)| *enabled)
            .map(|(side, _)| atom(side)),
    ));
    if let Some(stackup) = &value.stackup {
        children.push(stackup_form(stackup));
    }
    Sexp::List(children)
}

fn stackup_form(value: &AuthoredStackup) -> Sexp {
    let mut children = Vec::with_capacity(value.layers.len().saturating_add(5));
    children.push(atom("stackup"));
    children.extend(value.layers.iter().map(stackup_layer));
    if let Some(finish) = &value.copper_finish {
        children.push(form("copper_finish", [quoted(finish)]));
    }
    if value.dielectric_constraints {
        children.push(form("dielectric_constraints", [atom("yes")]));
    }
    if let Some(connector) = &value.edge_connector {
        children.push(form("edge_connector", [quoted(connector)]));
    }
    if value.edge_plating {
        children.push(form("edge_plating", [atom("yes")]));
    }
    Sexp::List(children)
}

fn stackup_layer(value: &AuthoredStackupLayer) -> Sexp {
    let mut children = vec![
        atom("layer"),
        quoted(&value.name),
        form("type", [quoted(&value.type_name)]),
    ];
    if let Some(thickness) = value.thickness_mm {
        let mut values = vec![float(thickness)];
        if value.thickness_locked {
            values.push(atom("locked"));
        }
        children.push(form("thickness", values));
    }
    if let Some(material) = &value.material {
        children.push(form("material", [quoted(material)]));
    }
    if let Some(epsilon_r) = value.epsilon_r {
        children.push(form("epsilon_r", [float(epsilon_r)]));
    }
    if let Some(loss_tangent) = value.loss_tangent {
        children.push(form("loss_tangent", [float(loss_tangent)]));
    }
    if let Some(color) = &value.color {
        children.push(form("color", [quoted(color)]));
    }
    Sexp::List(children)
}

fn net(value: &AuthoredNet) -> Sexp {
    form("net", [integer(value.code), quoted(&value.name)])
}

#[derive(Clone, Copy)]
enum FootprintEnvelope<'a> {
    Standalone(&'a AuthoredStandaloneFootprint),
    Occurrence(&'a AuthoredFootprintOccurrence),
}

fn footprint_form(
    value: &AuthoredFootprint,
    envelope: FootprintEnvelope<'_>,
    resources: &mut ResourceEncoder,
    limits: PcbAuthoringLimits,
) -> Result<Sexp, Error> {
    let mut children = vec![atom("footprint"), quoted(&value.name)];
    let occurrence = match envelope {
        FootprintEnvelope::Standalone(_) => None,
        FootprintEnvelope::Occurrence(value) => Some(value),
    };
    match envelope {
        FootprintEnvelope::Standalone(standalone) => {
            children.extend([
                form("version", [integer(standalone.version)]),
                form("generator", [quoted(&standalone.generator)]),
                form("generator_version", [quoted(&standalone.generator_version)]),
                form("layer", [quoted(&standalone.layer)]),
            ]);
        }
        FootprintEnvelope::Occurrence(occurrence) => {
            append_occurrence(occurrence, &mut children);
        }
    }
    append_footprint_metadata(value, &mut children);
    for property in &value.properties {
        children.push(footprint_property(property, occurrence, limits)?);
    }
    for text in &value.texts {
        children.push(footprint_text_value(text, occurrence, limits)?);
    }
    for box_value in &value.text_boxes {
        children.push(text_box(box_value, "fp_", occurrence, limits)?);
    }
    children.extend(value.graphics.iter().map(|item| graphic(item, "fp_")));
    children.extend(value.pads.iter().map(pad));
    children.extend(value.models.iter().map(model));
    if !value.embedded_files.is_empty() {
        children.push(resources.files(&value.embedded_files)?);
    }
    Ok(Sexp::List(children))
}

fn append_footprint_metadata(value: &AuthoredFootprint, children: &mut Vec<Sexp>) {
    if let Some(description) = &value.description {
        children.push(form("descr", [quoted(description)]));
    }
    if let Some(tags) = &value.tags {
        children.push(form("tags", [quoted(tags)]));
    }
    if !value.attributes.is_empty() {
        children.push(form("attr", value.attributes.iter().map(|item| atom(item))));
    }
    for (name, amount) in [
        ("solder_mask_margin", value.solder_mask_margin_mm),
        ("solder_paste_margin", value.solder_paste_margin_mm),
        ("solder_paste_margin_ratio", value.solder_paste_margin_ratio),
        ("clearance", value.clearance_mm),
    ] {
        if let Some(amount) = amount {
            children.push(form(name, [float(amount)]));
        }
    }
    if let Some(connection) = value.zone_connect {
        children.push(form("zone_connect", [integer(connection.source_code())]));
    }
}

fn append_occurrence(value: &AuthoredFootprintOccurrence, children: &mut Vec<Sexp>) {
    if value.locked {
        children.push(form("locked", [atom("yes")]));
    }
    children.push(form("layer", [quoted(&value.layer)]));
    children.push(position_form("at", value.at, value.angle_degrees));
    children.push(form("uuid", [quoted(&value.uuid)]));
    for (name, value) in [
        ("path", &value.placement_path),
        ("sheetname", &value.placement_sheet_name),
        ("sheetfile", &value.placement_sheet_file),
    ] {
        if let Some(value) = value {
            children.push(form(name, [quoted(value)]));
        }
    }
}

fn footprint_property(
    value: &AuthoredFootprintProperty,
    occurrence: Option<&AuthoredFootprintOccurrence>,
    limits: PcbAuthoringLimits,
) -> Result<Sexp, Error> {
    let mut children = vec![
        atom("property"),
        quoted(&value.name),
        quoted(&value.value),
        position_form("at", value.at, value.angle_degrees),
        form("layer", [quoted(&value.layer)]),
    ];
    if value.hidden {
        children.push(form("hide", [atom("yes")]));
    }
    if value.unlocked {
        children.push(form("unlocked", [atom("yes")]));
    }
    children.push(form("uuid", [quoted(&value.uuid)]));
    children.push(effects(&value.effects));
    if let Some(cache) = &value.render_cache {
        children.push(render_cache(
            cache,
            occurrence.map(|occurrence| CacheTransform::FootprintAnchor {
                occurrence,
                local_anchor: value.at,
            }),
            limits,
        )?);
    }
    Ok(Sexp::List(children))
}

fn footprint_text_value(
    value: &AuthoredFootprintText,
    occurrence: Option<&AuthoredFootprintOccurrence>,
    limits: PcbAuthoringLimits,
) -> Result<Sexp, Error> {
    let mut children = vec![
        atom("fp_text"),
        atom("user"),
        quoted(&value.text),
        position_form("at", value.at, value.angle_degrees),
        text_layer(&value.layer, value.knockout),
    ];
    if value.hidden {
        children.push(atom("hide"));
    }
    if value.unlocked {
        children.push(form("unlocked", [atom("yes")]));
    }
    children.push(form("uuid", [quoted(&value.uuid)]));
    children.push(effects(&value.effects));
    if let Some(cache) = &value.render_cache {
        children.push(render_cache(
            cache,
            occurrence.map(|occurrence| CacheTransform::FootprintAnchor {
                occurrence,
                local_anchor: value.at,
            }),
            limits,
        )?);
    }
    Ok(Sexp::List(children))
}

fn board_text_value(value: &AuthoredBoardText, limits: PcbAuthoringLimits) -> Result<Sexp, Error> {
    let mut children = vec![
        atom("gr_text"),
        quoted(&value.text),
        position_form("at", value.at, value.angle_degrees),
        text_layer(&value.layer, value.knockout),
    ];
    if value.locked {
        children.push(form("locked", [atom("yes")]));
    }
    children.push(form("uuid", [quoted(&value.uuid)]));
    children.push(effects(&value.effects));
    if let Some(cache) = &value.render_cache {
        children.push(render_cache(cache, None, limits)?);
    }
    Ok(Sexp::List(children))
}

fn text_box(
    value: &AuthoredTextBox,
    prefix: &str,
    occurrence: Option<&AuthoredFootprintOccurrence>,
    limits: PcbAuthoringLimits,
) -> Result<Sexp, Error> {
    let mut children = vec![atom(&format!("{prefix}text_box")), quoted(&value.text)];
    if value.locked {
        children.push(form("locked", [atom("yes")]));
    }
    match &value.geometry {
        AuthoredTextBoxGeometry::Rectangle { start, end } => {
            children.extend([point_form("start", *start), point_form("end", *end)]);
        }
        AuthoredTextBoxGeometry::Polygon { points } => children.push(points_form(points)),
    }
    children.extend([
        form("margins", value.margins_mm.into_iter().map(float)),
        form("angle", [float(value.angle_degrees)]),
        form("layer", [quoted(&value.layer)]),
        form("uuid", [quoted(&value.uuid)]),
        effects(&value.effects),
        form("border", [atom(if value.border { "yes" } else { "no" })]),
        form(
            "stroke",
            [
                form("width", [float(value.stroke_width_mm)]),
                form("type", [atom(&value.stroke_kind)]),
            ],
        ),
        form(
            "knockout",
            [atom(if value.knockout { "yes" } else { "no" })],
        ),
    ]);
    if let Some(cache) = &value.render_cache {
        children.push(render_cache(
            cache,
            occurrence.map(CacheTransform::FootprintTextBox),
            limits,
        )?);
    }
    Ok(Sexp::List(children))
}

fn text_layer(layer: &str, knockout: bool) -> Sexp {
    let mut values = vec![quoted(layer)];
    if knockout {
        values.push(atom("knockout"));
    }
    form("layer", values)
}

fn render_cache(
    value: &TextRenderCache,
    transform: Option<CacheTransform<'_>>,
    limits: PcbAuthoringLimits,
) -> Result<Sexp, Error> {
    let cache_limits = TextRenderCacheLimits {
        max_text_bytes: limits.max_string_bytes,
        max_polygons: limits.max_objects,
        max_contours: limits.max_objects,
        max_points: limits.max_points,
        max_output_bytes: limits.max_output_bytes,
        ..TextRenderCacheLimits::default()
    };
    let transformed;
    let value = if let Some(transform) = transform {
        transformed = occurrence_render_cache(value, transform);
        &transformed
    } else {
        value
    };
    let source = write_text_render_cache_a0(value, cache_limits).map_err(|error| {
        let kind = if error.kind == TextRenderCacheErrorKind::ResourceLimit {
            ErrorKind::ResourceLimit
        } else {
            ErrorKind::InvalidBuildValue
        };
        Error::build(kind, format!("invalid authored text render cache: {error}"))
    })?;
    parse_bytes(&source)
}

fn occurrence_render_cache(
    value: &TextRenderCache,
    transform: CacheTransform<'_>,
) -> TextRenderCache {
    let mut result = value.clone();
    let (occurrence, local_anchor) = match transform {
        CacheTransform::FootprintAnchor {
            occurrence,
            local_anchor,
        } => (occurrence, Some(local_anchor)),
        CacheTransform::FootprintTextBox(occurrence) => {
            result.angle_degrees =
                (result.angle_degrees + occurrence.angle_degrees).rem_euclid(360.0);
            (occurrence, None)
        }
    };
    let board_anchor = local_anchor.map(|point| occurrence_point(point, Some(occurrence)));
    for point in result
        .polygons
        .iter_mut()
        .flat_map(|polygon| &mut polygon.contours)
        .flat_map(|contour| &mut contour.points)
    {
        let local = AuthoredPoint {
            x_mm: point.x,
            y_mm: point.y,
        };
        let transformed =
            if let (Some(local_anchor), Some(board_anchor)) = (local_anchor, board_anchor) {
                AuthoredPoint {
                    x_mm: local.x_mm + board_anchor.x_mm - local_anchor.x_mm,
                    y_mm: local.y_mm + board_anchor.y_mm - local_anchor.y_mm,
                }
            } else {
                occurrence_point(local, Some(occurrence))
            };
        point.x = transformed.x_mm;
        point.y = transformed.y_mm;
    }
    result
}

#[derive(Clone, Copy)]
enum CacheTransform<'a> {
    FootprintAnchor {
        occurrence: &'a AuthoredFootprintOccurrence,
        local_anchor: AuthoredPoint,
    },
    FootprintTextBox(&'a AuthoredFootprintOccurrence),
}

fn occurrence_point(
    point: AuthoredPoint,
    occurrence: Option<&AuthoredFootprintOccurrence>,
) -> AuthoredPoint {
    let Some(occurrence) = occurrence else {
        return point;
    };
    let (sin, cos) = occurrence.angle_degrees.to_radians().sin_cos();
    AuthoredPoint {
        x_mm: occurrence.at.x_mm + point.x_mm * cos + point.y_mm * sin,
        y_mm: occurrence.at.y_mm - point.x_mm * sin + point.y_mm * cos,
    }
}

fn graphic(value: &AuthoredGraphic, prefix: &str) -> Sexp {
    let (suffix, mut children) = graphic_geometry(&value.geometry);
    children.insert(0, atom(&format!("{prefix}{suffix}")));
    children.push(form(
        "stroke",
        [
            form("width", [float(value.stroke_width_mm)]),
            form("type", [atom(&value.stroke_kind)]),
        ],
    ));
    if let Some(fill) = &value.fill {
        children.push(form("fill", [atom(fill)]));
    }
    children.push(form("layer", [quoted(&value.layer)]));
    children.push(form("uuid", [quoted(&value.uuid)]));
    Sexp::List(children)
}

fn graphic_geometry(value: &AuthoredGraphicGeometry) -> (&'static str, Vec<Sexp>) {
    match value {
        AuthoredGraphicGeometry::Line { start, end } => (
            "line",
            vec![point_form("start", *start), point_form("end", *end)],
        ),
        AuthoredGraphicGeometry::Arc { start, mid, end } => (
            "arc",
            vec![
                point_form("start", *start),
                point_form("mid", *mid),
                point_form("end", *end),
            ],
        ),
        AuthoredGraphicGeometry::Rect { start, end } => (
            "rect",
            vec![point_form("start", *start), point_form("end", *end)],
        ),
        AuthoredGraphicGeometry::Circle { center, end } => (
            "circle",
            vec![point_form("center", *center), point_form("end", *end)],
        ),
        AuthoredGraphicGeometry::Polygon { points } => ("poly", vec![points_form(points)]),
    }
}

fn pad(value: &AuthoredPad) -> Sexp {
    let mut children = vec![
        atom("pad"),
        quoted(&value.number),
        atom(value.kind.as_str()),
        atom(value.shape.as_str()),
        position_form("at", value.at, value.angle_degrees),
        form("size", [float(value.size_x_mm), float(value.size_y_mm)]),
    ];
    for (head, text) in [
        ("pinfunction", &value.pin_function),
        ("pintype", &value.pin_type),
    ] {
        if let Some(text) = text {
            children.push(form(head, [quoted(text)]));
        }
    }
    if let Some(length) = value.die_length_mm {
        children.push(form("die_length", [float(length)]));
    }
    if let Some(drill) = &value.drill {
        children.push(drill_form(drill));
    }
    children.push(form("layers", value.layers.iter().map(|item| quoted(item))));
    if let Some(net) = &value.net {
        children.push(form("net", [integer(net.code), quoted(&net.name)]));
    }
    children.push(form("uuid", [quoted(&value.uuid)]));
    append_shape(&value.shape, &mut children);
    append_pad_policies(value, &mut children);
    if let Some(stack) = &value.padstack {
        children.push(padstacks::padstack(stack));
    }
    Sexp::List(children)
}

fn drill_form(value: &AuthoredDrill) -> Sexp {
    let mut values = Vec::new();
    if let Some(height) = value.height_mm {
        values.extend([atom("oval"), float(value.width_mm), float(height)]);
    } else {
        values.push(float(value.width_mm));
    }
    values.push(point_form("offset", value.offset));
    form("drill", values)
}

fn append_shape(shape: &AuthoredPadShape, children: &mut Vec<Sexp>) {
    match shape {
        AuthoredPadShape::Trapezoid {
            delta_x_mm,
            delta_y_mm,
        } => {
            children.push(form("rect_delta", [float(*delta_x_mm), float(*delta_y_mm)]));
        }
        AuthoredPadShape::RoundRect { radius_ratio } => {
            children.push(form("roundrect_rratio", [float(*radius_ratio)]));
        }
        AuthoredPadShape::ChamferedRoundRect {
            radius_ratio,
            chamfer_ratio,
            corners,
        } => {
            children.push(form("roundrect_rratio", [float(*radius_ratio)]));
            children.push(form("chamfer_ratio", [float(*chamfer_ratio)]));
            children.push(form(
                "chamfer",
                corners.iter().map(|corner| atom(corner.as_str())),
            ));
        }
        AuthoredPadShape::CustomPolygon { points } => {
            custom_pads::append_custom(
                Some(AuthoredPadAnchor::Rect),
                Some(AuthoredCustomPadClearance::Outline),
                [custom_pads::polygon(
                    points.iter().map(|point| point_form("xy", *point)),
                    0.0,
                    Some(AuthoredPadPrimitiveFill::Solid),
                )],
                children,
            );
        }
        AuthoredPadShape::Custom {
            anchor,
            clearance,
            primitives,
        } => custom_pads::append_custom(
            *anchor,
            *clearance,
            primitives.iter().map(custom_pads::primitive),
            children,
        ),
        _ => {}
    }
}

fn append_pad_policies(value: &AuthoredPad, children: &mut Vec<Sexp>) {
    for (head, number) in [
        ("solder_mask_margin", value.solder_mask_margin_mm),
        ("solder_paste_margin", value.solder_paste_margin_mm),
        ("solder_paste_margin_ratio", value.solder_paste_margin_ratio),
        ("clearance", value.clearance_mm),
        ("thermal_bridge_width", value.thermal_bridge_width_mm),
        ("thermal_bridge_angle", value.thermal_bridge_angle_degrees),
        ("thermal_gap", value.thermal_gap_mm),
    ] {
        if let Some(number) = number {
            children.push(form(head, [float(number)]));
        }
    }
    if let Some(zone_connect) = value.zone_connect {
        children.push(form("zone_connect", [integer(zone_connect.source_code())]));
    }
    if !value.zone_layer_connections.is_empty() {
        children.push(form(
            "zone_layer_connections",
            value.zone_layer_connections.iter().map(|item| quoted(item)),
        ));
    }
    for (head, enabled) in [
        ("remove_unused_layers", value.remove_unused_layers),
        ("keep_end_layers", value.keep_end_layers),
    ] {
        if let Some(enabled) = enabled {
            children.push(form(head, [atom(if enabled { "yes" } else { "no" })]));
        }
    }
}

fn via(value: &AuthoredVia) -> Sexp {
    let mut children = vec![atom("via")];
    if let Some(kind) = value.kind.source_token() {
        children.push(atom(kind));
    }
    children.extend([
        point_form("at", value.at),
        form("size", [float(value.size_mm)]),
        form("drill", [float(value.drill_mm)]),
        form(
            "layers",
            [quoted(&value.start_layer), quoted(&value.end_layer)],
        ),
    ]);
    if value.free {
        children.push(form("free", [atom("yes")]));
    }
    if let Some(stack) = &value.padstack {
        children.push(padstacks::viastack(stack));
    }
    if let Some(drill) = &value.backdrill {
        children.push(form(
            "backdrill",
            [
                form("size", [float(drill.size_mm)]),
                form(
                    "layers",
                    [quoted(&drill.start_layer), quoted(&drill.end_layer)],
                ),
            ],
        ));
    }
    append_via_policies(value, &mut children);
    children.extend([
        form("net", [integer(value.net_code)]),
        form("uuid", [quoted(&value.uuid)]),
    ]);
    Sexp::List(children)
}

fn append_via_policies(value: &AuthoredVia, children: &mut Vec<Sexp>) {
    for (head, policy) in [
        ("tenting", &value.tenting),
        ("covering", &value.covering),
        ("plugging", &value.plugging),
    ] {
        if let Some(policy) = policy {
            children.push(front_back_policy(head, policy));
        }
    }
    for (head, enabled) in [("capping", value.capping), ("filling", value.filling)] {
        if let Some(enabled) = enabled {
            children.push(form(head, [yes_no(enabled)]));
        }
    }
    if !value.zone_layer_connections.is_empty() {
        children.push(form(
            "zone_layer_connections",
            value
                .zone_layer_connections
                .iter()
                .map(|layer| quoted(layer)),
        ));
    }
    for (head, enabled) in [
        ("remove_unused_layers", value.remove_unused_layers),
        ("keep_end_layers", value.keep_end_layers),
        ("start_end_only", value.start_end_only),
    ] {
        if let Some(enabled) = enabled {
            children.push(form(head, [yes_no(enabled)]));
        }
    }
}

fn front_back_policy(head: &str, value: &AuthoredFrontBackPolicy) -> Sexp {
    let mut children = Vec::with_capacity(2);
    if let Some(front) = value.front {
        children.push(form("front", [yes_no(front)]));
    }
    if let Some(back) = value.back {
        children.push(form("back", [yes_no(back)]));
    }
    form(head, children)
}

fn yes_no(value: bool) -> Sexp {
    atom(if value { "yes" } else { "no" })
}

fn model(value: &AuthoredModel) -> Sexp {
    form(
        "model",
        [
            quoted(&value.path),
            xyz_parent("offset", value.offset_mm),
            xyz_parent("scale", value.scale),
            xyz_parent("rotate", value.rotate_degrees),
        ],
    )
}

fn segment(value: &AuthoredSegment) -> Sexp {
    form(
        "segment",
        [
            point_form("start", value.start),
            point_form("end", value.end),
            form("width", [float(value.width_mm)]),
            form("layer", [quoted(&value.layer)]),
            form("net", [integer(value.net_code)]),
            form("uuid", [quoted(&value.uuid)]),
        ],
    )
}

fn routing_arc(value: &AuthoredRoutingArc) -> Sexp {
    form(
        "arc",
        [
            point_form("start", value.start),
            point_form("mid", value.mid),
            point_form("end", value.end),
            form("width", [float(value.width_mm)]),
            form("layer", [quoted(&value.layer)]),
            form("net", [integer(value.net_code)]),
            form("uuid", [quoted(&value.uuid)]),
        ],
    )
}

fn zone(value: &AuthoredZone) -> Sexp {
    let mut children = vec![
        atom("zone"),
        form("net", [integer(value.net.code)]),
        form("net_name", [quoted(&value.net.name)]),
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
        children.push(form("locked", [atom("yes")]));
    }
    children.push(form("uuid", [quoted(&value.uuid)]));
    if let Some(name) = &value.name {
        children.push(form("name", [quoted(name)]));
    }
    children.push(form(
        "hatch",
        [atom(value.hatch.as_str()), float(value.hatch_pitch_mm)],
    ));
    if value.priority != 0 {
        children.push(form("priority", [integer(value.priority)]));
    }
    let mut connect_pads = Vec::with_capacity(2);
    if let Some(mode) = value.pad_connection.source_token() {
        connect_pads.push(atom(mode));
    }
    connect_pads.push(form("clearance", [float(value.connect_pads_clearance_mm)]));
    children.push(form("connect_pads", connect_pads));
    children.push(form("min_thickness", [float(value.min_thickness_mm)]));
    if let Some(enabled) = value.filled_areas_thickness {
        children.push(form("filled_areas_thickness", [yes_no(enabled)]));
    }
    if let Some(fill) = &value.fill {
        children.push(zone_fill(fill));
    }
    children.extend(value.outlines.iter().map(zone_polygon));
    children.extend(value.filled_polygons.iter().map(zone_filled_polygon));
    Sexp::List(children)
}

fn zone_fill(value: &AuthoredZoneFill) -> Sexp {
    let mut children = vec![
        atom("yes"),
        form("thermal_gap", [float(value.thermal_gap_mm)]),
        form(
            "thermal_bridge_width",
            [float(value.thermal_bridge_width_mm)],
        ),
    ];
    if let Some(mode) = value.island_removal_mode {
        children.push(form("island_removal_mode", [integer(mode)]));
    }
    if let Some(area) = value.island_area_min_mm2 {
        children.push(form("island_area_min", [float(area)]));
    }
    form("fill", children)
}

fn zone_polygon(value: &AuthoredZonePolygon) -> Sexp {
    form("polygon", [points_form(&value.points)])
}

fn zone_filled_polygon(value: &AuthoredZoneFilledPolygon) -> Sexp {
    let mut children = vec![form("layer", [quoted(&value.layer)])];
    if value.island {
        children.push(Sexp::List(vec![atom("island")]));
    }
    children.push(points_form(&value.points));
    form("filled_polygon", children)
}

fn points_form(points: &[AuthoredPoint]) -> Sexp {
    form("pts", points.iter().map(|point| point_form("xy", *point)))
}

fn point_form(head: &str, point: AuthoredPoint) -> Sexp {
    form(head, [float(point.x_mm), float(point.y_mm)])
}

fn position_form(head: &str, point: AuthoredPoint, angle: f64) -> Sexp {
    form(head, [float(point.x_mm), float(point.y_mm), float(angle)])
}

fn xyz_parent(head: &str, values: [f64; 3]) -> Sexp {
    form(
        head,
        [form(
            "xyz",
            [float(values[0]), float(values[1]), float(values[2])],
        )],
    )
}

fn form(head: &str, children: impl IntoIterator<Item = Sexp>) -> Sexp {
    let mut values = vec![atom(head)];
    values.extend(children);
    Sexp::List(values)
}

fn atom(value: &str) -> Sexp {
    Sexp::Atom(value.to_owned())
}

fn quoted(value: &str) -> Sexp {
    Sexp::Quoted(value.to_owned())
}

const fn integer(value: i64) -> Sexp {
    Sexp::Integer(value)
}

const fn float(value: f64) -> Sexp {
    Sexp::Float(value)
}
