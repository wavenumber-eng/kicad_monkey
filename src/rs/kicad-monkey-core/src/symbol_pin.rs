//! Non-text library-symbol pin geometry.

use crate::plotter_types::{PlotterCircle, PlotterFill, PlotterOperation, PlotterPoly};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position, Sexp};

const PIN_COLOR: &str = "#840000FF";
const PIN_WIDTH_NM: i64 = 152_400;
const PIN_RADIUS_NM: i64 = 635_000;
const JS_SAFE_MAX: f64 = 9_007_199_254_740_991.0;
const JS_SAFE_MAX_I64: i64 = 9_007_199_254_740_991;

pub(crate) fn pin_operations(
    form: &Sexp,
    max_points: usize,
    point_count: &mut usize,
    position: Position,
) -> Result<Vec<PlotterOperation>, Error> {
    if pin_is_hidden(form) {
        return Ok(Vec::new());
    }
    value_at(form, 1)
        .ok_or_else(|| model_error("Symbol pin requires electrical type", position))?;
    let style = value_at(form, 2)
        .ok_or_else(|| model_error("Symbol pin requires graphic style", position))?;
    validate_style(style, position)?;
    let at = child(form, "at");
    let x = numeric_or_missing(at, 1, 0.0, position)?;
    let y = numeric_or_missing(at, 2, 0.0, position)?;
    let angle = angle_or_default(at, 3, position)?;
    let external = [mm_to_nm(x, position)?, mm_to_nm(-y, position)?];
    let length = numeric_or_missing(child(form, "length"), 1, 2.54, position)?;
    let length_nm = mm_to_nm(length, position)?;
    let root = pin_root(x, y, angle, length, external, length_nm, position)?;
    pin_style_operations(root, external, style, max_points, point_count)
}

fn validate_style(style: &str, position: Position) -> Result<(), Error> {
    if matches!(
        style,
        "line"
            | "inverted"
            | "clock"
            | "inverted_clock"
            | "input_low"
            | "clock_low"
            | "output_low"
            | "edge_clock_high"
            | "non_logic"
    ) {
        Ok(())
    } else {
        Err(model_error(
            "Unsupported symbol pin graphic style",
            position,
        ))
    }
}

fn pin_root(
    x: f64,
    y: f64,
    angle: f64,
    length: f64,
    external: [i64; 2],
    length_nm: i64,
    position: Position,
) -> Result<[i64; 2], Error> {
    match (angle.round_ties_even() as i64).rem_euclid(360) {
        0 => Ok([
            coordinate_add(external[0], length_nm, position)?,
            external[1],
        ]),
        90 => Ok([
            external[0],
            coordinate_add(external[1], -length_nm, position)?,
        ]),
        180 => Ok([
            coordinate_add(external[0], -length_nm, position)?,
            external[1],
        ]),
        270 => Ok([
            external[0],
            coordinate_add(external[1], length_nm, position)?,
        ]),
        _ => {
            let radians = angle.to_radians();
            Ok([
                mm_to_nm(x + length * radians.cos(), position)?,
                mm_to_nm(-(y + length * radians.sin()), position)?,
            ])
        }
    }
}

fn pin_style_operations(
    start: [i64; 2],
    end: [i64; 2],
    style: &str,
    max_points: usize,
    point_count: &mut usize,
) -> Result<Vec<PlotterOperation>, Error> {
    let axis = axis_step(start, end);
    let mut operations = Vec::new();
    match style {
        "inverted" | "inverted_clock" => {
            operations.push(pin_circle(step(start, axis, PIN_RADIUS_NM))?);
            push_pin_line(
                &mut operations,
                step(start, axis, PIN_RADIUS_NM * 2),
                end,
                max_points,
                point_count,
            )?;
        }
        "edge_clock_high" => {
            edge_clock_high(&mut operations, start, end, axis, max_points, point_count)?
        }
        _ => push_pin_line(&mut operations, start, end, max_points, point_count)?,
    }
    add_pin_decorations(&mut operations, start, axis, style, max_points, point_count)?;
    Ok(operations)
}

fn edge_clock_high(
    operations: &mut Vec<PlotterOperation>,
    start: [i64; 2],
    end: [i64; 2],
    [mx, my]: [i64; 2],
    max_points: usize,
    point_count: &mut usize,
) -> Result<(), Error> {
    let points = if my == 0 {
        vec![
            [start[0], start[1] + PIN_RADIUS_NM],
            [start[0] + mx * PIN_RADIUS_NM * 2, start[1]],
            [start[0], start[1] - PIN_RADIUS_NM],
        ]
    } else {
        vec![
            [start[0] + PIN_RADIUS_NM, start[1]],
            [start[0], start[1] + my * PIN_RADIUS_NM * 2],
            [start[0] - PIN_RADIUS_NM, start[1]],
        ]
    };
    operations.push(pin_poly(points, max_points, point_count)?);
    push_pin_line(
        operations,
        step(start, [mx, my], PIN_RADIUS_NM * 2),
        end,
        max_points,
        point_count,
    )
}

