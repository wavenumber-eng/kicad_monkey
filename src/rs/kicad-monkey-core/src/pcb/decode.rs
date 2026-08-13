use super::*;

pub(super) fn layer_from_span(source: &str, span: &FormSpan) -> Result<PcbLayer, Error> {
    let values = scalar_values(source, span)?;
    let ordinal = span
        .head
        .as_deref()
        .ok_or_else(|| source_error("Expected layer ordinal", span.start))?
        .parse()
        .map_err(|_| source_error("Expected layer ordinal", span.start))?;
    Ok(PcbLayer {
        ordinal,
        name: required_string(values.first(), "Expected layer name", span)?,
        kind: required_string(values.get(1), "Expected layer kind", span)?,
        user_name: values.get(2).map(token_string),
        source_range: span.range.clone(),
    })
}

pub(super) fn net_from_span(source: &str, span: &FormSpan) -> Result<PcbNet, Error> {
    let values = scalar_values(source, span)?;
    Ok(PcbNet {
        code: required_i64(values.first(), "Expected net code", span)?,
        name: required_string(values.get(1), "Expected net name", span)?,
        source_range: span.range.clone(),
    })
}

pub(super) fn property_from_span(source: &str, span: &FormSpan) -> Result<PcbProperty, Error> {
    let values = scalar_values(source, span)?;
    let name = required_string(values.first(), "Expected property name", span)?;
    let token = values
        .get(1)
        .ok_or_else(|| source_error("Expected property value", span.end))?;
    Ok(PcbProperty {
        name,
        value: token_string(token),
        source_range: span.range.clone(),
        value_range: (span.range.start + token.position.offset)
            ..(span.range.start + token.position.offset + token.lexeme.len()),
    })
}

pub(super) fn footprint_from_span(
    source: &str,
    indexed: &IndexedFootprint,
    limits: PcbLimits,
) -> Result<PcbFootprint, Error> {
    let header = scalar_values(source, &indexed.span)?;
    let library_link = required_string(
        header.first(),
        "Expected footprint library link",
        &indexed.span,
    )?;
    let children = direct_children(source, &indexed.span, limits.max_footprint_children, limits)?;
    let locked = has_flag(&header, "locked") || child_bool(source, &children, "locked")?;
    let embedded_fonts = child_bool(source, &children, "embedded_fonts")?;
    let duplicate_pad_numbers_are_jumpers = child(&children, "duplicate_pad_numbers_are_jumpers")
        .map(|_| child_bool(source, &children, "duplicate_pad_numbers_are_jumpers"))
        .transpose()?;
    let mut result = empty_footprint(
        indexed,
        library_link,
        locked,
        embedded_fonts,
        duplicate_pad_numbers_are_jumpers,
    );
    for child in &children {
        if apply_footprint_property(source, child, &mut result)? {
            continue;
        }
        if apply_footprint_placement(source, child, &mut result)? {
            continue;
        }
        if apply_footprint_metadata(source, child, limits, &mut result)? {
            continue;
        }
        apply_footprint_fabrication(source, child, &mut result)?;
    }
    Ok(result)
}

fn empty_footprint(
    indexed: &IndexedFootprint,
    library_link: String,
    locked: bool,
    embedded_fonts: bool,
    duplicate_pad_numbers_are_jumpers: Option<bool>,
) -> PcbFootprint {
    PcbFootprint {
        library_link,
        reference: None,
        value: None,
        layer: None,
        at_x: None,
        at_y: None,
        angle: None,
        locked,
        placement_path: None,
        placement_sheet_name: None,
        placement_sheet_file: None,
        uuid: None,
        description: String::new(),
        tags: String::new(),
        attributes: Vec::new(),
        embedded_fonts,
        duplicate_pad_numbers_are_jumpers,
        solder_mask_margin: None,
        solder_paste_margin: None,
        solder_paste_margin_ratio: None,
        clearance: None,
        zone_connect: None,
        property_count: indexed.property_count,
        graphic_count: indexed.graphic_count,
        text_count: indexed.text_count,
        text_box_count: indexed.text_box_count,
        pad_count: indexed.pad_count,
        model_count: indexed.model_count,
        source_range: indexed.span.range.clone(),
    }
}

