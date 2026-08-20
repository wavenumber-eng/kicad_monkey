//! Bounded placed-symbol composition from embedded schematic library data.

use super::annotation_render::{apply_center_defaults, schematic_text, text_style};
use super::{
    PlotBudget, SchematicDrawingSettings, SchematicPlotContext, SchematicPlotLimits,
    SchematicPlotOperation, SchematicPlotRecord, SchematicStyledThickSegment,
    SchematicSymbolInstanceRecord, SchematicSymbolOverplotRecord, SchematicSymbolPinAttrs,
    SchematicSymbolPinBlock, SchematicTextOperation, child, child_string, limit_error, list,
    mm_to_nm, model_error, number_at, parse_span, scalar_at, text, value_at,
};
use crate::plotter_text_cache::{PlotterTextCacheSession, PlotterTextLayout};
use crate::plotter_types::{
    PlotterOperation, PlotterText, PlotterTextHAlign, PlotterTextVAlign, ThickSegment,
};
use crate::sexpr::{Error, Position, Sexp};
use crate::sexpr_projection::FormSpan;
use crate::symbol_pin::pin_operations;
use crate::symbol_plotter_ir::{convert_shape, split_filled_outline, subsymbol_identity};
use crate::symbol_text::{
    SymbolTextBudget, SymbolTextSettings, SymbolTextVariables, body_text_operation,
    pin_text_operations,
};
use crate::{SymbolPlotLimits, TextHorizontalAlignment, TextVerticalAlignment};
use std::collections::{BTreeMap, BTreeSet};

const REFERENCE_COLOR: &str = "#006464FF";
const VALUE_COLOR: &str = "#006464FF";
const FIELDS_COLOR: &str = "#840084FF";
const DNP_COLOR: &str = "#DC090DD9";
const BACKGROUND_COLOR: &str = "#F5F4EFFF";
const DNP_WIDTH_NM: i64 = 457_200;
const JS_SAFE_MAX: i64 = 9_007_199_254_740_991;

#[derive(Clone)]
struct PlacedSymbol {
    lib_id: String,
    lib_name: String,
    at_x_nm: i64,
    at_y_nm: i64,
    angle: f64,
    mirror: Option<String>,
    unit: u32,
    convert: u32,
    in_bom: bool,
    on_board: bool,
    dnp: bool,
    exclude_from_sim: bool,
    in_pos_files: bool,
    uuid: String,
    properties: Vec<Sexp>,
    placed_pins: BTreeMap<String, PlacedPin>,
    references: Vec<(String, String)>,
}

#[derive(Clone)]
struct PlacedPin {
    uuid: String,
    alternate: Option<String>,
}

#[derive(Clone, Copy)]
struct Transform {
    offset_x: i64,
    offset_y: i64,
    rotation: f64,
    mirror_x: bool,
    mirror_y: bool,
}

struct PrimaryEntry {
    symbol: PlacedSymbol,
    library_index: Option<usize>,
    dnp_markers: Vec<SchematicPlotOperation>,
    record: SchematicSymbolInstanceRecord,
    overlap_bbox: Option<Bbox>,
    unit_count: u32,
}

#[derive(Clone, Copy)]
pub(super) struct Bbox {
    pub(super) left: i64,
    pub(super) top: i64,
    pub(super) right: i64,
    pub(super) bottom: i64,
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the terminal symbol phase shares one bounded parse, font, and overlap session"
)]
pub(super) fn append_symbol_records(
    source: &str,
    library_span: Option<&FormSpan>,
    symbol_spans: &[FormSpan],
    context: &SchematicPlotContext,
    drawing: SchematicDrawingSettings,
    variables: &BTreeMap<String, String>,
    metrics: Option<&PlotterTextCacheSession<'_>>,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    let library = library_span
        .map(|span| parse_span(source, span, limits))
        .transpose()?;
    let libraries = library.as_ref().map_or(Vec::new(), direct_symbols);
    if libraries.len() > limits.max_library_symbols {
        return Err(limit_error());
    }
    let mut entries = Vec::with_capacity(symbol_spans.len());
    let mut total_properties = 0usize;
    let mut total_placed_pins = 0usize;
    let mut total_subsymbols = 0usize;
    let mut total_library_pins = 0usize;
    for span in symbol_spans {
        let form = parse_span(source, span, limits)?;
        let symbol = parse_placed_symbol(&form)?;
        total_properties = checked_count(
            total_properties,
            symbol.properties.len(),
            limits.max_symbol_properties,
        )?;
        total_placed_pins = checked_count(
            total_placed_pins,
            symbol.placed_pins.len(),
            limits.max_symbol_pins,
        )?;
        let library_index = resolve_library(&libraries, &symbol.lib_name, &symbol.lib_id);
        let mut body_ops = Vec::new();
        let mut pin_ops = Vec::new();
        let mut pin_root_bboxes = Vec::new();
        let mut unit_count = 1u32;
        if let Some(index) = library_index {
            let header = libraries[index];
            let geometry = resolve_geometry(&libraries, index);
            let subsymbols = direct_symbols(geometry);
            total_subsymbols = checked_count(
                total_subsymbols,
                subsymbols.len(),
                limits.max_library_subsymbols,
            )?;
            unit_count = subsymbols
                .iter()
                .map(|sub| subsymbol_identity(value_at(sub, 1).unwrap_or_default().as_str()).0)
                .max()
                .unwrap_or(1)
                .max(1);
            let selected = subsymbols
                .into_iter()
                .filter(|sub| {
                    let (unit, style) =
                        subsymbol_identity(value_at(sub, 1).unwrap_or_default().as_str());
                    selected_subsymbol(unit, style, symbol.unit, symbol.convert)
                })
                .collect::<Vec<_>>();
            total_library_pins = checked_count(
                total_library_pins,
                selected
                    .iter()
                    .map(|sub| children(sub, "pin").count())
                    .sum(),
                limits.max_library_pins,
            )?;
            budget.charge_input_points(symbol_input_points(&symbol, &selected)?)?;
            let transform = placement_transform(&symbol);
            pin_root_bboxes = placed_pin_root_bboxes(&selected, transform)?;
            body_ops = body_operations(&selected, variables, transform, limits, budget)?;
            pin_ops = placed_pin_operations(
                &symbol,
                header,
                &selected,
                transform,
                drawing.default_line_width_nm,
                limits,
                budget,
                "",
            )?;
        }
        let dnp_markers = if symbol.dnp {
            dnp_marker_operations(&body_ops, &pin_ops, &pin_root_bboxes, metrics)?
        } else {
            Vec::new()
        };
        let estimated_operations = body_ops
            .len()
            .checked_add(pin_ops.len())
            .and_then(|value| value.checked_add(dnp_markers.len()))
            .filter(|value| *value <= budget.remaining_operations())
            .ok_or_else(limit_error)?;
        let mut operations = Vec::with_capacity(estimated_operations);
        operations.append(&mut body_ops);
        operations.append(&mut pin_ops);
        append_properties(
            &symbol,
            unit_count,
            context,
            drawing.default_line_width_nm,
            metrics,
            budget,
            &mut operations,
            dnp_markers.len(),
        )?;
        if symbol.dnp {
            dim_operations(&mut operations);
            operations.extend(dnp_markers.iter().cloned());
        }
        charge_operations(budget, &operations)?;
        let reference = instance_reference(&symbol, &context.sheet_instance_path);
        let metadata = symbol_metadata_bytes(&symbol, &reference)?;
        budget.charge_metadata(metadata)?;
        let record = SchematicSymbolInstanceRecord {
            uuid: symbol.uuid.clone(),
            lib_id: symbol.lib_id.clone(),
            lib_name: symbol.lib_name.clone(),
            reference,
            at_x_nm: symbol.at_x_nm,
            at_y_nm: symbol.at_y_nm,
            at_angle_deg: symbol.angle,
            mirror: symbol.mirror.clone(),
            unit: symbol.unit,
            convert: symbol.convert,
            in_bom: symbol.in_bom,
            on_board: symbol.on_board,
            dnp: symbol.dnp,
            exclude_from_sim: symbol.exclude_from_sim,
            in_pos_files: symbol.in_pos_files,
            operations,
        };
        let overlap_bbox = symbol_overlap_bbox(&record.operations, metrics)?;
        entries.push(PrimaryEntry {
            symbol,
            library_index,
            dnp_markers,
            record,
            overlap_bbox,
            unit_count,
        });
    }
    let overlapping = overlapping_indices(&entries, limits.max_symbol_overlap_checks)?;
    let mut overplot_records = Vec::new();
    let mut overplot_count = 0usize;
    for index in overlapping {
        let entry = &entries[index];
        let Some(library_index) = entry.library_index else {
            continue;
        };
        overplot_count = checked_count(overplot_count, 1, limits.max_symbol_overplots)?;
        let header = libraries[library_index];
        let geometry = resolve_geometry(&libraries, library_index);
        let selected = direct_symbols(geometry)
            .into_iter()
            .filter(|sub| {
                let (unit, style) =
                    subsymbol_identity(value_at(sub, 1).unwrap_or_default().as_str());
                selected_subsymbol(unit, style, entry.symbol.unit, entry.symbol.convert)
            })
            .collect::<Vec<_>>();
        let mut operations = Vec::new();
        append_properties(
            &entry.symbol,
            entry.unit_count,
            context,
            drawing.default_line_width_nm,
            metrics,
            budget,
            &mut operations,
            entry.dnp_markers.len(),
        )?;
        let mut overplot_pins = placed_pin_operations(
            &entry.symbol,
            header,
            &selected,
            placement_transform(&entry.symbol),
            drawing.default_line_width_nm,
            limits,
            budget,
            "__overplot",
        )?;
        ensure_operation_room(
            operations.len(),
            overplot_pins.len().saturating_add(entry.dnp_markers.len()),
            budget,
        )?;
        operations.append(&mut overplot_pins);
        if entry.symbol.dnp {
            dim_operations(&mut operations);
            operations.extend(entry.dnp_markers.iter().cloned());
        }
        if operations.is_empty() {
            continue;
        }
        charge_operations(budget, &operations)?;
        let uuid = format!("{}:overplot", entry.symbol.uuid);
        budget.charge_metadata(
            uuid.len()
                .saturating_add(entry.symbol.uuid.len())
                .saturating_add(entry.symbol.lib_id.len()),
        )?;
        overplot_records.push(SchematicPlotRecord::SymbolOverplot(
            SchematicSymbolOverplotRecord {
                uuid,
                source_symbol_uuid: entry.symbol.uuid.clone(),
                lib_id: entry.symbol.lib_id.clone(),
                operations,
            },
        ));
    }
    records.extend(
        entries
            .into_iter()
            .map(|entry| SchematicPlotRecord::SymbolInstance(entry.record)),
    );
    records.extend(overplot_records);
    Ok(())
}

