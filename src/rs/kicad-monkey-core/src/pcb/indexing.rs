use super::*;

pub(super) fn top_level_identifier(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<Option<String>, Error> {
    let maximum = match span.head.as_deref() {
        Some("footprint" | "module") => limits.max_footprint_children,
        Some("generated") => limits.max_generated_children,
        _ => limits.max_object_children,
    };
    let children = direct_children(source, span, maximum, limits)?;
    optional_uuid_or_id(source, &children)
}

pub(super) fn index_top_level(
    source: &str,
    top_level: &[FormSpan],
    limits: PcbLimits,
    selection: PcbSelection,
) -> Result<PcbIndex, Error> {
    let mut index = PcbIndex::default();
    for span in top_level {
        let Some(head) = span.head.as_deref() else {
            index.counts.unknown_top_level += 1;
            continue;
        };
        if top_level_family(head).is_some_and(|family| !selection.contains(family)) {
            continue;
        }
        if index_primary_family(source, span, head, limits, selection, &mut index)?
            || index_object_family(span, head, limits, &mut index)?
            || index_container_family(source, span, head, limits, &mut index)?
        {
            continue;
        }
        if !is_known_top_level(head) {
            index.counts.unknown_top_level += 1;
        }
    }
    Ok(index)
}

pub(super) fn top_level_family(head: &str) -> Option<PcbFamily> {
    primary_family(head)
        .or_else(|| object_family(head))
        .or_else(|| container_family(head))
}

fn primary_family(head: &str) -> Option<PcbFamily> {
    match head {
        "layers" => Some(PcbFamily::Layers),
        "net" => Some(PcbFamily::Nets),
        "property" => Some(PcbFamily::Properties),
        "footprint" | "module" => Some(PcbFamily::Footprints),
        "segment" => Some(PcbFamily::Segments),
        "via" => Some(PcbFamily::Vias),
        "zone" => Some(PcbFamily::Zones),
        "arc" => Some(PcbFamily::Arcs),
        _ => None,
    }
}

fn object_family(head: &str) -> Option<PcbFamily> {
    match head {
        head if graphic_kind(head).is_some() => Some(PcbFamily::Graphics),
        "group" => Some(PcbFamily::Groups),
        "dimension" => Some(PcbFamily::Dimensions),
        "generated" => Some(PcbFamily::GeneratedItems),
        _ => None,
    }
}

fn container_family(head: &str) -> Option<PcbFamily> {
    match head {
        "embedded_files" => Some(PcbFamily::EmbeddedFiles),
        "variants" => Some(PcbFamily::Variants),
        "image" => Some(PcbFamily::Images),
        "barcode" => Some(PcbFamily::Barcodes),
        "table" => Some(PcbFamily::Tables),
        _ => None,
    }
}

fn index_primary_family(
    source: &str,
    span: &FormSpan,
    head: &str,
    limits: PcbLimits,
    selection: PcbSelection,
    index: &mut PcbIndex,
) -> Result<bool, Error> {
    match head {
        "layers" => index_layers(source, span, limits, index)?,
        "net" => push_counted(
            &mut index.nets,
            span,
            limits.max_nets,
            &mut index.counts.nets,
        )?,
        "property" => push_counted(
            &mut index.properties,
            span,
            limits.max_properties,
            &mut index.counts.properties,
        )?,
        "footprint" | "module" => index_footprint(source, span, limits, selection, index)?,
        "segment" => push_counted(
            &mut index.segments,
            span,
            limits.max_segments,
            &mut index.counts.segments,
        )?,
        "via" => push_counted(
            &mut index.vias,
            span,
            limits.max_vias,
            &mut index.counts.vias,
        )?,
        "zone" => push_counted(
            &mut index.zones,
            span,
            limits.max_zones,
            &mut index.counts.zones,
        )?,
        "arc" => push_counted(
            &mut index.arcs,
            span,
            limits.max_arcs,
            &mut index.counts.arcs,
        )?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn index_object_family(
    span: &FormSpan,
    head: &str,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<bool, Error> {
    match head {
        head if graphic_kind(head).is_some() => {
            bounded_push(&mut index.graphics, span.clone(), limits.max_graphics)?;
            increment_graphic_count(&mut index.counts, head);
        }
        "group" => push_counted(
            &mut index.groups,
            span,
            limits.max_groups,
            &mut index.counts.groups,
        )?,
        "dimension" => push_counted(
            &mut index.dimensions,
            span,
            limits.max_dimensions,
            &mut index.counts.dimensions,
        )?,
        "generated" => push_counted(
            &mut index.generated_items,
            span,
            limits.max_generated_items,
            &mut index.counts.generated_items,
        )?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn index_container_family(
    source: &str,
    span: &FormSpan,
    head: &str,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<bool, Error> {
    match head {
        "embedded_files" => index_embedded_files(source, span, limits, index)?,
        "variants" => extended::index_variants(source, span, limits, index)?,
        "image" => push_counted(
            &mut index.images,
            span,
            limits.max_images,
            &mut index.counts.images,
        )?,
        "barcode" => push_counted(
            &mut index.barcodes,
            span,
            limits.max_barcodes,
            &mut index.counts.barcodes,
        )?,
        "table" => extended::index_table(source, span, limits, index)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn push_counted<T: Clone>(
    values: &mut Vec<T>,
    value: &T,
    maximum: usize,
    count: &mut usize,
) -> Result<(), Error> {
    bounded_push(values, value.clone(), maximum)?;
    *count = count.checked_add(1).ok_or_else(limit_error)?;
    Ok(())
}

pub(super) fn increment_graphic_count(counts: &mut PcbCounts, head: &str) {
    counts.graphics += 1;
    match head {
        "gr_text" => counts.gr_texts += 1,
        "gr_line" => counts.gr_lines += 1,
        "gr_rect" => counts.gr_rects += 1,
        "gr_arc" => counts.gr_arcs += 1,
        "gr_circle" => counts.gr_circles += 1,
        "gr_poly" => counts.gr_polys += 1,
        "gr_curve" => counts.gr_curves += 1,
        "gr_text_box" => counts.gr_text_boxes += 1,
        _ => {}
    }
}

pub(super) fn index_embedded_files(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    let children = direct_children(source, span, limits.max_embedded_files, limits)?;
    for child in children
        .into_iter()
        .filter(|child| child.head.as_deref() == Some("file"))
    {
        bounded_push(&mut index.embedded_files, child, limits.max_embedded_files)?;
    }
    index.counts.embedded_files = index.embedded_files.len();
    Ok(())
}

pub(super) fn index_layers(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    let forms = direct_children(source, span, limits.max_layers, limits)?;
    index.counts.layers = index
        .counts
        .layers
        .checked_add(forms.len())
        .ok_or_else(limit_error)?;
    if index.counts.layers > limits.max_layers {
        return Err(limit_error());
    }
    index.layer_forms.extend(forms);
    Ok(())
}

pub(super) fn index_footprint(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    selection: PcbSelection,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    if index.footprints.len() == limits.max_footprints {
        return Err(limit_error());
    }
    let children = direct_children(source, span, limits.max_footprint_children, limits)?;
    let footprint_index = index.footprints.len();
    let mut counts = FootprintChildCounts::default();
    for child in children {
        let indexed = IndexedNestedForm {
            parent_index: footprint_index,
            span: child,
        };
        index_footprint_child(indexed, limits, selection, index, &mut counts)?;
    }
    index.footprints.push(IndexedFootprint {
        span: span.clone(),
        property_count: counts.properties,
        graphic_count: counts.graphics,
        text_count: counts.texts,
        text_box_count: counts.text_boxes,
        pad_count: counts.pads,
        model_count: counts.models,
    });
    index.counts.footprint_properties = index.footprint_properties.len();
    index.counts.pads = index.pads.len();
    index.counts.models = index.models.len();
    index.counts.footprint_graphics = index.footprint_graphics.len();
    index.counts.footprint_texts = index.footprint_texts.len();
    index.counts.footprint_text_boxes = index.footprint_text_boxes.len();
    index.counts.footprints += 1;
    Ok(())
}

#[derive(Default)]
struct FootprintChildCounts {
    properties: usize,
    graphics: usize,
    texts: usize,
    text_boxes: usize,
    pads: usize,
    models: usize,
}

fn index_footprint_child(
    indexed: IndexedNestedForm,
    limits: PcbLimits,
    selection: PcbSelection,
    index: &mut PcbIndex,
    counts: &mut FootprintChildCounts,
) -> Result<(), Error> {
    match indexed.span.head.as_deref() {
        Some("property") => retain_nested(
            &mut counts.properties,
            &mut index.footprint_properties,
            indexed,
            selection.contains(PcbFamily::FootprintProperties),
            limits.max_footprint_properties,
        ),
        Some("pad") => retain_nested(
            &mut counts.pads,
            &mut index.pads,
            indexed,
            selection.contains(PcbFamily::Pads),
            limits.max_pads,
        ),
        Some("model") => retain_nested(
            &mut counts.models,
            &mut index.models,
            indexed,
            selection.contains(PcbFamily::Models),
            limits.max_models,
        ),
        Some(head) if physical::is_footprint_profile_head(head) => retain_nested(
            &mut counts.graphics,
            &mut index.footprint_graphics,
            indexed,
            selection.contains(PcbFamily::FootprintGraphics)
                || selection.contains(PcbFamily::Profile),
            limits.max_footprint_graphics,
        ),
        Some("fp_text") => retain_nested(
            &mut counts.texts,
            &mut index.footprint_texts,
            indexed,
            selection.contains(PcbFamily::FootprintTexts),
            limits.max_footprint_texts,
        ),
        Some("fp_text_box") => retain_nested(
            &mut counts.text_boxes,
            &mut index.footprint_text_boxes,
            indexed,
            selection.contains(PcbFamily::FootprintTextBoxes),
            limits.max_footprint_text_boxes,
        ),
        _ => Ok(()),
    }
}

fn retain_nested(
    count: &mut usize,
    values: &mut Vec<IndexedNestedForm>,
    value: IndexedNestedForm,
    retain: bool,
    maximum: usize,
) -> Result<(), Error> {
    *count = count.checked_add(1).ok_or_else(limit_error)?;
    if retain {
        bounded_push(values, value, maximum)?;
    }
    Ok(())
}