fn add_pin_decorations(
    operations: &mut Vec<PlotterOperation>,
    start: [i64; 2],
    [mx, my]: [i64; 2],
    style: &str,
    max_points: usize,
    point_count: &mut usize,
) -> Result<(), Error> {
    if matches!(style, "clock" | "inverted_clock" | "clock_low") {
        let points = if my == 0 {
            vec![
                [start[0], start[1] + PIN_RADIUS_NM],
                [start[0] - mx * PIN_RADIUS_NM * 2, start[1]],
                [start[0], start[1] - PIN_RADIUS_NM],
            ]
        } else {
            vec![
                [start[0] + PIN_RADIUS_NM, start[1]],
                [start[0], start[1] - my * PIN_RADIUS_NM * 2],
                [start[0] - PIN_RADIUS_NM, start[1]],
            ]
        };
        operations.push(pin_poly(points, max_points, point_count)?);
    }
    add_low_or_non_logic(operations, start, [mx, my], style, max_points, point_count)
}

fn add_low_or_non_logic(
    operations: &mut Vec<PlotterOperation>,
    start: [i64; 2],
    [mx, my]: [i64; 2],
    style: &str,
    max_points: usize,
    point_count: &mut usize,
) -> Result<(), Error> {
    if matches!(style, "input_low" | "clock_low") {
        let points = if my == 0 {
            vec![
                [start[0] + mx * PIN_RADIUS_NM * 2, start[1]],
                [
                    start[0] + mx * PIN_RADIUS_NM * 2,
                    start[1] - PIN_RADIUS_NM * 2,
                ],
                start,
            ]
        } else {
            vec![
                [start[0], start[1] + my * PIN_RADIUS_NM * 2],
                [
                    start[0] - PIN_RADIUS_NM * 2,
                    start[1] + my * PIN_RADIUS_NM * 2,
                ],
                start,
            ]
        };
        operations.push(pin_poly(points, max_points, point_count)?);
    }
    if style == "output_low" {
        let points = if my == 0 {
            vec![
                [start[0], start[1] - PIN_RADIUS_NM * 2],
                [start[0] + mx * PIN_RADIUS_NM * 2, start[1]],
            ]
        } else {
            vec![
                [start[0] - PIN_RADIUS_NM * 2, start[1]],
                [start[0], start[1] + my * PIN_RADIUS_NM * 2],
            ]
        };
        operations.push(pin_poly(points, max_points, point_count)?);
    } else if style == "non_logic" {
        add_non_logic(operations, start, [mx, my], max_points, point_count)?;
    }
    Ok(())
}

fn add_non_logic(
    operations: &mut Vec<PlotterOperation>,
    start: [i64; 2],
    [mx, my]: [i64; 2],
    max_points: usize,
    point_count: &mut usize,
) -> Result<(), Error> {
    let lines = [
        [
            [
                start[0] - (mx + my) * PIN_RADIUS_NM,
                start[1] - (my - mx) * PIN_RADIUS_NM,
            ],
            [
                start[0] + (mx + my) * PIN_RADIUS_NM,
                start[1] + (my - mx) * PIN_RADIUS_NM,
            ],
        ],
        [
            [
                start[0] - (mx - my) * PIN_RADIUS_NM,
                start[1] - (my + mx) * PIN_RADIUS_NM,
            ],
            [
                start[0] + (mx - my) * PIN_RADIUS_NM,
                start[1] + (my + mx) * PIN_RADIUS_NM,
            ],
        ],
    ];
    for points in lines {
        operations.push(pin_poly(points.to_vec(), max_points, point_count)?);
    }
    Ok(())
}

fn push_pin_line(
    operations: &mut Vec<PlotterOperation>,
    start: [i64; 2],
    end: [i64; 2],
    max_points: usize,
    point_count: &mut usize,
) -> Result<(), Error> {
    if start != end {
        operations.push(pin_poly(vec![start, end], max_points, point_count)?);
    }
    Ok(())
}

fn pin_poly(
    points: Vec<[i64; 2]>,
    max_points: usize,
    point_count: &mut usize,
) -> Result<PlotterOperation, Error> {
    if points
        .iter()
        .flatten()
        .any(|coordinate| !(-JS_SAFE_MAX_I64..=JS_SAFE_MAX_I64).contains(coordinate))
    {
        return Err(model_error(
            "Derived symbol pin geometry exceeds safe-integer range",
            Position::START,
        ));
    }
    *point_count = point_count.saturating_add(points.len());
    if *point_count > max_points {
        return Err(limit_error("Symbol geometry point limit exceeded"));
    }
    Ok(PlotterOperation::PlotPoly(PlotterPoly {
        points,
        fill: PlotterFill::NoFill,
        width_nm: PIN_WIDTH_NM,
        layer: None,
        stroke_color: Some(PIN_COLOR.to_owned()),
        fill_color: None,
        line_style: None,
    }))
}