fn direct_symbols(form: &Sexp) -> Vec<&Sexp> {
    children(form, "symbol").collect()
}

fn children<'a>(form: &'a Sexp, head: &'a str) -> impl Iterator<Item = &'a Sexp> {
    list(form).into_iter().flatten().filter(move |child| {
        list(child).and_then(|values| values.first()).and_then(text) == Some(head)
    })
}

fn resolve_library(libraries: &[&Sexp], lib_name: &str, lib_id: &str) -> Option<usize> {
    [lib_name, lib_id]
        .into_iter()
        .filter(|value| !value.is_empty())
        .find_map(|key| {
            libraries.iter().position(|library| {
                let name = value_at(library, 1).unwrap_or_default();
                name == key
                    || name.ends_with(&format!(":{key}"))
                    || key.rsplit_once(':').is_some_and(|(_, short)| {
                        name == short || name.ends_with(&format!(":{short}"))
                    })
            })
        })
}

fn resolve_geometry<'a>(libraries: &[&'a Sexp], start: usize) -> &'a Sexp {
    let mut current = start;
    let mut seen = BTreeSet::new();
    loop {
        let library = libraries[current];
        if children(library, "symbol").next().is_some() || !seen.insert(current) {
            return library;
        }
        let Some(base) = child_string(library, "extends") else {
            return library;
        };
        let Some(next) = resolve_library(libraries, "", &base) else {
            return library;
        };
        current = next;
    }
}

fn selected_subsymbol(unit: u32, style: u32, requested_unit: u32, requested_style: u32) -> bool {
    let unit_matches = unit == 0 || unit == requested_unit;
    let style_matches =
        style == 0 || style == requested_style || (requested_style == 0 && style == 1);
    unit_matches && style_matches
}

fn parse_placed_symbol(form: &Sexp) -> Result<PlacedSymbol, Error> {
    let at = child(form, "at");
    let at_x_nm = at.map_or(Ok(0), |at| mm_to_nm(number_at(at, 1)?))?;
    let at_y_nm = at.map_or(Ok(0), |at| mm_to_nm(number_at(at, 2)?))?;
    let angle = at
        .filter(|at| list(at).is_some_and(|values| values.len() > 3))
        .map_or(Ok(0.0), |at| number_at(at, 3))?;
    let properties = children(form, "property").cloned().collect::<Vec<_>>();
    let mut placed_pins = BTreeMap::new();
    for pin in children(form, "pin") {
        let number = value_at(pin, 1).unwrap_or_default();
        placed_pins.insert(
            number,
            PlacedPin {
                uuid: child_string(pin, "uuid").unwrap_or_default(),
                alternate: child_string(pin, "alternate"),
            },
        );
    }
    let mut references = Vec::new();
    if let Some(instances) = child(form, "instances") {
        for project in children(instances, "project") {
            for path in children(project, "path") {
                let value = value_at(path, 1).unwrap_or_default();
                let reference = child_string(path, "reference").unwrap_or_default();
                if !reference.is_empty() {
                    references.push((value.trim_end_matches('/').to_owned(), reference));
                }
            }
        }
    }
    let mirror = child_string(form, "mirror");
    if mirror
        .as_deref()
        .is_some_and(|value| value != "x" && value != "y")
    {
        return Err(model_error("Unsupported schematic symbol mirror"));
    }
    let symbol = PlacedSymbol {
        lib_id: child_string(form, "lib_id").unwrap_or_default(),
        lib_name: child_string(form, "lib_name").unwrap_or_default(),
        at_x_nm,
        at_y_nm,
        angle,
        mirror,
        unit: child_u32(form, "unit", 1)?,
        convert: child_u32(form, "convert", 1)?,
        in_bom: child_yes(form, "in_bom", true),
        on_board: child_yes(form, "on_board", true),
        dnp: child_yes(form, "dnp", false),
        exclude_from_sim: child_yes(form, "exclude_from_sim", false),
        in_pos_files: child_yes(form, "in_pos_files", true),
        uuid: child_string(form, "uuid").unwrap_or_default(),
        properties,
        placed_pins,
        references,
    };
    Ok(symbol)
}

