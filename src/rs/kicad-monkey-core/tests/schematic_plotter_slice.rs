use kicad_monkey_contracts::generated::{
    schematic_plot_document::SchematicPlotDocumentA0, shaping_record::ShapingRecordA0,
};
use kicad_monkey_contracts::validate_schematic_plot_document;
use kicad_monkey_core::{
    ErrorKind, PlotterFill, PlotterLineStyle, PlotterOperation, PlotterTextCacheLimits,
    PlotterTextCacheResources, PlotterTextFont, PlotterTextHAlign, PlotterTextVAlign,
    SchematicConnectivityRecordKind, SchematicDrawingSettings, SchematicPlotContext,
    SchematicPlotDocument, SchematicPlotLimits, SchematicPlotOperation, SchematicPlotRecord,
    SchematicPlotVariables, schematic_plot_document, schematic_plot_document_with_annotations,
    schematic_plot_document_with_graphics,
};
use serde_json::{Map, Value, json};

const SOURCE: &str = r#"(kicad_sch
  (version 20240101)
  (generator eeschema)
  (generator_version "10.0")
  (uuid "sch-1")
  (paper "User" 100 80 portrait)
  (title_block (title "${PROJECT}") (rev "A") (comment 1 "C"))
  (wire (pts (xy 1 2) (xy 3 4))
    (stroke (width 0) (type default)) (uuid "w"))
  (bus (pts (xy 5 6) (xy 7 8))
    (stroke (width 0.2) (type dash) (color 1 2 3 0.5)) (uuid "b"))
  (bus_entry (at 7 8) (size 2.54 -2.54)
    (stroke (width -1) (type dot)) (uuid "e"))
  (junction (at 9 10) (diameter 0) (color 10 20 30 0.5) (uuid "j"))
  (no_connect (at 11 12) (uuid "n")))"#;

const WORKSHEET: &str = r#"(kicad_wks
  (version 20210606)
  (generator pl_editor)
  (setup (textsize 1 1) (linewidth 0.15) (textlinewidth 0.15)
    (left_margin 0) (right_margin 0) (top_margin 0) (bottom_margin 0))
  (tbtext "${PROJECT}-${TITLE}-${#}/${##}-${SHEETNAME}"
    (name "") (pos 1 2 ltcorner)))"#;

fn context() -> SchematicPlotContext {
    SchematicPlotContext {
        source_path: Some("foundation.kicad_sch".to_owned()),
        document_id: Some("foundation".to_owned()),
        sheet_index: 2,
        sheet_count: 3,
        sheet_path: "/child".to_owned(),
        sheet_name: "Child".to_owned(),
        project_variables: SchematicPlotVariables::from_entries([
            ("PROJECT", "PX"),
            ("TITLE", "bad"),
        ]),
        worksheet_source: Some(WORKSHEET.as_bytes().to_vec()),
    }
}

fn plotter(operation: &SchematicPlotOperation) -> &PlotterOperation {
    let SchematicPlotOperation::Plotter(operation) = operation else {
        panic!("expected vector operation")
    };
    operation
}

fn insert_optional(object: &mut Map<String, Value>, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        object.insert(name.to_owned(), value);
    }
}

fn fill_name(fill: PlotterFill) -> &'static str {
    match fill {
        PlotterFill::NoFill => "NO_FILL",
        PlotterFill::FilledShape => "FILLED_SHAPE",
        PlotterFill::FilledWithBackgroundBodyColor => "FILLED_WITH_BG_BODYCOLOR",
        PlotterFill::FilledWithColor => "FILLED_WITH_COLOR",
        PlotterFill::Hatch => "HATCH",
        PlotterFill::ReverseHatch => "REVERSE_HATCH",
        PlotterFill::CrossHatch => "CROSS_HATCH",
    }
}

fn line_style_name(style: PlotterLineStyle) -> &'static str {
    match style {
        PlotterLineStyle::Default => "DEFAULT",
        PlotterLineStyle::Solid => "SOLID",
        PlotterLineStyle::Dash => "DASH",
        PlotterLineStyle::Dot => "DOT",
        PlotterLineStyle::DashDot => "DASH_DOT",
        PlotterLineStyle::DashDotDot => "DASH_DOT_DOT",
    }
}

fn h_align_name(align: PlotterTextHAlign) -> &'static str {
    match align {
        PlotterTextHAlign::Left => "GR_TEXT_H_ALIGN_LEFT",
        PlotterTextHAlign::Center => "GR_TEXT_H_ALIGN_CENTER",
        PlotterTextHAlign::Right => "GR_TEXT_H_ALIGN_RIGHT",
    }
}

fn v_align_name(align: PlotterTextVAlign) -> &'static str {
    match align {
        PlotterTextVAlign::Top => "GR_TEXT_V_ALIGN_TOP",
        PlotterTextVAlign::Center => "GR_TEXT_V_ALIGN_CENTER",
        PlotterTextVAlign::Bottom => "GR_TEXT_V_ALIGN_BOTTOM",
    }
}

fn text_json(
    value: &kicad_monkey_core::PlotterText,
    index: usize,
    hyperlink: Option<&str>,
) -> Value {
    let mut object = json!({
        "kind": "Text", "index": index, "x": value.x, "y": value.y,
        "text": value.text, "color": value.color,
        "orient_deg": value.orient_deg,
        "size_x_nm": value.size_x_nm, "size_y_nm": value.size_y_nm,
        "h_align": h_align_name(value.h_align),
        "v_align": v_align_name(value.v_align),
        "pen_width_nm": value.pen_width_nm,
        "italic": value.italic, "bold": value.bold,
        "multiline": value.multiline, "font_face": value.font_face,
    })
    .as_object()
    .expect("text object")
    .clone();
    insert_optional(&mut object, "layer", value.layer.as_ref().map(|v| json!(v)));
    insert_optional(
        &mut object,
        "context",
        hyperlink.map(|href| json!({"hyperlink": {"href": href}})),
    );
    Value::Object(object)
}