fn apply_footprint_property(
    source: &str,
    child: &FormSpan,
    result: &mut PcbFootprint,
) -> Result<bool, Error> {
    if child.head.as_deref() != Some("property") {
        return Ok(false);
    }
    let property = property_from_span(source, child)?;
    if property.name == "Reference" && result.reference.is_none() {
        result.reference = Some(property.value);
    } else if property.name == "Value" && result.value.is_none() {
        result.value = Some(property.value);
    }
    Ok(true)
}

fn apply_footprint_placement(
    source: &str,
    child: &FormSpan,
    result: &mut PcbFootprint,
) -> Result<bool, Error> {
    match child.head.as_deref() {
        Some("layer") => result.layer = first_string(source, child)?,
        Some("at") => {
            let values = scalar_values(source, child)?;
            result.at_x = optional_f64(values.first(), child)?;
            result.at_y = optional_f64(values.get(1), child)?;
            result.angle = optional_f64(values.get(2), child)?;
        }
        Some("path") => result.placement_path = first_string(source, child)?,
        Some("sheetname") => result.placement_sheet_name = first_string(source, child)?,
        Some("sheetfile") => result.placement_sheet_file = first_string(source, child)?,
        Some("uuid" | "tstamp") if result.uuid.is_none() => {
            result.uuid = first_string(source, child)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_footprint_metadata(
    source: &str,
    child: &FormSpan,
    limits: PcbLimits,
    result: &mut PcbFootprint,
) -> Result<bool, Error> {
    match child.head.as_deref() {
        Some("descr") => result.description = first_string(source, child)?.unwrap_or_default(),
        Some("tags") => result.tags = first_string(source, child)?.unwrap_or_default(),
        Some("attr") => {
            result.attributes =
                bounded_scalar_values(source, child, limits.max_footprint_attributes)?
                    .iter()
                    .map(token_string)
                    .collect();
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn apply_footprint_fabrication(
    source: &str,
    child: &FormSpan,
    result: &mut PcbFootprint,
) -> Result<(), Error> {
    match child.head.as_deref() {
        Some("solder_mask_margin") => result.solder_mask_margin = first_f64(source, child)?,
        Some("solder_paste_margin") => result.solder_paste_margin = first_f64(source, child)?,
        Some("solder_paste_margin_ratio") => {
            result.solder_paste_margin_ratio = first_f64(source, child)?;
        }
        Some("clearance") => result.clearance = first_f64(source, child)?,
        Some("zone_connect") => result.zone_connect = first_i64(source, child)?,
        _ => {}
    }
    Ok(())
}

pub(super) fn model_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbModelReference, Error> {
    let header = scalar_values(source, &indexed.span)?;
    let children = direct_children(source, &indexed.span, limits.max_model_children, limits)?;
    Ok(PcbModelReference {
        footprint_index: indexed.parent_index,
        path: required_string(header.first(), "Expected model path", &indexed.span)?,
        offset: nested_xyz(source, &children, "offset", [0.0, 0.0, 0.0], limits)?,
        scale: nested_xyz(source, &children, "scale", [1.0, 1.0, 1.0], limits)?,
        rotate: nested_xyz(source, &children, "rotate", [0.0, 0.0, 0.0], limits)?,
        source_range: indexed.span.range.clone(),
    })
}

pub(super) fn net_resolver_from_spans(
    source: &str,
    spans: &[FormSpan],
) -> Result<NetResolver, Error> {
    let mut resolver = NetResolver::default();
    for span in spans {
        let net = net_from_span(source, span)?;
        resolver.name_by_ordinal.insert(net.code, net.name.clone());
        if !net.name.is_empty() {
            resolver.ordinal_by_name.insert(net.name, net.code);
        }
    }
    Ok(resolver)
}

pub(super) fn segment_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbSegment, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let start = required_xy(source, &children, "start", span)?;
    let end = required_xy(source, &children, "end", span)?;
    Ok(PcbSegment {
        start_x: start.0,
        start_y: start.1,
        end_x: end.0,
        end_y: end.1,
        width: optional_child_f64(source, &children, "width")?,
        layer: optional_child_string(source, &children, "layer")?,
        net: child_net_ref_or_zero(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

pub(super) fn graphic_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbGraphic, Error> {
    let kind = span
        .head
        .as_deref()
        .and_then(graphic_kind)
        .ok_or_else(|| source_error("Expected board graphic form", span.start))?;
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let points = child(&children, "pts")
        .map(|points| points_from_span(source, points, limits))
        .transpose()?
        .unwrap_or_default();
    let (stroke_width, stroke_kind) = if let Some(stroke) = child(&children, "stroke") {
        let fields = direct_children(source, stroke, 16, limits)?;
        (
            optional_child_f64(source, &fields, "width")?,
            optional_child_string(source, &fields, "type")?,
        )
    } else {
        (None, None)
    };
    Ok(PcbGraphic {
        kind,
        text: matches!(kind, PcbGraphicKind::Text | PcbGraphicKind::TextBox)
            .then(|| header.first().map(token_string))
            .flatten(),
        at: optional_child_point(source, &children, "at")?,
        start: optional_child_point(source, &children, "start")?,
        mid: optional_child_point(source, &children, "mid")?,
        end: optional_child_point(source, &children, "end")?,
        center: optional_child_point(source, &children, "center")?,
        points,
        layer: optional_child_string(source, &children, "layer")?,
        stroke_width,
        stroke_kind,
        fill: optional_child_string(source, &children, "fill")?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

pub(super) fn routing_arc_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbRoutingArc, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    Ok(PcbRoutingArc {
        start: required_point(source, &children, "start", span)?,
        mid: required_point(source, &children, "mid", span)?,
        end: required_point(source, &children, "end", span)?,
        width: optional_child_f64(source, &children, "width")?,
        layer: optional_child_string(source, &children, "layer")?,
        net: child_net_ref_or_zero(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

pub(super) fn dimension_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbDimension, Error> {
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let points = child(&children, "pts")
        .map(|points| points_from_span(source, points, limits))
        .transpose()?
        .unwrap_or_default();
    Ok(PcbDimension {
        kind: optional_child_string(source, &children, "type")?
            .unwrap_or_else(|| "aligned".to_owned()),
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "Cmts.User".to_owned()),
        points,
        height: optional_child_f64(source, &children, "height")?.unwrap_or(0.0),
        leader_length: optional_child_f64(source, &children, "leader_length")?,
        orientation: optional_child_i64(source, &children, "orientation")?,
        locked: has_flag(&header, "locked") || child_bool(source, &children, "locked")?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

pub(super) fn group_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbGroup, Error> {
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    Ok(PcbGroup {
        name: required_string(header.first(), "Expected group name", span)?,
        uuid: optional_uuid_or_id(source, &children)?,
        locked: has_flag(&header, "locked") || child_bool(source, &children, "locked")?,
        members: child_strings(source, &children, "members", limits.max_members)?,
        source_range: span.range.clone(),
    })
}

pub(super) fn generated_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbGeneratedItem, Error> {
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_generated_children, limits)?;
    let known = ["id", "type", "name", "layer", "locked", "members"];
    let property_heads = children
        .iter()
        .filter_map(|child| child.head.as_ref())
        .filter(|head| !known.contains(&head.as_str()))
        .cloned()
        .collect();
    Ok(PcbGeneratedItem {
        kind: optional_child_string(source, &children, "type")?,
        name: optional_child_string(source, &children, "name")?,
        layer: optional_child_string(source, &children, "layer")?,
        uuid: optional_child_string(source, &children, "id")?,
        locked: has_flag(&header, "locked") || child_bool(source, &children, "locked")?,
        members: child_strings(source, &children, "members", limits.max_members)?,
        property_heads,
        source_range: span.range.clone(),
    })
}

pub(super) fn embedded_file_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbEmbeddedFile, Error> {
    let children = direct_children(source, span, 16, limits)?;
    let encoded_data_bytes = child(&children, "data")
        .map(|data| {
            scalar_values(source, data).map(|tokens| {
                tokens
                    .iter()
                    .map(token_string)
                    .map(|part| part.trim_matches('|').len())
                    .sum()
            })
        })
        .transpose()?
        .unwrap_or(0);
    Ok(PcbEmbeddedFile {
        name: optional_child_string(source, &children, "name")?.unwrap_or_default(),
        file_type: optional_child_string(source, &children, "type")?
            .unwrap_or_else(|| "other".to_owned()),
        checksum: optional_child_string(source, &children, "checksum")?,
        encoded_data_bytes,
        source_range: span.range.clone(),
    })
}