fn child_u32(form: &Sexp, head: &str, default: u32) -> Result<u32, Error> {
    let Some(value) = child(form, head) else {
        return Ok(default);
    };
    scalar_at(value, 1)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| model_error("Expected schematic symbol integer"))
}

fn child_yes(form: &Sexp, head: &str, default: bool) -> bool {
    child(form, head)
        .and_then(|value| scalar_at(value, 1))
        .map_or(default, |value| value == "yes")
}

fn body_operations(
    subsymbols: &[&Sexp],
    variables: &BTreeMap<String, String>,
    transform: Transform,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
) -> Result<Vec<SchematicPlotOperation>, Error> {
    let symbol_variables = SymbolTextVariables::from_entries(
        variables
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    let max_text_carriers = limits
        .max_library_subsymbols
        .checked_add(limits.max_library_pins.saturating_mul(2))
        .ok_or_else(limit_error)?;
    let mut text_budget = SymbolTextBudget::new(SymbolPlotLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_symbols: limits.max_library_symbols,
        max_subsymbols: limits.max_library_subsymbols,
        max_operations: budget.remaining_operations(),
        max_points: budget.remaining_points(),
        max_text_carriers,
        max_text_bytes: budget.remaining_text_bytes(),
    });
    let mut point_count = 0usize;
    let mut operations = Vec::new();
    let mut outlines = Vec::new();
    for subsymbol in subsymbols {
        for head in ["rectangle", "circle", "arc", "polyline", "bezier"] {
            for shape in children(subsymbol, head) {
                let Some(operation) = convert_shape(
                    shape,
                    head,
                    budget.remaining_points(),
                    &mut point_count,
                    Position::START,
                )?
                else {
                    continue;
                };
                let (fill, outline) = split_filled_outline(operation);
                ensure_operation_room(
                    operations.len().saturating_add(outlines.len()),
                    1 + usize::from(outline.is_some()),
                    budget,
                )?;
                operations.push(transform_operation(fill, transform)?);
                if let Some(outline) = outline {
                    outlines.push(transform_operation(outline, transform)?);
                }
            }
        }
        for body_text in children(subsymbol, "text") {
            if let Some(operation) = body_text_operation(
                body_text,
                &symbol_variables,
                &mut text_budget,
                Position::START,
            )? {
                ensure_operation_room(operations.len().saturating_add(outlines.len()), 1, budget)?;
                operations.push(transform_operation(operation, transform)?);
            }
        }
    }
    operations.extend(outlines);
    for operation in &operations {
        charge_operation_payload(budget, operation)?;
    }
    Ok(operations)
}

#[allow(
    clippy::too_many_arguments,
    reason = "pin rendering requires the placed carrier, selected library unit, and shared budgets"
)]
fn placed_pin_operations(
    symbol: &PlacedSymbol,
    header: &Sexp,
    subsymbols: &[&Sexp],
    transform: Transform,
    default_line_width_nm: i64,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    suffix: &str,
) -> Result<Vec<SchematicPlotOperation>, Error> {
    let settings = SymbolTextSettings::from_header(header, Position::START)?;
    let mut text_budget = SymbolTextBudget::new(SymbolPlotLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_symbols: limits.max_library_symbols,
        max_subsymbols: limits.max_library_subsymbols,
        max_operations: budget.remaining_operations(),
        max_points: budget.remaining_points(),
        max_text_carriers: limits.max_library_pins.saturating_mul(2),
        max_text_bytes: budget.remaining_text_bytes(),
    });
    let mut point_count = 0usize;
    let mut result = Vec::new();
    let mut seen = BTreeMap::<String, usize>::new();
    for subsymbol in subsymbols {
        for pin in children(subsymbol, "pin") {
            if hidden(pin) {
                continue;
            }
            let number = child(pin, "number")
                .and_then(|value| value_at(value, 1))
                .unwrap_or_default();
            let placed = symbol.placed_pins.get(&number);
            let pin = selected_alternate_pin(pin, placed.and_then(|pin| pin.alternate.as_deref()));
            let mut inner = Vec::new();
            for operation in pin_operations(
                &pin,
                budget.remaining_points(),
                &mut point_count,
                Position::START,
            )? {
                inner.push(transform_operation(operation, transform)?);
            }
            for operation in pin_text_operations(
                &pin,
                settings,
                Some(default_line_width_nm),
                &mut text_budget,
                Position::START,
            )? {
                inner.push(transform_pin_text(operation, transform)?);
            }
            if inner.is_empty() {
                continue;
            }
            ensure_operation_room(result.len(), inner.len().saturating_add(2), budget)?;
            let source_uuid = placed.map_or("", |pin| pin.uuid.as_str());
            let base = pin_group_id(&symbol.uuid, &number, source_uuid);
            if base.is_empty() {
                result.extend(inner);
                continue;
            }
            let candidate = format!("{base}{suffix}");
            let count = seen.entry(candidate.clone()).or_default();
            *count += 1;
            let label = if *count == 1 {
                candidate
            } else {
                format!("{candidate}__{}", *count)
            };
            let lib_pin_uuid = child_string(&pin, "uuid").unwrap_or_default();
            let block = SchematicSymbolPinBlock {
                label: label.clone(),
                data_uuid: label,
                object_id: if source_uuid.is_empty() {
                    base
                } else {
                    source_uuid.to_owned()
                },
                extra_attrs: SchematicSymbolPinAttrs {
                    primitive: "pin".to_owned(),
                    object_type: "pin".to_owned(),
                    pin: number,
                    symbol_uuid: symbol.uuid.clone(),
                    designator: authored_reference(symbol),
                    lib_pin_uuid,
                },
            };
            budget.charge_metadata(block_metadata_bytes(&block))?;
            result.push(SchematicPlotOperation::StartSymbolPinBlock(block));
            result.extend(inner);
            result.push(SchematicPlotOperation::EndBlock);
        }
    }
    for operation in &result {
        charge_operation_payload(budget, operation)?;
    }
    Ok(result)
}

fn placed_pin_root_bboxes(subsymbols: &[&Sexp], transform: Transform) -> Result<Vec<Bbox>, Error> {
    let mut result = Vec::new();
    for subsymbol in subsymbols {
        for pin in children(subsymbol, "pin") {
            if hidden(pin) {
                continue;
            }
            let at = child(pin, "at");
            let x_mm = at.map_or(Ok(0.0), |value| number_at(value, 1))?;
            let y_mm = at.map_or(Ok(0.0), |value| number_at(value, 2))?;
            let angle = at
                .filter(|value| list(value).is_some_and(|values| values.len() > 3))
                .map_or(Ok(0.0), |value| number_at(value, 3))?;
            let length_mm = child(pin, "length").map_or(Ok(0.0), |value| number_at(value, 1))?;
            let rounded = angle.round_ties_even() as i64;
            let root = match rounded.rem_euclid(360) {
                0 => [mm_to_nm(x_mm + length_mm)?, mm_to_nm(-y_mm)?],
                180 => [mm_to_nm(x_mm - length_mm)?, mm_to_nm(-y_mm)?],
                90 => [mm_to_nm(x_mm)?, mm_to_nm(-(y_mm + length_mm))?],
                270 => [mm_to_nm(x_mm)?, mm_to_nm(-(y_mm - length_mm))?],
                _ => {
                    let radians = angle.to_radians();
                    [
                        mm_to_nm(x_mm + length_mm * radians.cos())?,
                        mm_to_nm(-(y_mm + length_mm * radians.sin()))?,
                    ]
                }
            };
            let [x, y] = transform_point(root, transform)?;
            result.push(Bbox {
                left: x,
                top: y,
                right: x,
                bottom: y,
            });
        }
    }
    Ok(result)
}