fn operation_json(operation: &SchematicPlotOperation, index: usize) -> Value {
    match operation {
        SchematicPlotOperation::Text(value) => {
            text_json(&value.text, index, value.hyperlink_href.as_deref())
        }
        SchematicPlotOperation::StyledThickSegment(value) => {
            let segment = &value.segment;
            json!({
                "kind": "ThickSegment", "index": index,
                "start_x": segment.start_x, "start_y": segment.start_y,
                "end_x": segment.end_x, "end_y": segment.end_y,
                "width_nm": segment.width_nm,
                "stroke_color": value.stroke_color,
            })
        }
        SchematicPlotOperation::PlotImage(value) => {
            let mut object = json!({
                "kind": "PlotImage", "index": index,
                "x": value.x, "y": value.y,
                "width_nm": value.width_nm, "height_nm": value.height_nm,
                "scale": value.scale, "image_data_b64": value.image_data_b64,
                "image_format": value.image_format,
            })
            .as_object()
            .expect("image object")
            .clone();
            insert_optional(
                &mut object,
                "stroke_color",
                value.stroke_color.as_ref().map(|value| json!(value)),
            );
            Value::Object(object)
        }
        SchematicPlotOperation::Plotter(operation) => match operation {
            PlotterOperation::Rect(value) => {
                let mut object = json!({
                    "kind": "Rect", "index": index,
                    "x1": value.x1, "y1": value.y1, "x2": value.x2, "y2": value.y2,
                    "fill": fill_name(value.fill), "width_nm": value.width_nm,
                    "corner_radius_nm": value.corner_radius_nm,
                })
                .as_object()
                .expect("rect object")
                .clone();
                insert_optional(&mut object, "layer", value.layer.as_ref().map(|v| json!(v)));
                insert_optional(
                    &mut object,
                    "stroke_color",
                    value.stroke_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "fill_color",
                    value.fill_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "line_style",
                    value.line_style.map(|v| json!(line_style_name(v))),
                );
                Value::Object(object)
            }
            PlotterOperation::PlotPoly(value) => {
                let mut object = json!({
                    "kind": "PlotPoly", "index": index, "points": value.points,
                    "fill": fill_name(value.fill), "width_nm": value.width_nm,
                })
                .as_object()
                .expect("poly object")
                .clone();
                insert_optional(&mut object, "layer", value.layer.as_ref().map(|v| json!(v)));
                insert_optional(
                    &mut object,
                    "stroke_color",
                    value.stroke_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "fill_color",
                    value.fill_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "line_style",
                    value.line_style.map(|v| json!(line_style_name(v))),
                );
                Value::Object(object)
            }
            PlotterOperation::Circle(value) => {
                let mut object = json!({
                    "kind": "Circle", "index": index,
                    "cx": value.cx, "cy": value.cy, "diameter_nm": value.diameter_nm,
                    "fill": fill_name(value.fill), "width_nm": value.width_nm,
                })
                .as_object()
                .expect("circle object")
                .clone();
                insert_optional(&mut object, "layer", value.layer.as_ref().map(|v| json!(v)));
                insert_optional(&mut object, "role", value.role.as_ref().map(|v| json!(v)));
                if !value.layers.is_empty() {
                    object.insert("layers".to_owned(), json!(value.layers));
                }
                insert_optional(
                    &mut object,
                    "mask_margin_nm",
                    value.mask_margin_nm.map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "pad_size_x_nm",
                    value.pad_size_x_nm.map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "pad_size_y_nm",
                    value.pad_size_y_nm.map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "stroke_color",
                    value.stroke_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "fill_color",
                    value.fill_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "line_style",
                    value.line_style.map(|v| json!(line_style_name(v))),
                );
                Value::Object(object)
            }
            PlotterOperation::Text(value) => text_json(value, index, None),
            PlotterOperation::ArcThreePoint(value) => {
                let mut object = json!({
                    "kind": "ArcThreePoint", "index": index,
                    "start_x": value.start_x, "start_y": value.start_y,
                    "mid_x": value.mid_x, "mid_y": value.mid_y,
                    "end_x": value.end_x, "end_y": value.end_y,
                    "fill": fill_name(value.fill), "width_nm": value.width_nm,
                })
                .as_object()
                .expect("arc object")
                .clone();
                insert_optional(&mut object, "layer", value.layer.as_ref().map(|v| json!(v)));
                insert_optional(
                    &mut object,
                    "stroke_color",
                    value.stroke_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "fill_color",
                    value.fill_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "line_style",
                    value.line_style.map(|v| json!(line_style_name(v))),
                );
                Value::Object(object)
            }
            PlotterOperation::BezierCurve(value) => {
                let mut object = json!({
                    "kind": "BezierCurve", "index": index,
                    "start_x": value.start_x, "start_y": value.start_y,
                    "ctrl1_x": value.ctrl1_x, "ctrl1_y": value.ctrl1_y,
                    "ctrl2_x": value.ctrl2_x, "ctrl2_y": value.ctrl2_y,
                    "end_x": value.end_x, "end_y": value.end_y,
                    "width_nm": value.width_nm, "tolerance_nm": value.tolerance_nm,
                })
                .as_object()
                .expect("bezier object")
                .clone();
                insert_optional(&mut object, "layer", value.layer.as_ref().map(|v| json!(v)));
                insert_optional(
                    &mut object,
                    "stroke_color",
                    value.stroke_color.as_ref().map(|v| json!(v)),
                );
                insert_optional(
                    &mut object,
                    "line_style",
                    value.line_style.map(|v| json!(line_style_name(v))),
                );
                Value::Object(object)
            }
            PlotterOperation::ThickSegment(_)
            | PlotterOperation::FlashPadCircle(_)
            | PlotterOperation::FlashPadOval(_)
            | PlotterOperation::FlashPadRect(_)
            | PlotterOperation::FlashPadRoundRect(_)
            | PlotterOperation::FlashPadCustom(_)
            | PlotterOperation::FlashPadTrapez(_) => {
                panic!("operation outside the P5_060 schematic vocabulary")
            }
        },
    }
}

fn document_json(document: &SchematicPlotDocument) -> Value {
    let records = document
        .records
        .iter()
        .map(|record| match record {
            SchematicPlotRecord::SheetHeader(value) => {
                let mut object = json!({
                    "uuid": value.uuid, "kind": "sheet_header", "object_id": value.uuid,
                    "operation_count": value.operations.len(),
                    "operations": value.operations.iter().enumerate().map(|(index, value)| operation_json(value, index)).collect::<Vec<_>>(),
                    "paper_size": value.paper_size,
                    "paper_width_mm": value.paper_width_mm,
                    "paper_height_mm": value.paper_height_mm,
                    "paper_portrait": value.paper_portrait,
                    "sheet_width_nm": value.sheet_width_nm,
                    "sheet_height_nm": value.sheet_height_nm,
                    "version": value.version, "generator": value.generator,
                    "generator_version": value.generator_version,
                })
                .as_object()
                .expect("header object")
                .clone();
                insert_optional(
                    &mut object,
                    "title_block",
                    value.title_block.as_ref().map(|title| {
                        json!({
                            "title": title.title, "date": title.date,
                            "rev": title.revision, "company": title.company,
                            "comments": title.comments,
                        })
                    }),
                );
                Value::Object(object)
            }
            SchematicPlotRecord::Connectivity(value) => {
                let mut object = json!({
                    "uuid": value.uuid, "kind": value.kind.as_str(), "object_id": value.uuid,
                    "operation_count": value.operations.len(),
                    "operations": value.operations.iter().enumerate().map(|(index, value)| operation_json(value, index)).collect::<Vec<_>>(),
                })
                .as_object()
                .expect("connectivity object")
                .clone();
                if value.kind == SchematicConnectivityRecordKind::Junction
                    && value.junction_color_authored
                {
                    object.insert(
                        "color".to_owned(),
                        value
                            .junction_color
                            .as_ref()
                            .map_or(Value::Null, |value| json!(value)),
                    );
                }
                Value::Object(object)
            }
            SchematicPlotRecord::Annotation(value) => {
                let mut object = json!({
                    "uuid": value.uuid, "kind": value.kind.as_str(),
                    "object_id": value.object_id,
                    "operation_count": value.operations.len(),
                    "operations": value.operations.iter().enumerate()
                        .map(|(index, value)| operation_json(value, index)).collect::<Vec<_>>(),
                })
                .as_object()
                .expect("annotation object")
                .clone();
                insert_optional(&mut object, "text", value.text.as_ref().map(|v| json!(v)));
                insert_optional(&mut object, "shape", value.shape.as_ref().map(|v| json!(v)));
                insert_optional(&mut object, "at_x_nm", value.at_x_nm.map(|v| json!(v)));
                insert_optional(&mut object, "at_y_nm", value.at_y_nm.map(|v| json!(v)));
                insert_optional(&mut object, "length_nm", value.length_nm.map(|v| json!(v)));
                Value::Object(object)
            }
            SchematicPlotRecord::Graphic(value) => json!({
                "uuid": value.uuid, "kind": value.kind.as_str(), "object_id": value.uuid,
                "operation_count": value.operations.len(),
                "operations": value.operations.iter().enumerate()
                    .map(|(index, value)| operation_json(value, index)).collect::<Vec<_>>(),
            }),
            SchematicPlotRecord::RuleArea(value) => json!({
                "uuid": value.uuid, "kind": "rule_area", "object_id": value.uuid,
                "operation_count": value.operations.len(),
                "operations": value.operations.iter().enumerate()
                    .map(|(index, value)| operation_json(value, index)).collect::<Vec<_>>(),
                "shape": value.shape.as_str(), "locked": value.locked,
                "exclude_from_sim": value.exclude_from_sim, "in_bom": value.in_bom,
                "on_board": value.on_board, "dnp": value.dnp,
            }),
            SchematicPlotRecord::Image(value) => json!({
                "uuid": value.uuid, "kind": "image", "object_id": value.uuid,
                "operation_count": value.operations.len(),
                "operations": value.operations.iter().enumerate()
                    .map(|(index, value)| operation_json(value, index)).collect::<Vec<_>>(),
                "scale": value.scale, "image_format": value.image_format,
                "width_nm": value.width_nm, "height_nm": value.height_nm,
            }),
            SchematicPlotRecord::Table(value) => json!({
                "uuid": value.uuid, "kind": "table", "object_id": value.uuid,
                "operation_count": value.operations.len(),
                "operations": value.operations.iter().enumerate()
                    .map(|(index, value)| operation_json(value, index)).collect::<Vec<_>>(),
                "cell_count": value.cell_count,
            }),
        })
        .collect::<Vec<_>>();
    let mut object = json!({
        "schema": "kicad.plotter_ir.a0", "source_kind": "SCH",
        "total_operations": document.records.iter().map(SchematicPlotRecord::operation_count).sum::<usize>(),
        "records": records, "document_id": document.document_id,
        "canvas": {"width_nm": document.canvas.width_nm, "height_nm": document.canvas.height_nm},
        "coordinate_space": {"unit": "nm", "y_axis": "down"},
    })
    .as_object()
    .expect("document object")
    .clone();
    insert_optional(
        &mut object,
        "source_path",
        document.source_path.as_ref().map(|value| json!(value)),
    );
    Value::Object(object)
}