fn pin_circle([cx, cy]: [i64; 2]) -> Result<PlotterOperation, Error> {
    if !(-JS_SAFE_MAX_I64..=JS_SAFE_MAX_I64).contains(&cx)
        || !(-JS_SAFE_MAX_I64..=JS_SAFE_MAX_I64).contains(&cy)
    {
        return Err(model_error(
            "Derived symbol pin geometry exceeds safe-integer range",
            Position::START,
        ));
    }
    Ok(PlotterOperation::Circle(PlotterCircle {
        cx,
        cy,
        diameter_nm: PIN_RADIUS_NM * 2,
        fill: PlotterFill::NoFill,
        width_nm: PIN_WIDTH_NM,
        layer: None,
        role: None,
        layers: Vec::new(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
        stroke_color: Some(PIN_COLOR.to_owned()),
        fill_color: None,
        line_style: None,
    }))
}

fn step([x, y]: [i64; 2], [mx, my]: [i64; 2], distance: i64) -> [i64; 2] {
    [x + mx * distance, y + my * distance]
}

fn axis_step([start_x, start_y]: [i64; 2], [end_x, end_y]: [i64; 2]) -> [i64; 2] {
    [(end_x - start_x).signum(), (end_y - start_y).signum()]
}

fn pin_is_hidden(form: &Sexp) -> bool {
    has_atom(form, "hide")
        || child(form, "hide").and_then(|value| value_at(value, 1)) == Some("yes")
}

fn mm_to_nm(value: f64, position: Position) -> Result<i64, Error> {
    let scaled = value * 1_000_000.0;
    if !scaled.is_finite() || !(-JS_SAFE_MAX..=JS_SAFE_MAX).contains(&scaled) {
        return Err(model_error(
            "Symbol pin coordinate exceeds safe-integer range",
            position,
        ));
    }
    Ok(scaled.round_ties_even() as i64)
}

fn coordinate_add(left: i64, right: i64, position: Position) -> Result<i64, Error> {
    let value = left as i128 + right as i128;
    if !(-(JS_SAFE_MAX_I64 as i128)..=JS_SAFE_MAX_I64 as i128).contains(&value) {
        return Err(model_error(
            "Derived symbol pin coordinate exceeds safe-integer range",
            position,
        ));
    }
    Ok(value as i64)
}

fn has_atom(form: &Sexp, expected: &str) -> bool {
    list(form).is_some_and(|values| values.iter().any(|value| text(value) == Some(expected)))
}

fn child<'a>(form: &'a Sexp, head: &str) -> Option<&'a Sexp> {
    list(form)?.iter().find(|candidate| {
        list(candidate)
            .and_then(|values| values.first())
            .and_then(text)
            == Some(head)
    })
}

fn list(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn text(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

fn value_at(form: &Sexp, index: usize) -> Option<&str> {
    list(form)?.get(index).and_then(text)
}

fn numeric_or_missing(
    form: Option<&Sexp>,
    index: usize,
    default: f64,
    position: Position,
) -> Result<f64, Error> {
    let Some(value) = form.and_then(list).and_then(|values| values.get(index)) else {
        return Ok(default);
    };
    finite_numeric(value, position)
}

fn angle_or_default(form: Option<&Sexp>, index: usize, position: Position) -> Result<f64, Error> {
    let Some(value) = form.and_then(list).and_then(|values| values.get(index)) else {
        return Ok(0.0);
    };
    match parse_numeric(value) {
        Ok(number) if number.is_finite() => Ok(number),
        Ok(_) => Err(model_error("Symbol pin value must be finite", position)),
        Err(()) => Ok(0.0),
    }
}

fn finite_numeric(value: &Sexp, position: Position) -> Result<f64, Error> {
    let number = parse_numeric(value)
        .map_err(|()| model_error("Expected numeric symbol pin value", position))?;
    if number.is_finite() {
        Ok(number)
    } else {
        Err(model_error("Symbol pin value must be finite", position))
    }
}

fn parse_numeric(value: &Sexp) -> Result<f64, ()> {
    let number = match value {
        Sexp::Integer(value) => *value as f64,
        Sexp::Float(value) => *value,
        Sexp::Atom(value) | Sexp::Quoted(value) => value.parse().map_err(|_| ())?,
        _ => return Err(()),
    };
    Ok(number)
}

fn model_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn limit_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        Position::START,
    )
}