fn selected_alternate_pin(pin: &Sexp, alternate: Option<&str>) -> Sexp {
    let Some(alternate) = alternate else {
        return pin.clone();
    };
    let mut output = pin.clone();
    let Sexp::List(values) = &mut output else {
        return output;
    };
    for alt in children(pin, "alternate") {
        if value_at(alt, 1).as_deref() == Some(alternate) {
            if let Some(style) = scalar_at(alt, 3)
                && values.len() > 2
            {
                values[2] = Sexp::Atom(style);
            }
            if let Some(name) = values.iter_mut().find(|value| {
                list(value).and_then(|values| values.first()).and_then(text) == Some("name")
            }) && let Sexp::List(name_values) = name
                && name_values.len() > 1
            {
                name_values[1] = Sexp::Quoted(alternate.to_owned());
            }
            break;
        }
    }
    output
}

fn placement_transform(symbol: &PlacedSymbol) -> Transform {
    Transform {
        offset_x: symbol.at_x_nm,
        offset_y: symbol.at_y_nm,
        rotation: -symbol.angle,
        mirror_x: symbol.mirror.as_deref() == Some("x"),
        mirror_y: symbol.mirror.as_deref() == Some("y"),
    }
}

fn transform_point(point: [i64; 2], transform: Transform) -> Result<[i64; 2], Error> {
    let angle = transform.rotation.rem_euclid(360.0);
    let (mut x, mut y) = match angle {
        0.0 => (point[0] as f64, point[1] as f64),
        90.0 => (-(point[1] as f64), point[0] as f64),
        180.0 => (-(point[0] as f64), -(point[1] as f64)),
        270.0 => (point[1] as f64, -(point[0] as f64)),
        _ => {
            let radians = angle.to_radians();
            (
                point[0] as f64 * radians.cos() - point[1] as f64 * radians.sin(),
                point[0] as f64 * radians.sin() + point[1] as f64 * radians.cos(),
            )
        }
    };
    if transform.mirror_x {
        y = -y;
    }
    if transform.mirror_y {
        x = -x;
    }
    checked_point(
        rounded_coordinate(x, transform.offset_x)?,
        rounded_coordinate(y, transform.offset_y)?,
    )
}

fn rounded_coordinate(value: f64, offset: i64) -> Result<i64, Error> {
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(model_error(
            "Derived schematic symbol coordinate is invalid",
        ));
    }
    let rounded = value.round_ties_even() as i64;
    rounded
        .checked_add(offset)
        .filter(|value| (-JS_SAFE_MAX..=JS_SAFE_MAX).contains(value))
        .ok_or_else(|| model_error("Derived schematic symbol coordinate is unsafe"))
}

fn transform_orient(angle: f64, transform: Transform) -> Result<f64, Error> {
    let mut value = angle + transform.rotation;
    if transform.mirror_x {
        value = -value;
    }
    if transform.mirror_y {
        value = -value;
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err(model_error("Derived schematic symbol angle is invalid"))
    }
}

fn transform_operation(
    operation: PlotterOperation,
    transform: Transform,
) -> Result<SchematicPlotOperation, Error> {
    let transformed = match operation {
        PlotterOperation::ThickSegment(mut value) => {
            [value.start_x, value.start_y] =
                transform_point([value.start_x, value.start_y], transform)?;
            [value.end_x, value.end_y] = transform_point([value.end_x, value.end_y], transform)?;
            PlotterOperation::ThickSegment(value)
        }
        PlotterOperation::ArcThreePoint(mut value) => {
            [value.start_x, value.start_y] =
                transform_point([value.start_x, value.start_y], transform)?;
            [value.mid_x, value.mid_y] = transform_point([value.mid_x, value.mid_y], transform)?;
            [value.end_x, value.end_y] = transform_point([value.end_x, value.end_y], transform)?;
            PlotterOperation::ArcThreePoint(value)
        }
        PlotterOperation::Circle(mut value) => {
            [value.cx, value.cy] = transform_point([value.cx, value.cy], transform)?;
            PlotterOperation::Circle(value)
        }
        PlotterOperation::Rect(mut value) => {
            [value.x1, value.y1] = transform_point([value.x1, value.y1], transform)?;
            [value.x2, value.y2] = transform_point([value.x2, value.y2], transform)?;
            PlotterOperation::Rect(value)
        }
        PlotterOperation::PlotPoly(mut value) => {
            value.points = value
                .points
                .into_iter()
                .map(|point| transform_point(point, transform))
                .collect::<Result<Vec<_>, _>>()?;
            PlotterOperation::PlotPoly(value)
        }
        PlotterOperation::BezierCurve(mut value) => {
            [value.start_x, value.start_y] =
                transform_point([value.start_x, value.start_y], transform)?;
            [value.ctrl1_x, value.ctrl1_y] =
                transform_point([value.ctrl1_x, value.ctrl1_y], transform)?;
            [value.ctrl2_x, value.ctrl2_y] =
                transform_point([value.ctrl2_x, value.ctrl2_y], transform)?;
            [value.end_x, value.end_y] = transform_point([value.end_x, value.end_y], transform)?;
            PlotterOperation::BezierCurve(value)
        }
        PlotterOperation::Text(mut value) => {
            [value.x, value.y] = transform_point([value.x, value.y], transform)?;
            let local = value.orient_deg;
            value.orient_deg = transform_orient(local, transform)?;
            apply_device_text_attributes(&mut value, local, transform);
            return Ok(SchematicPlotOperation::Text(SchematicTextOperation {
                text: value,
                hyperlink_href: None,
            }));
        }
        unsupported => unsupported,
    };
    Ok(SchematicPlotOperation::Plotter(transformed))
}

fn transform_pin_text(
    operation: PlotterOperation,
    transform: Transform,
) -> Result<SchematicPlotOperation, Error> {
    let PlotterOperation::Text(mut text) = operation else {
        return transform_operation(operation, transform);
    };
    [text.x, text.y] = transform_point([text.x, text.y], transform)?;
    text.orient_deg = transform_orient(text.orient_deg, transform)?;
    if transform.mirror_x ^ transform.mirror_y {
        text.h_align = flip_h(text.h_align);
    }
    Ok(SchematicPlotOperation::Text(SchematicTextOperation {
        text,
        hyperlink_href: None,
    }))
}

fn apply_device_text_attributes(text: &mut PlotterText, local_angle: f64, transform: Transform) {
    let (x1, y1, x2, y2) = transform_matrix(transform);
    let original_horizontal = rounded_angle(local_angle) % 180 == 0;
    let screen_horizontal = (x1 != 0) ^ !original_horizontal;
    text.orient_deg = if screen_horizontal { 0.0 } else { 90.0 };
    let flip_horizontal = if original_horizontal {
        if screen_horizontal { x1 < 0 } else { x2 > 0 }
    } else if screen_horizontal {
        y1 > 0
    } else {
        y2 < 0
    };
    if flip_horizontal {
        text.h_align = flip_h(text.h_align);
    }
    let determinant = x1 * y2 - x2 * y1;
    if determinant < 0 && (original_horizontal == (x1 > 0)) {
        text.v_align = flip_v(text.v_align);
    }
}