fn vector_context(vector: &Value) -> SchematicPlotContext {
    let variables = vector
        .get("project_variables")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|values| values.iter())
        .map(|(name, value)| {
            (
                name.clone(),
                value.as_str().expect("variable string").to_owned(),
            )
        });
    SchematicPlotContext {
        source_path: vector
            .get("source_path")
            .and_then(Value::as_str)
            .map(str::to_owned),
        document_id: vector
            .get("document_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        sheet_index: vector
            .get("sheet_index")
            .and_then(Value::as_u64)
            .unwrap_or(1) as usize,
        sheet_count: vector
            .get("sheet_count")
            .and_then(Value::as_u64)
            .unwrap_or(1) as usize,
        sheet_path: vector
            .get("sheet_path")
            .and_then(Value::as_str)
            .unwrap_or("/")
            .to_owned(),
        sheet_name: vector
            .get("sheet_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        project_variables: SchematicPlotVariables::from_entries(variables),
        worksheet_source: vector
            .get("worksheet_source")
            .and_then(Value::as_str)
            .map(|value| value.as_bytes().to_vec()),
    }
}

fn vector_drawing_settings(vector: &Value) -> SchematicDrawingSettings {
    let values = vector.get("drawing_settings");
    SchematicDrawingSettings {
        text_offset_ratio: values
            .and_then(|value| value.get("text_offset_ratio"))
            .and_then(Value::as_f64)
            .unwrap_or(0.15),
        default_line_width_nm: values
            .and_then(|value| value.get("default_line_thickness"))
            .and_then(Value::as_f64)
            .map(|mils| (mils * 25_400.0).round_ties_even() as i64)
            .unwrap_or(152_400),
    }
}

const METRIC_FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/parity/fonts/shaping-variable-fixture.ttf"
));

fn metric_font() -> PlotterTextFont<'static> {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/font_shaping_a0_vectors.json"
    )))
    .expect("shaping vectors");
    let record: ShapingRecordA0 = vectors["records"]
        .as_array()
        .expect("shaping records")
        .iter()
        .find(|record| record["case_id"] == "fixture_default_variation_axis")
        .cloned()
        .map(serde_json::from_value)
        .expect("metric shaping case")
        .expect("metric shaping record");
    PlotterTextFont {
        face: "KiCad Monkey Shaping Fixture",
        bold: false,
        italic: false,
        font_bytes: METRIC_FONT_BYTES,
        shaping: record.input,
        fake_bold: false,
        fake_italic: false,
    }
}