fn transform_matrix(transform: Transform) -> (i64, i64, i64, i64) {
    let mut matrix = match rounded_angle(-transform.rotation) {
        90 => (0, 1, -1, 0),
        180 => (-1, 0, 0, -1),
        270 => (0, -1, 1, 0),
        _ => (1, 0, 0, 1),
    };
    if transform.mirror_x {
        matrix = compose_matrix(matrix, (1, 0, 0, -1));
    }
    if transform.mirror_y {
        matrix = compose_matrix(matrix, (-1, 0, 0, 1));
    }
    matrix
}

fn compose_matrix(
    (x1, y1, x2, y2): (i64, i64, i64, i64),
    (tx1, ty1, tx2, ty2): (i64, i64, i64, i64),
) -> (i64, i64, i64, i64) {
    (
        x1 * tx1 + x2 * ty1,
        y1 * tx1 + y2 * ty1,
        x1 * tx2 + x2 * ty2,
        y1 * tx2 + y2 * ty2,
    )
}

fn flip_h(value: PlotterTextHAlign) -> PlotterTextHAlign {
    match value {
        PlotterTextHAlign::Left => PlotterTextHAlign::Right,
        PlotterTextHAlign::Right => PlotterTextHAlign::Left,
        PlotterTextHAlign::Center => PlotterTextHAlign::Center,
    }
}

fn flip_v(value: PlotterTextVAlign) -> PlotterTextVAlign {
    match value {
        PlotterTextVAlign::Top => PlotterTextVAlign::Bottom,
        PlotterTextVAlign::Bottom => PlotterTextVAlign::Top,
        PlotterTextVAlign::Center => PlotterTextVAlign::Center,
    }
}

fn rounded_angle(value: f64) -> i64 {
    (value.round_ties_even() as i64).rem_euclid(360)
}

#[allow(
    clippy::too_many_arguments,
    reason = "field emission shares placed-symbol geometry, occurrence context, metrics, and budgets"
)]
fn append_properties(
    symbol: &PlacedSymbol,
    unit_count: u32,
    context: &SchematicPlotContext,
    default_line_width_nm: i64,
    metrics: Option<&PlotterTextCacheSession<'_>>,
    budget: &mut PlotBudget,
    operations: &mut Vec<SchematicPlotOperation>,
    reserved_operations: usize,
) -> Result<(), Error> {
    let suffix = if unit_count > 1 {
        unit_suffix(symbol.unit)
    } else {
        String::new()
    };
    for property in &symbol.properties {
        if hidden(property) {
            continue;
        }
        let key = value_at(property, 1).unwrap_or_default();
        let mut value = resolved_property_value(property, symbol);
        if key == "Reference" {
            value = instance_reference(symbol, &context.sheet_instance_path);
            if !value.is_empty() && !suffix.is_empty() && !value.ends_with(&suffix) {
                value.push_str(&suffix);
            }
        }
        if value == "~" {
            continue;
        }
        if child_yes(property, "show_name", false) {
            value = format!("{key}: {value}");
        }
        if value.is_empty() {
            continue;
        }
        ensure_operation_room(
            operations.len(),
            reserved_operations.saturating_add(1),
            budget,
        )?;
        let color = match key.as_str() {
            "Reference" => REFERENCE_COLOR,
            "Value" => VALUE_COLOR,
            _ => FIELDS_COLOR,
        };
        let at = super::annotation_render::parse_at(property)?;
        let mut style = text_style(property, color, Some(default_line_width_nm))?;
        apply_center_defaults(property, &mut style);
        style.h_align = PlotterTextHAlign::Center;
        style.v_align = PlotterTextVAlign::Center;
        let orient = if rounded_angle(symbol.angle) % 180 == 90 {
            if rounded_angle(at.angle) % 180 == 0 {
                90.0
            } else {
                0.0
            }
        } else {
            at.angle
        };
        let (x, y) = property_center(property, symbol, &value, &style, metrics)?;
        budget.charge_text(value.len())?;
        budget.charge_metadata(
            key.len()
                .saturating_add(style.color.len())
                .saturating_add(style.font_face.len())
                .saturating_add(style.hyperlink_href.as_deref().map_or(0, str::len)),
        )?;
        operations.push(schematic_text(x, y, value, orient, style, false));
    }
    Ok(())
}

fn resolved_property_value(property: &Sexp, symbol: &PlacedSymbol) -> String {
    let raw = value_at(property, 2).unwrap_or_default();
    if !(raw.starts_with("${") && raw.ends_with('}')) {
        return raw;
    }
    let token = raw[2..raw.len() - 1].trim().to_lowercase();
    symbol
        .properties
        .iter()
        .find(|candidate| {
            value_at(candidate, 1).is_some_and(|key| key.trim().to_lowercase() == token)
        })
        .and_then(|candidate| value_at(candidate, 2))
        .unwrap_or(raw)
}

fn property_center(
    property: &Sexp,
    symbol: &PlacedSymbol,
    value: &str,
    style: &super::annotation_render::TextStyle,
    metrics: Option<&PlotterTextCacheSession<'_>>,
) -> Result<(i64, i64), Error> {
    let at = super::annotation_render::parse_at(property)?;
    let (h, v) = authored_justify(property);
    if h == PlotterTextHAlign::Center && v == PlotterTextVAlign::Center {
        return Ok((at.x, at.y));
    }
    let measured = measure_text(metrics, value, style)?;
    let mut local_x = 0i64;
    if h == PlotterTextHAlign::Left {
        local_x = half_round(measured.0)?;
    } else if h == PlotterTextHAlign::Right {
        local_x = -half_round(measured.0)?;
    }
    let mut local_y = 0i64;
    let line_height_iu = measured.1 / 100;
    let center_offset = checked_add(
        (line_height_iu / 2)
            .checked_sub(ki_round(line_height_iu as f64 * 0.17)?)
            .and_then(|value| value.checked_mul(100))
            .ok_or_else(|| model_error("Schematic symbol field offset overflowed"))?,
        if value.contains("~{") {
            ((line_height_iu as f64 * (1.0 / 6.0)) as i64 / 2)
                .checked_mul(100)
                .ok_or_else(|| model_error("Schematic symbol overbar offset overflowed"))?
        } else {
            0
        },
    )?;
    if v == PlotterTextVAlign::Top {
        local_y = center_offset;
    } else if v == PlotterTextVAlign::Bottom {
        local_y = -center_offset;
    }
    let rotated = rotate_screen(local_x, local_y, at.angle)?;
    let matrix = transform_matrix(placement_transform(symbol));
    let dx = matrix.0 * rotated[0] + matrix.1 * rotated[1];
    let dy = matrix.2 * rotated[0] + matrix.3 * rotated[1];
    Ok((checked_add(at.x, dx)?, checked_add(at.y, dy)?))
}

fn authored_justify(form: &Sexp) -> (PlotterTextHAlign, PlotterTextVAlign) {
    let mut horizontal = PlotterTextHAlign::Center;
    let mut vertical = PlotterTextVAlign::Center;
    if let Some(justify) = child(form, "effects").and_then(|effects| child(effects, "justify")) {
        for token in list(justify).into_iter().flatten().skip(1).filter_map(text) {
            match token {
                "left" => horizontal = PlotterTextHAlign::Left,
                "right" => horizontal = PlotterTextHAlign::Right,
                "top" => vertical = PlotterTextVAlign::Top,
                "bottom" => vertical = PlotterTextVAlign::Bottom,
                _ => {}
            }
        }
    }
    (horizontal, vertical)
}

fn measure_text(
    metrics: Option<&PlotterTextCacheSession<'_>>,
    text: &str,
    style: &super::annotation_render::TextStyle,
) -> Result<(i64, i64), Error> {
    let metrics = metrics
        .ok_or_else(|| model_error("Placed symbol metrics require explicit font resources"))?;
    let measured = metrics.measure(PlotterTextLayout {
        text,
        face: &style.font_face,
        bold: style.bold,
        italic: style.italic,
        size_x: style.size_x_nm as f64 / 1_000_000.0,
        size_y: style.size_y_nm as f64 / 1_000_000.0,
        position_x: 0.0,
        position_y: 0.0,
        angle_degrees: 0.0,
        mirrored: false,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Bottom,
        line_spacing: 1.0,
        stroke_width: style.pen_width_nm as f64 / 1_000_000.0,
    })?;
    Ok((
        round_to_100(measured.width * 1_000_000.0)?,
        round_to_100(measured.line_height * 1_000_000.0)?,
    ))
}

fn instance_reference(symbol: &PlacedSymbol, path: &str) -> String {
    let target = path.trim_end_matches('/');
    symbol
        .references
        .iter()
        .find(|(candidate, _)| candidate == target)
        .or_else(|| symbol.references.first())
        .map(|(_, reference)| reference.clone())
        .unwrap_or_else(|| authored_reference(symbol))
}

fn authored_reference(symbol: &PlacedSymbol) -> String {
    symbol
        .properties
        .iter()
        .find(|property| value_at(property, 1).as_deref() == Some("Reference"))
        .and_then(|property| value_at(property, 2))
        .unwrap_or_default()
}

fn unit_suffix(mut unit: u32) -> String {
    if unit == 0 {
        return String::new();
    }
    let mut letters = Vec::new();
    while unit > 0 {
        unit -= 1;
        letters.push((b'A' + (unit % 26) as u8) as char);
        unit /= 26;
    }
    letters.into_iter().rev().collect()
}

fn pin_group_id(symbol_uuid: &str, pin_number: &str, source_uuid: &str) -> String {
    if !source_uuid.is_empty() {
        return source_uuid.to_owned();
    }
    if symbol_uuid.is_empty() || pin_number.is_empty() {
        return String::new();
    }
    let mut token = String::new();
    let mut pending = false;
    for character in pin_number.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':' | '-') {
            if pending && !token.is_empty() {
                token.push('_');
            }
            token.push(character);
            pending = false;
        } else {
            pending = true;
        }
    }
    if token.is_empty() {
        token.push_str("pin");
    }
    format!("{symbol_uuid}__pin__{token}")
}

fn hidden(form: &Sexp) -> bool {
    child_yes(form, "hide", false)
        || list(form)
            .into_iter()
            .flatten()
            .skip(1)
            .filter_map(text)
            .any(|value| value == "hide")
        || child(form, "effects").is_some_and(|effects| child_yes(effects, "hide", false))
}

pub(super) fn dim_operations(operations: &mut [SchematicPlotOperation]) {
    for operation in operations {
        match operation {
            SchematicPlotOperation::Plotter(operation) => dim_plotter(operation),
            SchematicPlotOperation::Text(operation) => {
                operation.text.color = dim_color(&operation.text.color);
            }
            SchematicPlotOperation::StyledThickSegment(operation) => {
                operation.stroke_color = dim_color(&operation.stroke_color);
            }
            _ => {}
        }
    }
}

fn dim_plotter(operation: &mut PlotterOperation) {
    macro_rules! dim {
        ($value:expr) => {
            if let Some(color) = $value.as_mut() {
                *color = dim_color(color);
            }
        };
    }
    match operation {
        PlotterOperation::ArcThreePoint(value) => {
            dim!(value.stroke_color);
            dim!(value.fill_color);
        }
        PlotterOperation::Circle(value) => {
            dim!(value.stroke_color);
            dim!(value.fill_color);
        }
        PlotterOperation::Rect(value) => {
            dim!(value.stroke_color);
            dim!(value.fill_color);
        }
        PlotterOperation::PlotPoly(value) => {
            dim!(value.stroke_color);
            dim!(value.fill_color);
        }
        PlotterOperation::BezierCurve(value) => dim!(value.stroke_color),
        PlotterOperation::Text(value) => value.color = dim_color(&value.color),
        _ => {}
    }
}

fn dim_color(color: &str) -> String {
    let Some((r, g, b, a)) = parse_rgba(color) else {
        return color.to_owned();
    };
    let (br, bg, bb, _) = parse_rgba(BACKGROUND_COLOR).expect("constant color");
    let lightness = (r.max(g).max(b) + r.min(g).min(b)) / 2.0;
    rgba_hex(
        (br + lightness) / 2.0,
        (bg + lightness) / 2.0,
        (bb + lightness) / 2.0,
        a,
    )
}

fn parse_rgba(value: &str) -> Option<(f64, f64, f64, f64)> {
    let body = value.strip_prefix('#')?;
    if body.len() != 8 {
        return None;
    }
    let channel = |start| {
        u8::from_str_radix(&body[start..start + 2], 16)
            .ok()
            .map(|v| f64::from(v) / 255.0)
    };
    Some((channel(0)?, channel(2)?, channel(4)?, channel(6)?))
}

fn rgba_hex(r: f64, g: f64, b: f64, a: f64) -> String {
    let channel = |value: f64| ((value * 255.0).round() as i64).clamp(0, 255);
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        channel(r),
        channel(g),
        channel(b),
        channel(a)
    )
}

fn dnp_marker_operations(
    body: &[SchematicPlotOperation],
    pins: &[SchematicPlotOperation],
    pin_root_bboxes: &[Bbox],
    metrics: Option<&PlotterTextCacheSession<'_>>,
) -> Result<Vec<SchematicPlotOperation>, Error> {
    let mut body_bbox = operations_bbox(body, metrics, false)?;
    for pin_root in pin_root_bboxes {
        body_bbox = union_bbox(body_bbox, Some(*pin_root));
    }
    let Some(body_bbox) = body_bbox else {
        return Ok(Vec::new());
    };
    let full_bbox =
        union_bbox(Some(body_bbox), operations_bbox(pins, metrics, false)?).unwrap_or(body_bbox);
    dnp_marker_operations_for_bboxes(body_bbox, full_bbox)
}

pub(super) fn dnp_marker_operations_for_bboxes(
    body_bbox: Bbox,
    full_bbox: Bbox,
) -> Result<Vec<SchematicPlotOperation>, Error> {
    let margin_x = (body_bbox.left - full_bbox.left).max(full_bbox.right - body_bbox.right) as f64;
    let margin_y = (body_bbox.top - full_bbox.top).max(full_bbox.bottom - body_bbox.bottom) as f64;
    let margin_x = (margin_x * 0.6).max(margin_y * 0.3);
    let margin_y = (margin_y * 0.6).max(margin_x * 0.3);
    let left = checked_sub(body_bbox.left, ki_round(margin_x)?)?;
    let top = checked_sub(body_bbox.top, ki_round(margin_y)?)?;
    let right = checked_add(body_bbox.right, ki_round(margin_x)?)?;
    let bottom = checked_add(body_bbox.bottom, ki_round(margin_y)?)?;
    Ok(vec![
        styled_segment(left, top, right, bottom),
        styled_segment(right, top, left, bottom),
    ])
}