#[test]
fn custom_worksheet_and_connectivity_match_python_foundation() {
    let document = schematic_plot_document(SOURCE, SchematicPlotLimits::default(), &context())
        .expect("schematic plot");
    assert_eq!(
        document.source_path.as_deref(),
        Some("foundation.kicad_sch")
    );
    assert_eq!(document.document_id, "foundation");
    assert_eq!(
        (document.canvas.width_nm, document.canvas.height_nm),
        (80_000_000, 100_000_000)
    );
    assert_eq!(document.records.len(), 6);
    assert_eq!(
        document
            .records
            .iter()
            .map(SchematicPlotRecord::operation_count)
            .sum::<usize>(),
        8
    );

    let SchematicPlotRecord::SheetHeader(header) = &document.records[0] else {
        panic!("header")
    };
    assert_eq!(header.uuid, "sch-1");
    assert_eq!(header.paper_size, "User");
    assert_eq!(header.operations.len(), 2);
    let PlotterOperation::Rect(background) = plotter(&header.operations[0]) else {
        panic!("background")
    };
    assert_eq!((background.x2, background.y2), (80_000_000, 100_000_000));
    assert_eq!(background.fill_color.as_deref(), Some("#F5F4EFFF"));
    let PlotterOperation::Text(text) = plotter(&header.operations[1]) else {
        panic!("worksheet text")
    };
    assert_eq!(text.text, "PX-PX-2/3-Child");
    assert_eq!((text.x, text.y), (1_000_000, 2_000_000));
    assert_eq!(text.font_face, "Arial");

    let kinds = document.records[1..]
        .iter()
        .map(|record| match record {
            SchematicPlotRecord::Connectivity(record) => record.kind,
            SchematicPlotRecord::SheetHeader(_) => panic!("second header"),
            SchematicPlotRecord::Annotation(_) => panic!("unexpected annotation"),
            _ => panic!("unexpected P5_062 record in foundation entry"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            SchematicConnectivityRecordKind::Wire,
            SchematicConnectivityRecordKind::Bus,
            SchematicConnectivityRecordKind::BusEntry,
            SchematicConnectivityRecordKind::Junction,
            SchematicConnectivityRecordKind::NoConnect,
        ]
    );

    let SchematicPlotRecord::Connectivity(wire) = &document.records[1] else {
        unreachable!()
    };
    let PlotterOperation::PlotPoly(wire_poly) = plotter(&wire.operations[0]) else {
        panic!("wire")
    };
    assert_eq!(
        wire_poly.points,
        [[1_000_000, 2_000_000], [3_000_000, 4_000_000]]
    );
    assert_eq!(wire_poly.width_nm, 152_400);
    assert_eq!(wire_poly.stroke_color.as_deref(), Some("#009600FF"));
    assert_eq!(wire_poly.line_style, Some(PlotterLineStyle::Default));

    let SchematicPlotRecord::Connectivity(bus) = &document.records[2] else {
        unreachable!()
    };
    let PlotterOperation::PlotPoly(bus_poly) = plotter(&bus.operations[0]) else {
        panic!("bus")
    };
    assert_eq!(bus_poly.width_nm, 200_000);
    assert_eq!(bus_poly.stroke_color.as_deref(), Some("#01020380"));
    assert_eq!(bus_poly.line_style, Some(PlotterLineStyle::Dash));

    let SchematicPlotRecord::Connectivity(entry) = &document.records[3] else {
        unreachable!()
    };
    let PlotterOperation::PlotPoly(entry_poly) = plotter(&entry.operations[0]) else {
        panic!("entry")
    };
    assert_eq!(
        entry_poly.points,
        [[7_000_000, 8_000_000], [9_540_000, 5_460_000]]
    );
    assert_eq!(entry_poly.width_nm, 0);

    let SchematicPlotRecord::Connectivity(junction) = &document.records[4] else {
        unreachable!()
    };
    assert!(junction.junction_color_authored);
    assert_eq!(junction.junction_color.as_deref(), Some("#0A141E80"));
    let PlotterOperation::Circle(circle) = plotter(&junction.operations[0]) else {
        panic!("junction")
    };
    assert_eq!(
        (circle.cx, circle.cy, circle.diameter_nm),
        (9_000_000, 10_000_000, 914_400)
    );

    let SchematicPlotRecord::Connectivity(no_connect) = &document.records[5] else {
        unreachable!()
    };
    assert_eq!(no_connect.operations.len(), 2);
    let PlotterOperation::PlotPoly(first) = plotter(&no_connect.operations[0]) else {
        panic!("no connect")
    };
    assert_eq!(first.width_nm, 152_400);
    assert_eq!(
        first.points,
        [[10_390_400, 11_390_400], [11_609_600, 12_609_600]]
    );
}

#[test]
fn default_worksheet_and_document_id_fallback_are_stable() {
    let context = SchematicPlotContext::default();
    let document = schematic_plot_document(
        "(kicad_sch (uuid source-uuid) (paper A4))",
        SchematicPlotLimits::default(),
        &context,
    )
    .expect("default worksheet");
    assert_eq!(document.document_id, "source-uuid");
    let SchematicPlotRecord::SheetHeader(header) = &document.records[0] else {
        unreachable!()
    };
    assert_eq!(header.operations.len(), 59);
}

#[test]
fn independent_family_input_and_worksheet_limits_fail_closed() {
    let cases = [
        SchematicPlotLimits {
            max_wires: 0,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_buses: 0,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_bus_entries: 0,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_junctions: 0,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_no_connects: 0,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_input_points: 7,
            ..SchematicPlotLimits::default()
        },
    ];
    for limits in cases {
        assert_eq!(
            schematic_plot_document(SOURCE, limits, &context())
                .expect_err("resource limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }

    let repeat_context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks (line (start 0 0) (end 1 1) (repeat 2)))".to_vec()),
        ..SchematicPlotContext::default()
    };
    assert_eq!(
        schematic_plot_document(
            "(kicad_sch)",
            SchematicPlotLimits {
                max_worksheet_repeats: 1,
                ..SchematicPlotLimits::default()
            },
            &repeat_context,
        )
        .expect_err("worksheet repeat")
        .kind,
        ErrorKind::ResourceLimit
    );

    let polygon_context = SchematicPlotContext {
        worksheet_source: Some(
            b"(kicad_wks (polygon (pos 0 0) (pts (xy 0 0)) (pts (xy 1 1))))".to_vec(),
        ),
        ..SchematicPlotContext::default()
    };
    assert_eq!(
        schematic_plot_document(
            "(kicad_sch)",
            SchematicPlotLimits {
                max_worksheet_point_sets: 1,
                ..SchematicPlotLimits::default()
            },
            &polygon_context,
        )
        .expect_err("worksheet point sets")
        .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn aggregate_parse_node_limits_cover_unselected_source_and_worksheet_forms() {
    let source = "(kicad_sch (unknown (a 1) (b 2) (c 3) (d 4)))";
    let limits = SchematicPlotLimits {
        max_parse_nodes: 8,
        ..SchematicPlotLimits::default()
    };
    assert_eq!(
        schematic_plot_document(source, limits, &SchematicPlotContext::default())
            .expect_err("source aggregate nodes")
            .kind,
        ErrorKind::ResourceLimit
    );

    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks (unknown (a 1) (b 2) (c 3) (d 4)))".to_vec()),
        ..SchematicPlotContext::default()
    };
    assert_eq!(
        schematic_plot_document("(kicad_sch)", limits, &context)
            .expect_err("worksheet aggregate nodes")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn source_and_worksheet_authority_edges_are_preserved() {
    let source = r#"(kicad_sch
      (uuid edge)
      (paper User portrait 12 34)
      (title_block (title "") (title "later"))
      (wire (pts (xy 1) (xy 1.000001 2)) (uuid w)))"#;
    let context = SchematicPlotContext {
        project_variables: SchematicPlotVariables::from_entries([("PROJECT", "PX")]),
        worksheet_source: Some(
            "(kicad_wks (setup (left_margin 0) (right_margin 0) (top_margin 0) (bottom_margin 0)) (tbtext \"${}|${ PROJECT }|${VARIANT}|${ UNKNOWN }|x\\n\" (pos 0 0 ltcorner)))"
                .as_bytes()
                .to_vec(),
        ),
        ..SchematicPlotContext::default()
    };
    let document = schematic_plot_document(source, SchematicPlotLimits::default(), &context)
        .expect("authority edges");
    let SchematicPlotRecord::SheetHeader(header) = &document.records[0] else {
        unreachable!()
    };
    assert_eq!(header.paper_width_mm, None);
    assert_eq!(header.paper_height_mm, Some(12.0));
    assert_eq!(header.title_block.as_ref().expect("title").title, "");
    let PlotterOperation::Text(text) = plotter(&header.operations[1]) else {
        panic!("text")
    };
    assert_eq!(text.text, "${}|PX||${ UNKNOWN }|x");
    let SchematicPlotRecord::Connectivity(wire) = &document.records[1] else {
        unreachable!()
    };
    let PlotterOperation::PlotPoly(poly) = plotter(&wire.operations[0]) else {
        panic!("wire")
    };
    assert_eq!(poly.points, [[1_000_001, 2_000_000]]);
}

#[test]
fn duplicated_document_and_repeated_payload_bytes_are_bounded() {
    let metadata_limited = SchematicPlotLimits {
        max_metadata_bytes: 19,
        ..SchematicPlotLimits::default()
    };
    assert_eq!(
        schematic_plot_document(
            "(kicad_sch (uuid abc) (paper A4))",
            metadata_limited,
            &SchematicPlotContext {
                worksheet_source: Some(b"(kicad_wks)".to_vec()),
                ..SchematicPlotContext::default()
            },
        )
        .expect_err("uuid retained as header and document id")
        .kind,
        ErrorKind::ResourceLimit
    );

    let repeated_text = SchematicPlotContext {
        worksheet_source: Some(
            b"(kicad_wks (setup (left_margin 0) (right_margin 0) (top_margin 0) (bottom_margin 0)) (tbtext x (pos 1 1 ltcorner) (font (face Arial)) (repeat 2) (incrx 1)))".to_vec(),
        ),
        ..SchematicPlotContext::default()
    };
    assert_eq!(
        schematic_plot_document(
            "(kicad_sch)",
            SchematicPlotLimits {
                max_metadata_bytes: 19,
                ..SchematicPlotLimits::default()
            },
            &repeated_text,
        )
        .expect_err("repeated font face clones")
        .kind,
        ErrorKind::ResourceLimit
    );

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let repeated_bitmap = SchematicPlotContext {
        worksheet_source: Some(
            format!("(kicad_wks (setup (left_margin 0) (right_margin 0) (top_margin 0) (bottom_margin 0)) (bitmap (pos 1 1 ltcorner) (data \"{PNG}\") (repeat 2) (incrx 1)))")
                .into_bytes(),
        ),
        ..SchematicPlotContext::default()
    };
    assert_eq!(
        schematic_plot_document(
            "(kicad_sch)",
            SchematicPlotLimits {
                max_metadata_bytes: 14 + PNG.len(),
                ..SchematicPlotLimits::default()
            },
            &repeated_bitmap,
        )
        .expect_err("repeated bitmap payload clones")
        .kind,
        ErrorKind::ResourceLimit
    );

    let two_bitmaps = SchematicPlotContext {
        worksheet_source: Some(
            format!(
                "(kicad_wks (bitmap (pos 1 1) (data \"{PNG}\")) (bitmap (pos 2 2) (data \"{PNG}\")))"
            )
            .into_bytes(),
        ),
        ..SchematicPlotContext::default()
    };
    assert_eq!(
        schematic_plot_document(
            "(kicad_sch)",
            SchematicPlotLimits {
                max_worksheet_bitmap_decode_work: 439,
                ..SchematicPlotLimits::default()
            },
            &two_bitmaps,
        )
        .expect_err("aggregate bitmap metadata scan work")
        .kind,
        ErrorKind::ResourceLimit
    );

    let zero_scale = SchematicPlotContext {
        worksheet_source: Some(
            format!("(kicad_wks (bitmap (pos 1 1) (scale 0) (data \"{PNG}\")))").into_bytes(),
        ),
        ..SchematicPlotContext::default()
    };
    assert!(
        schematic_plot_document("(kicad_sch)", SchematicPlotLimits::default(), &zero_scale)
            .is_err()
    );
}

#[test]
fn builtins_decode_literal_newlines_and_self_reference_falls_back_to_project() {
    let source = r#"(kicad_sch
      (title_block (title "${TITLE}") (company "A\\nB")))"#;
    let context = SchematicPlotContext {
        project_variables: SchematicPlotVariables::from_entries([("TITLE", "PV")]),
        worksheet_source: Some(
            b"(kicad_wks (setup (left_margin 0) (right_margin 0) (top_margin 0) (bottom_margin 0)) (tbtext \"${COMPANY}|${TITLE}\" (pos 0 0 ltcorner)))".to_vec(),
        ),
        ..SchematicPlotContext::default()
    };
    let document = schematic_plot_document(source, SchematicPlotLimits::default(), &context)
        .expect("builtin expansion");
    let SchematicPlotRecord::SheetHeader(header) = &document.records[0] else {
        unreachable!()
    };
    let PlotterOperation::Text(text) = plotter(&header.operations[1]) else {
        panic!("text")
    };
    assert_eq!(text.text, "A\nB|PV");
    assert!(text.multiline);
}

#[test]
fn every_shared_vector_is_exactly_projectable_to_the_strict_contract() {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/schematic_plotter_a0_vectors.json"
    )))
    .expect("shared vectors");
    assert_eq!(
        vectors["schema"],
        "kicad_monkey.schematic_plotter_parity.a0"
    );
    for vector in vectors["vectors"].as_array().expect("vector array") {
        let source = vector["source"].as_str().expect("source");
        let context = vector_context(vector);
        let settings = vector_drawing_settings(vector);
        let document = if vector.get("font_resource").is_some() {
            let fonts = [metric_font()];
            let resources = PlotterTextCacheResources {
                fonts: &fonts,
                limits: PlotterTextCacheLimits::default(),
            };
            schematic_plot_document_with_graphics(
                source,
                SchematicPlotLimits::default(),
                &context,
                settings,
                Some(&resources),
            )
        } else {
            schematic_plot_document_with_graphics(
                source,
                SchematicPlotLimits::default(),
                &context,
                settings,
                None,
            )
        }
        .unwrap_or_else(|error| panic!("{}: {error}", vector["id"]));
        let actual = document_json(&document);
        assert_eq!(actual, vector["expected"], "{}", vector["id"]);
        let contract: SchematicPlotDocumentA0 =
            serde_json::from_value(actual).expect("generated contract decode");
        validate_schematic_plot_document(&contract).expect("strict semantic validation");
    }
}