fn styled_segment(start_x: i64, start_y: i64, end_x: i64, end_y: i64) -> SchematicPlotOperation {
    SchematicPlotOperation::StyledThickSegment(SchematicStyledThickSegment {
        segment: ThickSegment {
            start_x,
            start_y,
            end_x,
            end_y,
            width_nm: DNP_WIDTH_NM,
            layer: None,
            role: None,
            layers: Vec::new(),
            mask_margin_nm: None,
            pad_size_x_nm: None,
            pad_size_y_nm: None,
        },
        stroke_color: DNP_COLOR.to_owned(),
    })
}

fn symbol_overlap_bbox(
    operations: &[SchematicPlotOperation],
    metrics: Option<&PlotterTextCacheSession<'_>>,
) -> Result<Option<Bbox>, Error> {
    let mut bbox = None;
    let mut pin_block = false;
    for operation in operations {
        match operation {
            SchematicPlotOperation::StartSymbolPinBlock(_) => {
                pin_block = true;
                continue;
            }
            SchematicPlotOperation::EndBlock => {
                pin_block = false;
                continue;
            }
            SchematicPlotOperation::Text(_) if pin_block => continue,
            _ => {}
        }
        bbox = union_bbox(bbox, operation_bbox(operation, metrics)?);
    }
    Ok(bbox)
}

pub(super) fn operations_bbox(
    operations: &[SchematicPlotOperation],
    metrics: Option<&PlotterTextCacheSession<'_>>,
    include_text: bool,
) -> Result<Option<Bbox>, Error> {
    let mut bbox = None;
    for operation in operations {
        if !include_text && matches!(operation, SchematicPlotOperation::Text(_)) {
            continue;
        }
        bbox = union_bbox(bbox, operation_bbox(operation, metrics)?);
    }
    Ok(bbox)
}

fn operation_bbox(
    operation: &SchematicPlotOperation,
    metrics: Option<&PlotterTextCacheSession<'_>>,
) -> Result<Option<Bbox>, Error> {
    let (bbox, width) = match operation {
        SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(value)) => {
            (bbox_points(&value.points), value.width_nm)
        }
        SchematicPlotOperation::Plotter(PlotterOperation::Rect(value)) => (
            Some(Bbox::new(value.x1, value.y1, value.x2, value.y2)),
            value.width_nm,
        ),
        SchematicPlotOperation::Plotter(PlotterOperation::Circle(value)) => {
            let radius = value.diameter_nm / 2;
            (
                Some(Bbox::new(
                    checked_sub(value.cx, radius)?,
                    checked_sub(value.cy, radius)?,
                    checked_add(value.cx, radius)?,
                    checked_add(value.cy, radius)?,
                )),
                value.width_nm,
            )
        }
        SchematicPlotOperation::Plotter(PlotterOperation::ArcThreePoint(value)) => (
            bbox_points(&[
                [value.start_x, value.start_y],
                [value.mid_x, value.mid_y],
                [value.end_x, value.end_y],
            ]),
            value.width_nm,
        ),
        SchematicPlotOperation::Plotter(PlotterOperation::BezierCurve(value)) => (
            bbox_points(&[
                [value.start_x, value.start_y],
                [value.ctrl1_x, value.ctrl1_y],
                [value.ctrl2_x, value.ctrl2_y],
                [value.end_x, value.end_y],
            ]),
            value.width_nm,
        ),
        SchematicPlotOperation::Text(value) => {
            let style = super::annotation_render::TextStyle {
                color: value.text.color.clone(),
                size_x_nm: value.text.size_x_nm,
                size_y_nm: value.text.size_y_nm,
                h_align: value.text.h_align,
                v_align: value.text.v_align,
                pen_width_nm: value.text.pen_width_nm,
                italic: value.text.italic,
                bold: value.text.bold,
                font_face: value.text.font_face.clone(),
                hyperlink_href: value.hyperlink_href.clone(),
            };
            let (width, _) = measure_text(metrics, &value.text.text, &style)?;
            let horizontal = rounded_angle(value.text.orient_deg) % 180 == 0;
            let (half_x, half_y) = if horizontal {
                (width / 2, value.text.size_y_nm / 2)
            } else {
                (value.text.size_y_nm / 2, width / 2)
            };
            (
                Some(Bbox::new(
                    checked_sub(value.text.x, half_x)?,
                    checked_sub(value.text.y, half_y)?,
                    checked_add(value.text.x, half_x)?,
                    checked_add(value.text.y, half_y)?,
                )),
                // Python's schematic `_op_bbox_nm` uses the Text glyph box
                // directly; Text has `pen_width_nm`, not the generic
                // geometry `width_nm` that participates in bbox inflation.
                0,
            )
        }
        SchematicPlotOperation::StyledThickSegment(value) => (
            Some(Bbox::new(
                value.segment.start_x,
                value.segment.start_y,
                value.segment.end_x,
                value.segment.end_y,
            )),
            value.segment.width_nm,
        ),
        _ => (None, 0),
    };
    bbox.map(|bbox| bbox.inflate(width / 2)).transpose()
}

impl Bbox {
    pub(super) fn new(x1: i64, y1: i64, x2: i64, y2: i64) -> Self {
        Self {
            left: x1.min(x2),
            top: y1.min(y2),
            right: x1.max(x2),
            bottom: y1.max(y2),
        }
    }

    fn inflate(self, amount: i64) -> Result<Self, Error> {
        if amount <= 0 {
            return Ok(self);
        }
        Ok(Self {
            left: checked_sub(self.left, amount)?,
            top: checked_sub(self.top, amount)?,
            right: checked_add(self.right, amount)?,
            bottom: checked_add(self.bottom, amount)?,
        })
    }
}

fn bbox_points(points: &[[i64; 2]]) -> Option<Bbox> {
    let first = points.first()?;
    Some(points.iter().skip(1).fold(
        Bbox::new(first[0], first[1], first[0], first[1]),
        |bbox, point| Bbox {
            left: bbox.left.min(point[0]),
            top: bbox.top.min(point[1]),
            right: bbox.right.max(point[0]),
            bottom: bbox.bottom.max(point[1]),
        },
    ))
}

pub(super) fn union_bbox(left: Option<Bbox>, right: Option<Bbox>) -> Option<Bbox> {
    match (left, right) {
        (None, value) | (value, None) => value,
        (Some(left), Some(right)) => Some(Bbox {
            left: left.left.min(right.left),
            top: left.top.min(right.top),
            right: left.right.max(right.right),
            bottom: left.bottom.max(right.bottom),
        }),
    }
}

fn overlapping_indices(
    entries: &[PrimaryEntry],
    maximum_checks: usize,
) -> Result<Vec<usize>, Error> {
    let mut checks = 0usize;
    let mut overlapping = BTreeSet::new();
    for left in 0..entries.len() {
        let Some(a) = entries[left].overlap_bbox else {
            continue;
        };
        for (right, entry) in entries.iter().enumerate().skip(left + 1) {
            checks = checked_count(checks, 1, maximum_checks)?;
            let Some(b) = entry.overlap_bbox else {
                continue;
            };
            if !(a.right < b.left || b.right < a.left || a.bottom < b.top || b.bottom < a.top) {
                overlapping.insert(left);
                overlapping.insert(right);
            }
        }
    }
    Ok(overlapping.into_iter().collect())
}

fn charge_operations(
    budget: &mut PlotBudget,
    operations: &[SchematicPlotOperation],
) -> Result<(), Error> {
    let points = operations.iter().map(operation_points).sum();
    budget.charge(1, operations.len(), points)
}