fn assert_resource_pair(
    source: &str,
    context: &SchematicPlotContext,
    exact: SchematicPlotLimits,
    one_over: SchematicPlotLimits,
) {
    schematic_plot_document(source, exact, context).expect("exact resource boundary");
    assert_eq!(
        schematic_plot_document(source, one_over, context)
            .expect_err("one-over resource boundary")
            .kind,
        ErrorKind::ResourceLimit
    );
}

fn assert_annotation_resource_pair(
    source: &str,
    context: &SchematicPlotContext,
    exact: SchematicPlotLimits,
    one_over: SchematicPlotLimits,
) {
    schematic_plot_document_with_annotations(
        source,
        exact,
        context,
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("exact annotation resource boundary");
    assert_eq!(
        schematic_plot_document_with_annotations(
            source,
            one_over,
            context,
            SchematicDrawingSettings::default(),
            None,
        )
        .expect_err("one-over annotation resource boundary")
        .kind,
        ErrorKind::ResourceLimit
    );
}

fn assert_graphics_resource_pair(
    source: &str,
    context: &SchematicPlotContext,
    exact: SchematicPlotLimits,
    one_under: SchematicPlotLimits,
) {
    schematic_plot_document_with_graphics(
        source,
        exact,
        context,
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("exact graphics resource boundary");
    assert_eq!(
        schematic_plot_document_with_graphics(
            source,
            one_under,
            context,
            SchematicDrawingSettings::default(),
            None,
        )
        .expect_err("one-under graphics resource boundary")
        .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn earlier_schematic_entries_exclude_p5_062_carriers() {
    let source = r#"(kicad_sch
      (polyline (pts (xy 0 0) (xy 1 1)) (uuid "p"))
      (table (uuid "t") (cells (table_cell "x" (at 0 0) (size 0 0)))))"#;
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let foundation = schematic_plot_document(source, SchematicPlotLimits::default(), &context)
        .expect("foundation scope");
    let annotations = schematic_plot_document_with_annotations(
        source,
        SchematicPlotLimits::default(),
        &context,
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("annotation scope");
    let graphics = schematic_plot_document_with_graphics(
        source,
        SchematicPlotLimits::default(),
        &context,
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("graphics scope");
    assert_eq!(foundation.records.len(), 1);
    assert_eq!(annotations.records.len(), 1);
    assert_eq!(graphics.records.len(), 3);
}

#[test]
fn graphics_family_table_and_image_ceilings_are_independent() {
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let families: [(&str, fn(&mut SchematicPlotLimits, usize)); 8] = [
        (
            "(kicad_sch (polyline (pts (xy 0 0) (xy 1 1))))",
            |limits: &mut SchematicPlotLimits, value| limits.max_polylines = value,
        ),
        (
            "(kicad_sch (arc (start 0 0) (mid 1 1) (end 2 0)))",
            |limits: &mut SchematicPlotLimits, value| limits.max_arcs = value,
        ),
        (
            "(kicad_sch (circle (center 0 0) (radius 1)))",
            |limits: &mut SchematicPlotLimits, value| limits.max_circles = value,
        ),
        (
            "(kicad_sch (rectangle (start 0 0) (end 1 1)))",
            |limits: &mut SchematicPlotLimits, value| limits.max_rectangles = value,
        ),
        (
            "(kicad_sch (bezier (pts (xy 0 0) (xy 1 0) (xy 1 1) (xy 2 1))))",
            |limits: &mut SchematicPlotLimits, value| limits.max_beziers = value,
        ),
        (
            "(kicad_sch (rule_area (polyline (pts (xy 0 0) (xy 1 1)))))",
            |limits: &mut SchematicPlotLimits, value| limits.max_rule_areas = value,
        ),
        (
            "(kicad_sch (image (data \"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=\")))",
            |limits: &mut SchematicPlotLimits, value| limits.max_images = value,
        ),
        (
            "(kicad_sch (table (cells)))",
            |limits: &mut SchematicPlotLimits, value| limits.max_tables = value,
        ),
    ];
    for (source, set) in families {
        let mut exact = SchematicPlotLimits::default();
        let mut one_under = exact;
        set(&mut exact, 1);
        set(&mut one_under, 0);
        assert_graphics_resource_pair(source, &context, exact, one_under);
    }

    let cell = "(kicad_sch (table (cells (table_cell \"x\" (at 0 0) (size 0 0)))))";
    for (exact, one_under) in [
        (
            SchematicPlotLimits {
                max_table_cells: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_table_cells: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_table_cell_lines: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_table_cell_lines: 0,
                ..SchematicPlotLimits::default()
            },
        ),
    ] {
        assert_graphics_resource_pair(cell, &context, exact, one_under);
    }

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let image = format!("(kicad_sch (image (data \"{PNG}\")))");
    for (exact, one_under) in [
        (
            SchematicPlotLimits {
                max_image_data_parts: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_image_data_parts: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_image_encoded_bytes: PNG.len(),
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_image_encoded_bytes: PNG.len() - 1,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_image_decoded_bytes: 68,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_image_decoded_bytes: 67,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_image_width_px: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_image_width_px: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_image_height_px: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_image_height_px: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_image_pixels: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_image_pixels: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_image_decode_work: PNG.len() + 68 + 60,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_image_decode_work: PNG.len() + 68 + 59,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            // Header strings (14), one retained encoded image (92), two
            // retained format strings (6), and the image stroke color (9).
            SchematicPlotLimits {
                max_metadata_bytes: 14 + PNG.len() + 6 + 9,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_metadata_bytes: 14 + PNG.len() + 6 + 8,
                ..SchematicPlotLimits::default()
            },
        ),
    ] {
        assert_graphics_resource_pair(&image, &context, exact, one_under);
    }
}

#[test]
fn malformed_graphics_and_images_fail_before_publication() {
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    for source in [
        "(kicad_sch (circle (center 0 0) (radius -1)))",
        "(kicad_sch (rectangle (start 0 0) (end 1 1) (radius -1)))",
        "(kicad_sch (image (scale 0) (data \"AAAA\")))",
        "(kicad_sch (image (scale -1) (data \"AAAA\")))",
        "(kicad_sch (image (data \"AA=A\")))",
        "(kicad_sch (image (data \"AAAA\")))",
        // Repeated 0xFF marker prefixes are not collapsed by the Python
        // authority. Treating this as SOF0 would publish invented dimensions.
        "(kicad_sch (image (data \"/9j//8AABwgAAQAB\")))",
        // A complete PNG terminates at IEND. Chunks appended after IEND must
        // not be silently ignored by the native metadata projection.
        "(kicad_sch (image (data \"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYIIAAAAJcEhZcwAAAAEAAAABAQAAAAA=\")))",
    ] {
        assert!(
            schematic_plot_document_with_graphics(
                source,
                SchematicPlotLimits::default(),
                &context,
                SchematicDrawingSettings::default(),
                None,
            )
            .is_err(),
            "{source}"
        );
    }
}

#[test]
fn graphics_and_table_retained_budgets_are_exact() {
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let polyline = "(kicad_sch (polyline (pts (xy 0 0) (xy 1 1))))";
    for (exact, one_under) in [
        (
            SchematicPlotLimits {
                max_input_points: 2,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_input_points: 1,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_points: 2,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_points: 1,
                ..SchematicPlotLimits::default()
            },
        ),
    ] {
        assert_graphics_resource_pair(polyline, &context, exact, one_under);
    }

    let split = r#"(kicad_sch
      (rectangle (start 0 0) (end 1 1)
        (fill (type color) (color 1 2 3 1))))"#;
    assert_graphics_resource_pair(
        split,
        &context,
        SchematicPlotLimits {
            max_operations: 3,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_operations: 2,
            ..SchematicPlotLimits::default()
        },
    );

    let table = r#"(kicad_sch
      (table (uuid "u") (cells
        (table_cell "A" (at 0 0) (size 0 0) (margins 0 0 0 0)
          (fill (type none)) (effects (href "h"))))))"#;
    for (exact, one_under) in [
        (
            SchematicPlotLimits {
                max_text_bytes: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_text_bytes: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            // Header strings (14), table UUID/object (2), retained outline
            // color (9), and Text color/face/hyperlink strings (15).
            SchematicPlotLimits {
                max_metadata_bytes: 40,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_metadata_bytes: 39,
                ..SchematicPlotLimits::default()
            },
        ),
    ] {
        assert_graphics_resource_pair(table, &context, exact, one_under);
    }
}

#[test]
fn annotation_and_table_share_one_metric_work_session() {
    let source = r#"(kicad_sch
      (global_label "AB" (shape input) (at 1 1)
        (effects (font (face "KiCad Monkey Shaping Fixture") (size 1 1))))
      (table (cells
        (table_cell "AB AB" (at 5 6) (size 2.5 3) (margins 0 0 0 0)
          (fill (type none))
          (effects (font (face "KiCad Monkey Shaping Fixture") (size 1 1)))))))"#;
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let fonts = [metric_font()];
    let run = |maximum| {
        let resources = PlotterTextCacheResources {
            fonts: &fonts,
            limits: PlotterTextCacheLimits {
                // One validation hash, one two-pass global-label measure, and
                // one two-pass table-cell linebreak.
                max_hash_bytes: maximum,
                ..PlotterTextCacheLimits::default()
            },
        };
        schematic_plot_document_with_graphics(
            source,
            SchematicPlotLimits::default(),
            &context,
            SchematicDrawingSettings::default(),
            Some(&resources),
        )
    };
    run(METRIC_FONT_BYTES.len() * 5).expect("one aggregate metric session");
    assert_eq!(
        run(METRIC_FONT_BYTES.len() * 5 - 1)
            .expect_err("combined annotation/table hash work")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn table_outline_fill_mapping_matches_schematic_authority() {
    let source = r#"(kicad_sch
      (table (cells
        (table_cell "" (at 0 0) (size 1 1) (fill (type outline)))
        (table_cell "" (at 2 2) (size 1 1) (fill (type solid))))))"#;
    let document = schematic_plot_document_with_graphics(
        source,
        SchematicPlotLimits::default(),
        &SchematicPlotContext {
            worksheet_source: Some(b"(kicad_wks)".to_vec()),
            ..SchematicPlotContext::default()
        },
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("table fill mapping");
    let table = document
        .records
        .iter()
        .find_map(|record| match record {
            SchematicPlotRecord::Table(record) => Some(record),
            _ => None,
        })
        .expect("table record");
    let fills = table
        .operations
        .iter()
        .filter_map(|operation| match operation {
            SchematicPlotOperation::Plotter(PlotterOperation::Rect(rect)) => Some(rect.fill),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fills, [PlotterFill::FilledShape, PlotterFill::NoFill]);
}

#[test]
fn annotation_family_and_line_ceilings_have_exact_and_one_over_cases() {
    let source = r#"(kicad_sch
      (label "L" (at 1 1) (uuid "l"))
      (global_label "" (shape passive) (at 2 2) (uuid "g"))
      (hierarchical_label "H" (shape input) (at 3 3) (uuid "h"))
      (netclass_flag "N" (shape dot) (length 1) (at 4 4) (uuid "n")
        (property "Net Class" "Fast" (at 4 4)))
      (text "\nT" (at 5 5) (uuid "t"))
      (text_box "A\nB" (at 6 6) (size 0 0) (margins 0 0 0 0)
        (fill (type none)) (uuid "b")))"#;
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let pairs = [
        (
            SchematicPlotLimits {
                max_labels: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_labels: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_global_labels: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_global_labels: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_hierarchical_labels: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_hierarchical_labels: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_netclass_flags: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_netclass_flags: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_netclass_flag_properties: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_netclass_flag_properties: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_texts: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_texts: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_text_boxes: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_text_boxes: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_text_box_lines: 2,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_text_box_lines: 1,
                ..SchematicPlotLimits::default()
            },
        ),
    ];
    for (exact, one_over) in pairs {
        assert_annotation_resource_pair(source, &context, exact, one_over);
    }
}

#[test]
fn annotation_settings_and_font_metric_dependencies_fail_closed() {
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    for settings in [
        SchematicDrawingSettings {
            text_offset_ratio: f64::NAN,
            ..SchematicDrawingSettings::default()
        },
        SchematicDrawingSettings {
            default_line_width_nm: 84_699,
            ..SchematicDrawingSettings::default()
        },
    ] {
        assert_eq!(
            schematic_plot_document_with_annotations(
                "(kicad_sch)",
                SchematicPlotLimits::default(),
                &context,
                settings,
                None,
            )
            .expect_err("invalid drawing setting")
            .kind,
            ErrorKind::InvalidBuildValue
        );
    }
    let metric_source = r#"(kicad_sch
      (global_label "AB" (shape passive) (at 0 0)
        (effects (font (face "Unavailable") (size 1 1)))))"#;
    assert_eq!(
        schematic_plot_document_with_annotations(
            metric_source,
            SchematicPlotLimits::default(),
            &context,
            SchematicDrawingSettings::default(),
            None,
        )
        .expect_err("nonempty global decoration requires explicit font metrics")
        .kind,
        ErrorKind::InvalidBuildValue
    );

    let wrapping_source = r#"(kicad_sch
      (text_box "A" (at 0 0) (size 10 10) (margins 0 0 0 0)
        (fill (type none)) (uuid "b")))"#;
    assert_eq!(
        schematic_plot_document_with_annotations(
            wrapping_source,
            SchematicPlotLimits {
                max_text_box_lines: 0,
                ..SchematicPlotLimits::default()
            },
            &context,
            SchematicDrawingSettings::default(),
            None,
        )
        .expect_err("zero line ceiling is checked before font linebreaking")
        .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn annotation_defaults_and_kicad_half_away_rounding_match_python() {
    let source = r#"(kicad_sch
      (label "L" (at 0 0) (effects (font (size 0.000004 0.000004))) (uuid "l"))
      (global_label "" (at 0 0) (effects (font (size 0.000012 0.000012))) (uuid "g"))
      (hierarchical_label "H" (shape future) (at 0 0) (uuid "h"))
      (netclass_flag "N" (at 0 0) (uuid "n"))
      (text "\nX" (at 0 0) (effects (font (size 0.00065 0.00065))) (uuid "t")))"#;
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let document = schematic_plot_document_with_annotations(
        source,
        SchematicPlotLimits::default(),
        &context,
        SchematicDrawingSettings {
            text_offset_ratio: 0.0,
            ..SchematicDrawingSettings::default()
        },
        None,
    )
    .expect("Python annotation defaults and rounding");
    let annotations = document
        .records
        .iter()
        .filter_map(|record| match record {
            SchematicPlotRecord::Annotation(record) => Some(record),
            _ => None,
        })
        .collect::<Vec<_>>();
    let local = annotations
        .iter()
        .find(|record| record.kind.as_str() == "label")
        .expect("local label");
    let SchematicPlotOperation::Text(local_text) = &local.operations[0] else {
        panic!("local label text")
    };
    assert_eq!((local_text.text.x, local_text.text.y), (0, -1));

    let global = annotations
        .iter()
        .find(|record| record.kind.as_str() == "global_label")
        .expect("global label");
    assert_eq!(global.shape.as_deref(), Some("input"));
    let SchematicPlotOperation::Text(global_text) = &global.operations[0] else {
        panic!("global label text")
    };
    assert_eq!(global_text.text.x, 14);

    let hierarchical = annotations
        .iter()
        .find(|record| record.kind.as_str() == "hierarchical_label")
        .expect("hierarchical label");
    assert_eq!(hierarchical.shape.as_deref(), Some("input"));

    let netclass = annotations
        .iter()
        .find(|record| record.kind.as_str() == "netclass_flag")
        .expect("netclass flag");
    assert_eq!(netclass.length_nm, Some(2_540_000));

    let text = annotations
        .iter()
        .find(|record| record.kind.as_str() == "text")
        .expect("ordinary text");
    let SchematicPlotOperation::Text(text_op) = &text.operations[0] else {
        panic!("ordinary text operation")
    };
    assert_eq!(text_op.text.y, -249_700);
}

#[test]
fn annotation_expansion_and_derived_geometry_fail_before_unbounded_publication() {
    let context = SchematicPlotContext {
        project_variables: SchematicPlotVariables::from_entries([("X", "0123456789abcdef")]),
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let expansion = r#"(kicad_sch (text "${X}${X}" (at 0 0) (uuid "t")))"#;
    schematic_plot_document_with_annotations(
        expansion,
        SchematicPlotLimits {
            max_text_bytes: 32,
            ..SchematicPlotLimits::default()
        },
        &context,
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("exact expanded byte ceiling");
    assert_eq!(
        schematic_plot_document_with_annotations(
            expansion,
            SchematicPlotLimits {
                max_text_bytes: 31,
                ..SchematicPlotLimits::default()
            },
            &context,
            SchematicDrawingSettings::default(),
            None,
        )
        .expect_err("expanded byte ceiling before allocation")
        .kind,
        ErrorKind::ResourceLimit
    );

    let expanded_builtin_source = r#"(kicad_sch (title_block (title "${X}") (date "${X}")))"#;
    let expanded_builtin_context = SchematicPlotContext {
        project_variables: SchematicPlotVariables::from_entries([(
            "X",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )]),
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    schematic_plot_document_with_annotations(
        expanded_builtin_source,
        SchematicPlotLimits {
            max_text_bytes: 128,
            max_metadata_bytes: 128,
            ..SchematicPlotLimits::default()
        },
        &expanded_builtin_context,
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("exact aggregate built-in expansion ceiling");
    assert_eq!(
        schematic_plot_document_with_annotations(
            expanded_builtin_source,
            SchematicPlotLimits {
                max_text_bytes: 127,
                max_metadata_bytes: 127,
                ..SchematicPlotLimits::default()
            },
            &expanded_builtin_context,
            SchematicDrawingSettings::default(),
            None,
        )
        .expect_err("aggregate built-in expansion ceiling")
        .kind,
        ErrorKind::ResourceLimit
    );

    let huge_ratio = r#"(kicad_sch
      (label "L" (at 0 0) (effects (font (size 1 1))) (uuid "l")))"#;
    assert_eq!(
        schematic_plot_document_with_annotations(
            huge_ratio,
            SchematicPlotLimits::default(),
            &SchematicPlotContext {
                worksheet_source: Some(b"(kicad_wks)".to_vec()),
                ..SchematicPlotContext::default()
            },
            SchematicDrawingSettings {
                text_offset_ratio: f64::MAX,
                ..SchematicDrawingSettings::default()
            },
            None,
        )
        .expect_err("unsafe derived label offset")
        .kind,
        ErrorKind::InvalidBuildValue
    );

    let huge_text_box = r#"(kicad_sch
      (text_box "A\nB" (at 0 0) (size 0 0) (margins 0 0 0 0)
        (effects (font (size 9007199254.740991 9007199254.740991)))
        (fill (type none)) (uuid "b")))"#;
    assert!(
        schematic_plot_document_with_annotations(
            huge_text_box,
            SchematicPlotLimits::default(),
            &SchematicPlotContext {
                worksheet_source: Some(b"(kicad_wks)".to_vec()),
                ..SchematicPlotContext::default()
            },
            SchematicDrawingSettings::default(),
            None,
        )
        .is_err()
    );
}

#[test]
fn text_box_legacy_margin_rounds_once_after_raw_mm_combination() {
    let source = r#"(kicad_sch
      (text_box "A" (at 0 0) (size 0 0)
        (stroke (width 0.0000014))
        (fill (type none))
        (effects (font (size 0.000008 0.000008)) (justify left top))
        (uuid "b")))"#;
    let document = schematic_plot_document_with_annotations(
        source,
        SchematicPlotLimits::default(),
        &SchematicPlotContext {
            worksheet_source: Some(b"(kicad_wks)".to_vec()),
            ..SchematicPlotContext::default()
        },
        SchematicDrawingSettings::default(),
        None,
    )
    .expect("legacy margin projection");
    let text_box = document
        .records
        .iter()
        .find_map(|record| match record {
            SchematicPlotRecord::Annotation(record) if record.kind.as_str() == "text_box" => {
                Some(record)
            }
            _ => None,
        })
        .expect("text-box record");
    let SchematicPlotOperation::Text(text) = &text_box.operations[1] else {
        panic!("text-box body")
    };
    assert_eq!((text.text.x, text.text.y), (7, 7));
}

#[test]
fn annotation_retained_payload_operation_and_point_budgets_are_exact() {
    let context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let retained = r#"(kicad_sch
      (text_box "A" (at 0 0) (size 0 0) (margins 0 0 0 0)
        (fill (type none)) (effects (href "h")) (uuid "u")))"#;
    for (exact, one_over) in [
        (
            SchematicPlotLimits {
                max_text_bytes: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_text_bytes: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            // Header strings (14), record UUID/object/text (3), and the
            // retained Text color/face/hyperlink strings (15).
            SchematicPlotLimits {
                max_metadata_bytes: 32,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_metadata_bytes: 31,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_operations: 3,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_operations: 2,
                ..SchematicPlotLimits::default()
            },
        ),
    ] {
        assert_annotation_resource_pair(retained, &context, exact, one_over);
    }

    let points = r#"(kicad_sch
      (hierarchical_label "H" (shape input) (at 0 0) (uuid "h")))"#;
    assert_annotation_resource_pair(
        points,
        &context,
        SchematicPlotLimits {
            max_points: 6,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_points: 5,
            ..SchematicPlotLimits::default()
        },
    );
}

#[test]
fn annotation_metric_selection_hash_and_linebreak_work_are_bounded() {
    let vectors: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/schematic_plotter_a0_vectors.json"
    )))
    .expect("schematic vectors");
    let vector = vectors["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .find(|vector| vector["id"] == "explicit-font-metrics-for-schematic-annotations")
        .expect("metric vector");
    let source = vector["source"].as_str().expect("source");
    let context = vector_context(vector);
    let fonts = [metric_font()];

    let run = |cache_limits: PlotterTextCacheLimits| {
        let resources = PlotterTextCacheResources {
            fonts: &fonts,
            limits: cache_limits,
        };
        schematic_plot_document_with_annotations(
            source,
            SchematicPlotLimits::default(),
            &context,
            vector_drawing_settings(vector),
            Some(&resources),
        )
    };

    run(PlotterTextCacheLimits {
        // One validation hash plus two hashes for each of global-label
        // measure, text line-height measure, and text-box linebreaking.
        max_hash_bytes: METRIC_FONT_BYTES.len() * 7,
        ..PlotterTextCacheLimits::default()
    })
    .expect("exact annotation metric hash work");
    assert_eq!(
        run(PlotterTextCacheLimits {
            max_hash_bytes: METRIC_FONT_BYTES.len() * 7 - 1,
            ..PlotterTextCacheLimits::default()
        })
        .expect_err("one-under metric hash work")
        .kind,
        ErrorKind::ResourceLimit
    );

    let mut exact_linebreak = PlotterTextCacheLimits::default();
    exact_linebreak.linebreak.max_tokens = 2;
    run(exact_linebreak).expect("exact text-box linebreak token work");
    let mut one_under_linebreak = PlotterTextCacheLimits::default();
    one_under_linebreak.linebreak.max_tokens = 1;
    assert_eq!(
        run(one_under_linebreak)
            .expect_err("one-under text-box linebreak token work")
            .kind,
        ErrorKind::ResourceLimit
    );

    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    assert_eq!(
        schematic_plot_document_with_annotations(
            source,
            SchematicPlotLimits {
                max_text_box_lines: 1,
                ..SchematicPlotLimits::default()
            },
            &context,
            vector_drawing_settings(vector),
            Some(&resources),
        )
        .expect_err("wrapped line ceiling is enforced during construction")
        .kind,
        ErrorKind::ResourceLimit
    );

    assert_eq!(
        run(PlotterTextCacheLimits {
            max_fonts: 0,
            ..PlotterTextCacheLimits::default()
        })
        .expect_err("font selection is bounded before publication")
        .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn every_independent_resource_ceiling_has_an_exact_and_one_over_case() {
    let empty_worksheet = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    let minimal = "(kicad_sch)";
    assert_resource_pair(
        minimal,
        &empty_worksheet,
        SchematicPlotLimits {
            max_source_bytes: minimal.len(),
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_source_bytes: minimal.len() - 1,
            ..SchematicPlotLimits::default()
        },
    );
    assert_resource_pair(
        "(kicad_sch (unknown))",
        &empty_worksheet,
        SchematicPlotLimits {
            max_depth: 1,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_depth: 0,
            ..SchematicPlotLimits::default()
        },
    );
    assert_resource_pair(
        minimal,
        &empty_worksheet,
        SchematicPlotLimits {
            max_selected_forms: 1,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_selected_forms: 0,
            ..SchematicPlotLimits::default()
        },
    );
    assert_resource_pair(
        minimal,
        &empty_worksheet,
        SchematicPlotLimits {
            max_records: 1,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_records: 0,
            ..SchematicPlotLimits::default()
        },
    );
    assert_resource_pair(
        minimal,
        &empty_worksheet,
        SchematicPlotLimits {
            max_operations: 1,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_operations: 0,
            ..SchematicPlotLimits::default()
        },
    );

    let wire = "(kicad_sch (wire (pts (xy 0 0) (xy 1 1))))";
    assert_resource_pair(
        wire,
        &empty_worksheet,
        SchematicPlotLimits {
            max_points: 2,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_points: 1,
            ..SchematicPlotLimits::default()
        },
    );

    let text_worksheet = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks (tbtext ab (pos 0 0)))".to_vec()),
        ..SchematicPlotContext::default()
    };
    assert_resource_pair(
        minimal,
        &text_worksheet,
        SchematicPlotLimits {
            max_text_bytes: 2,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_text_bytes: 1,
            ..SchematicPlotLimits::default()
        },
    );

    let variable_context = SchematicPlotContext {
        project_variables: SchematicPlotVariables::from_entries([("A", "B")]),
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    assert_resource_pair(
        minimal,
        &variable_context,
        SchematicPlotLimits {
            max_project_variables: 1,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_project_variables: 0,
            ..SchematicPlotLimits::default()
        },
    );
    assert_resource_pair(
        minimal,
        &variable_context,
        SchematicPlotLimits {
            max_project_variable_bytes: 2,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_project_variable_bytes: 1,
            ..SchematicPlotLimits::default()
        },
    );

    let worksheet_bytes = empty_worksheet
        .worksheet_source
        .as_ref()
        .expect("worksheet")
        .len();
    assert_resource_pair(
        minimal,
        &empty_worksheet,
        SchematicPlotLimits {
            max_worksheet_bytes: worksheet_bytes,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_worksheet_bytes: worksheet_bytes - 1,
            ..SchematicPlotLimits::default()
        },
    );
    let line_context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks (line (start 0 0) (end 1 1)))".to_vec()),
        ..SchematicPlotContext::default()
    };
    assert_resource_pair(
        minimal,
        &line_context,
        SchematicPlotLimits {
            max_worksheet_items: 1,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_worksheet_items: 0,
            ..SchematicPlotLimits::default()
        },
    );
    let polygon_context = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks (polygon (pos 0 0) (pts (xy 0 0) (xy 1 1))))".to_vec()),
        ..SchematicPlotContext::default()
    };
    assert_resource_pair(
        minimal,
        &polygon_context,
        SchematicPlotLimits {
            max_worksheet_points: 2,
            ..SchematicPlotLimits::default()
        },
        SchematicPlotLimits {
            max_worksheet_points: 1,
            ..SchematicPlotLimits::default()
        },
    );

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let bitmap_context = SchematicPlotContext {
        worksheet_source: Some(
            format!("(kicad_wks (bitmap (pos 0 0) (data \"{PNG}\")))").into_bytes(),
        ),
        ..SchematicPlotContext::default()
    };
    let bitmap_pairs = [
        (
            SchematicPlotLimits {
                max_worksheet_bitmap_data_parts: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_worksheet_bitmap_data_parts: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_worksheet_bitmap_encoded_bytes: PNG.len(),
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_worksheet_bitmap_encoded_bytes: PNG.len() - 1,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_worksheet_bitmap_decoded_bytes: 68,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_worksheet_bitmap_decoded_bytes: 67,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_worksheet_bitmap_width_px: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_worksheet_bitmap_width_px: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_worksheet_bitmap_height_px: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_worksheet_bitmap_height_px: 0,
                ..SchematicPlotLimits::default()
            },
        ),
        (
            SchematicPlotLimits {
                max_worksheet_bitmap_pixels: 1,
                ..SchematicPlotLimits::default()
            },
            SchematicPlotLimits {
                max_worksheet_bitmap_pixels: 0,
                ..SchematicPlotLimits::default()
            },
        ),
    ];
    for (exact, one_over) in bitmap_pairs {
        assert_resource_pair(minimal, &bitmap_context, exact, one_over);
    }
}

#[test]
fn malformed_paper_and_bitmap_encodings_fail_closed() {
    let empty_worksheet = SchematicPlotContext {
        worksheet_source: Some(b"(kicad_wks)".to_vec()),
        ..SchematicPlotContext::default()
    };
    assert!(
        schematic_plot_document(
            "(kicad_sch (paper User -1 2))",
            SchematicPlotLimits::default(),
            &empty_worksheet,
        )
        .is_err()
    );
    for data in ["A===", "AA=A", "AB==", "AAAA"] {
        let context = SchematicPlotContext {
            worksheet_source: Some(
                format!("(kicad_wks (bitmap (pos 0 0) (data \"{data}\")))").into_bytes(),
            ),
            ..SchematicPlotContext::default()
        };
        assert!(
            schematic_plot_document("(kicad_sch)", SchematicPlotLimits::default(), &context)
                .is_err(),
            "{data}"
        );
    }
}