fn operation_points(operation: &SchematicPlotOperation) -> usize {
    match operation {
        SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(value)) => value.points.len(),
        SchematicPlotOperation::Plotter(PlotterOperation::ArcThreePoint(_)) => 3,
        SchematicPlotOperation::Plotter(PlotterOperation::BezierCurve(_)) => 4,
        SchematicPlotOperation::Plotter(PlotterOperation::Rect(_))
        | SchematicPlotOperation::Plotter(PlotterOperation::ThickSegment(_))
        | SchematicPlotOperation::StyledThickSegment(_) => 2,
        SchematicPlotOperation::Plotter(PlotterOperation::Circle(_))
        | SchematicPlotOperation::Text(_) => 1,
        _ => 0,
    }
}

fn charge_operation_payload(
    budget: &mut PlotBudget,
    operation: &SchematicPlotOperation,
) -> Result<(), Error> {
    match operation {
        SchematicPlotOperation::Text(value) => {
            budget.charge_text(value.text.text.len())?;
            budget.charge_metadata(
                value
                    .text
                    .color
                    .len()
                    .saturating_add(value.text.font_face.len())
                    .saturating_add(value.hyperlink_href.as_deref().map_or(0, str::len)),
            )?;
        }
        SchematicPlotOperation::Plotter(operation) => {
            budget.charge_metadata(plotter_metadata_bytes(operation))?;
        }
        SchematicPlotOperation::StyledThickSegment(value) => {
            budget.charge_metadata(value.stroke_color.len())?;
        }
        _ => {}
    }
    Ok(())
}

fn plotter_metadata_bytes(operation: &PlotterOperation) -> usize {
    match operation {
        PlotterOperation::ArcThreePoint(value) => {
            optional_colors(&value.stroke_color, &value.fill_color)
        }
        PlotterOperation::Circle(value) => optional_colors(&value.stroke_color, &value.fill_color),
        PlotterOperation::Rect(value) => optional_colors(&value.stroke_color, &value.fill_color),
        PlotterOperation::PlotPoly(value) => {
            optional_colors(&value.stroke_color, &value.fill_color)
        }
        PlotterOperation::BezierCurve(value) => value.stroke_color.as_deref().map_or(0, str::len),
        PlotterOperation::Text(value) => {
            value.text.len() + value.color.len() + value.font_face.len()
        }
        _ => 0,
    }
}

fn optional_colors(left: &Option<String>, right: &Option<String>) -> usize {
    left.as_deref().map_or(0, str::len) + right.as_deref().map_or(0, str::len)
}

fn symbol_metadata_bytes(symbol: &PlacedSymbol, reference: &str) -> Result<usize, Error> {
    [
        symbol.uuid.len(),
        symbol.lib_id.len(),
        symbol.lib_name.len(),
        reference.len(),
        symbol.mirror.as_deref().map_or(0, str::len),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or_else(limit_error)
    })
}

fn block_metadata_bytes(block: &SchematicSymbolPinBlock) -> usize {
    let attrs = &block.extra_attrs;
    block.label.len()
        + block.data_uuid.len()
        + block.object_id.len()
        + attrs.primitive.len()
        + attrs.object_type.len()
        + attrs.pin.len()
        + attrs.symbol_uuid.len()
        + attrs.designator.len()
        + attrs.lib_pin_uuid.len()
        + "symbol_pin".len()
}

fn checked_count(current: usize, additional: usize, maximum: usize) -> Result<usize, Error> {
    current
        .checked_add(additional)
        .filter(|value| *value <= maximum)
        .ok_or_else(limit_error)
}

fn ensure_operation_room(
    current: usize,
    additional: usize,
    budget: &PlotBudget,
) -> Result<(), Error> {
    current
        .checked_add(additional)
        .filter(|value| *value <= budget.remaining_operations())
        .map(|_| ())
        .ok_or_else(limit_error)
}

fn symbol_input_points(symbol: &PlacedSymbol, subsymbols: &[&Sexp]) -> Result<usize, Error> {
    let mut points = symbol.properties.len();
    for subsymbol in subsymbols {
        points = points
            .checked_add(children(subsymbol, "text").count())
            .and_then(|value| value.checked_add(children(subsymbol, "pin").count()))
            .ok_or_else(limit_error)?;
        for (head, default) in [
            ("rectangle", 2usize),
            ("circle", 1),
            ("arc", 3),
            ("polyline", 0),
            ("bezier", 0),
        ] {
            for shape in children(subsymbol, head) {
                let count = if default == 0 {
                    child(shape, "pts").map_or(0, |pts| children(pts, "xy").count())
                } else {
                    default
                };
                points = points.checked_add(count).ok_or_else(limit_error)?;
            }
        }
    }
    Ok(points)
}

fn checked_add(left: i64, right: i64) -> Result<i64, Error> {
    left.checked_add(right)
        .filter(|value| (-JS_SAFE_MAX..=JS_SAFE_MAX).contains(value))
        .ok_or_else(|| model_error("Derived schematic symbol coordinate is unsafe"))
}

fn checked_sub(left: i64, right: i64) -> Result<i64, Error> {
    left.checked_sub(right)
        .filter(|value| (-JS_SAFE_MAX..=JS_SAFE_MAX).contains(value))
        .ok_or_else(|| model_error("Derived schematic symbol coordinate is unsafe"))
}

fn checked_point(x: i64, y: i64) -> Result<[i64; 2], Error> {
    if (-JS_SAFE_MAX..=JS_SAFE_MAX).contains(&x) && (-JS_SAFE_MAX..=JS_SAFE_MAX).contains(&y) {
        Ok([x, y])
    } else {
        Err(model_error("Derived schematic symbol coordinate is unsafe"))
    }
}

fn half_round(value: i64) -> Result<i64, Error> {
    ki_round(value as f64 / 100.0)?
        .div_euclid(2)
        .checked_mul(100)
        .ok_or_else(|| model_error("Schematic symbol half-width overflowed"))
}

fn round_to_100(value: f64) -> Result<i64, Error> {
    if !value.is_finite() {
        return Err(model_error("Schematic symbol metric is invalid"));
    }
    // KiCad's outline-font bbox bridge truncates the scaled metric to the
    // schematic IU grid before converting that integer back to nanometres.
    let rounded = (value / 100.0)
        .trunc()
        .clamp(i64::MIN as f64, i64::MAX as f64) as i64;
    rounded
        .checked_mul(100)
        .and_then(|value| checked_add(value, 0).ok())
        .ok_or_else(|| model_error("Schematic symbol metric is unsafe"))
}

fn ki_round(value: f64) -> Result<i64, Error> {
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(model_error("Schematic symbol rounding is invalid"));
    }
    Ok(if value >= 0.0 {
        (value + 0.5).floor() as i64
    } else {
        (value - 0.5).ceil() as i64
    })
}

fn rotate_screen(x: i64, y: i64, angle: f64) -> Result<[i64; 2], Error> {
    match rounded_angle(angle) {
        0 => Ok([x, y]),
        90 => Ok([y, checked_sub(0, x)?]),
        180 => Ok([checked_sub(0, x)?, checked_sub(0, y)?]),
        270 => Ok([checked_sub(0, y)?, x]),
        _ => {
            let radians = angle.to_radians();
            checked_point(
                ki_round(y as f64 * radians.sin() + x as f64 * radians.cos())?,
                ki_round(y as f64 * radians.cos() - x as f64 * radians.sin())?,
            )
        }
    }
}
