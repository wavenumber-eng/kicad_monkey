//! Bounded KiCad-compatible quadratic and cubic outline decomposition.

use std::fmt;

/// One point in the caller's declared coordinate space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextPoint {
    pub x: f64,
    pub y: f64,
}

/// Independent retained-output and decomposition-work ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextBezierLimits {
    pub max_points: usize,
    pub max_work_items: usize,
}

impl Default for TextBezierLimits {
    fn default() -> Self {
        Self {
            max_points: 16 * 1024 * 1024,
            max_work_items: 16 * 1024 * 1024,
        }
    }
}

/// Flattened points and exact charged work for boundary tests and profiling.
#[derive(Clone, Debug)]
pub struct TextBezierOutput {
    pub points: Vec<TextPoint>,
    pub work_items: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextBezierErrorKind {
    InvalidInput,
    ResourceLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextBezierError {
    pub kind: TextBezierErrorKind,
    pub message: &'static str,
}

impl fmt::Display for TextBezierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for TextBezierError {}

/// Flatten one quadratic curve using KiCad's outline-decomposer approximation.
pub fn flatten_quadratic_bezier(
    control: [TextPoint; 3],
    max_error: f64,
    limits: TextBezierLimits,
) -> Result<TextBezierOutput, TextBezierError> {
    validate_input(&control, max_error)?;
    let max_error = effective_error(max_error);
    let mut state = FlattenState::new(limits);
    state.charge()?;
    let ddx = 2.0 * control[1].x - control[0].x - control[2].x;
    let ddy = 2.0 * control[1].y - control[0].y - control[2].y;
    let u0 = (control[1].x - control[0].x) * ddx + (control[1].y - control[0].y) * ddy;
    let u2 = (control[2].x - control[1].x) * ddx + (control[2].y - control[1].y) * ddy;
    let cross = (control[2].x - control[0].x) * ddy - (control[2].y - control[0].y) * ddx;
    let denominator = ddx.hypot(ddy);
    if cross == 0.0 || denominator == 0.0 {
        state.push(control[0])?;
        state.push(control[2])?;
        return Ok(state.finish());
    }

    let x0 = u0 / cross;
    let x2 = u2 / cross;
    if x2 == x0 {
        state.push(control[0])?;
        state.push(control[2])?;
        return Ok(state.finish());
    }
    let scale = cross.abs() / (denominator * (x2 - x0).abs());
    if scale <= 0.0 {
        state.push(control[0])?;
        state.push(control[2])?;
        return Ok(state.finish());
    }

    let a0 = approx_int(x0);
    let a2 = approx_int(x2);
    let segment_count_f = (0.5 * (a2 - a0).abs() * (scale / max_error).sqrt()).ceil();
    let segment_count = bounded_count(segment_count_f, limits)?;
    let v0 = approx_inv_int(a0);
    let v2 = approx_inv_int(a2);
    state.push(control[0])?;
    if v2 != v0 {
        for index in 0..segment_count {
            state.charge()?;
            let u = approx_inv_int(a0 + (a2 - a0) * index as f64 / segment_count as f64);
            state.push(eval_quadratic(control, (u - v0) / (v2 - v0)))?;
        }
    }
    state.push(control[2])?;
    Ok(state.finish())
}

/// Flatten one cubic curve using KiCad's inflection-aware approximation.
pub fn flatten_cubic_bezier(
    control: [TextPoint; 4],
    max_error: f64,
    limits: TextBezierLimits,
) -> Result<TextBezierOutput, TextBezierError> {
    validate_input(&control, max_error)?;
    let mut state = FlattenState::new(limits);
    state.push(control[0])?;
    cubic_poly(control, effective_error(max_error), &mut state)?;
    Ok(state.finish())
}

struct FlattenState {
    points: Vec<TextPoint>,
    limits: TextBezierLimits,
    work_items: usize,
}

impl FlattenState {
    fn new(limits: TextBezierLimits) -> Self {
        Self {
            points: Vec::with_capacity(limits.max_points.min(256)),
            limits,
            work_items: 0,
        }
    }

    fn push(&mut self, point: TextPoint) -> Result<(), TextBezierError> {
        if self.points.len() >= self.limits.max_points {
            return Err(resource_error(
                "flattened point count exceeds the configured limit",
            ));
        }
        self.points.push(point);
        Ok(())
    }

    fn charge(&mut self) -> Result<(), TextBezierError> {
        if self.work_items >= self.limits.max_work_items {
            return Err(resource_error(
                "Bezier decomposition work exceeds the configured limit",
            ));
        }
        self.work_items += 1;
        Ok(())
    }

    fn reserve_pending(&self, count: usize) -> Result<(), TextBezierError> {
        if count > self.limits.max_work_items.saturating_sub(self.work_items) {
            Err(resource_error(
                "Bezier work stack exceeds the configured limit",
            ))
        } else {
            Ok(())
        }
    }

    fn finish(self) -> TextBezierOutput {
        TextBezierOutput {
            points: self.points,
            work_items: self.work_items,
        }
    }
}

fn cubic_poly(
    control: [TextPoint; 4],
    max_error: f64,
    state: &mut FlattenState,
) -> Result<(), TextBezierError> {
    state.charge()?;
    if inflection_class(control) == 0 {
        return cubic_parabolic(control, max_error, state);
    }
    let (count, first_t, _) = inflection_points(control);
    match count {
        2 => cubic_two_inflections(control, first_t, max_error, state),
        1 => {
            let (first, second) = subdivide_cubic(control, first_t);
            cubic_parabolic(first, max_error, state)?;
            cubic_parabolic(second, max_error, state)
        }
        _ => cubic_parabolic(control, max_error, state),
    }
}

fn cubic_two_inflections(
    control: [TextPoint; 4],
    first_t: f64,
    max_error: f64,
    state: &mut FlattenState,
) -> Result<(), TextBezierError> {
    let (first, remainder) = subdivide_cubic(control, first_t);
    cubic_parabolic(first, max_error, state)?;
    let (second_count, second_t, _) = inflection_points(remainder);
    if !matches!(second_count, 1 | 2) {
        return state.push(remainder[3]);
    }
    let (middle, last) = subdivide_cubic(remainder, second_t);
    recursive_cubic(middle, max_error, state)?;
    cubic_parabolic(last, max_error, state)
}

fn cubic_parabolic(
    mut current: [TextPoint; 4],
    max_error: f64,
    state: &mut FlattenState,
) -> Result<(), TextBezierError> {
    loop {
        state.charge()?;
        if current
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid_error(
                "Bezier subdivision produced a non-finite coordinate",
            ));
        }
        if cubic_is_flat(current, max_error) {
            return state.push(current[3]);
        }
        let deviation = third_control_deviation(current);
        if deviation <= 0.0 {
            return recursive_cubic(current, max_error, state);
        }
        let t = 2.0 * (max_error / (3.0 * deviation)).sqrt();
        if !t.is_finite() || t > 1.0 {
            return recursive_cubic(current, max_error, state);
        }
        let (first, second) = subdivide_cubic(current, t);
        if cubic_is_flat(first, max_error) {
            state.push(first[3])?;
        } else {
            recursive_cubic(first, max_error, state)?;
        }
        current = second;
    }
}

fn recursive_cubic(
    control: [TextPoint; 4],
    max_error: f64,
    state: &mut FlattenState,
) -> Result<(), TextBezierError> {
    let mut stack = vec![control];
    while let Some(curve) = stack.pop() {
        state.charge()?;
        if curve[3] == curve[0] {
            continue;
        }
        if cubic_is_flat(curve, max_error) {
            state.push(curve[3])?;
            continue;
        }
        state.reserve_pending(stack.len().saturating_add(2))?;
        let (left, right) = subdivide_cubic(curve, 0.5);
        stack.push(right);
        stack.push(left);
    }
    Ok(())
}

fn validate_input<const N: usize>(
    control: &[TextPoint; N],
    max_error: f64,
) -> Result<(), TextBezierError> {
    if !max_error.is_finite()
        || control
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        Err(invalid_error(
            "Bezier coordinates and tolerance must be finite",
        ))
    } else {
        Ok(())
    }
}

fn effective_error(max_error: f64) -> f64 {
    if max_error <= 0.0 { 10.0 } else { max_error }
}

fn bounded_count(value: f64, limits: TextBezierLimits) -> Result<usize, TextBezierError> {
    if !value.is_finite() || value < 0.0 || value > limits.max_work_items as f64 {
        Err(resource_error(
            "quadratic segment count exceeds the configured work limit",
        ))
    } else {
        Ok(value as usize)
    }
}

fn approx_int(value: f64) -> f64 {
    let d = 0.674_489_750_196_081_7;
    let d4 = d * d * d * d;
    value / (1.0 - d + (d4 + value * value * 0.25).powf(0.25))
}

fn approx_inv_int(value: f64) -> f64 {
    let p = 0.395_388_16;
    value * (1.0 - p + (p * p + 0.25 * value * value).sqrt())
}

fn eval_quadratic(control: [TextPoint; 3], t: f64) -> TextPoint {
    let omt = 1.0 - t;
    add(
        add(
            scale(omt * omt, control[0]),
            scale(2.0 * omt * t, control[1]),
        ),
        scale(t * t, control[2]),
    )
}

fn cubic_is_flat(control: [TextPoint; 4], max_error: f64) -> bool {
    let delta = sub(control[3], control[0]);
    let delta_norm = squared_norm(delta);
    if delta_norm == 0.0 {
        return true;
    }
    let cross1 = cross(delta, sub(control[1], control[0]));
    let cross2 = cross(delta, sub(control[2], control[0]));
    let d1 = cross1 * cross1 / delta_norm;
    let d2 = cross2 * cross2 / delta_norm;
    let factor: f64 = if cross1 * cross2 > 0.0 {
        3.0 / 4.0
    } else {
        4.0 / 9.0
    };
    let tolerance = max_error * max_error;
    d1 * factor.powi(2) <= tolerance && d2 * factor.powi(2) <= tolerance
}

fn subdivide_cubic(control: [TextPoint; 4], t: f64) -> ([TextPoint; 4], [TextPoint; 4]) {
    let left1 = add(control[0], scale(t, sub(control[1], control[0])));
    let middle = add(control[1], scale(t, sub(control[2], control[1])));
    let left2 = add(left1, scale(t, sub(middle, left1)));
    let right2 = add(control[2], scale(t, sub(control[3], control[2])));
    let right1 = add(middle, scale(t, sub(right2, middle)));
    let shared = add(left2, scale(t, sub(right1, left2)));
    (
        [control[0], left1, left2, shared],
        [shared, right1, right2, control[3]],
    )
}

fn inflection_class(control: [TextPoint; 4]) -> i8 {
    let d21 = sub(control[1], control[0]);
    let d32 = sub(control[2], control[1]);
    let d43 = sub(control[3], control[2]);
    let cross1 = cross(d21, d32) * cross(d32, d43);
    let cross2 = cross(d21, d32) * cross(d21, d43);
    if cross1 < 0.0 {
        1
    } else if cross2 > 0.0 || (dot(d21, d32) > 0.0) ^ (dot(d32, d43) > 0.0) {
        0
    } else {
        -1
    }
}

fn third_control_deviation(control: [TextPoint; 4]) -> f64 {
    let delta = sub(control[1], control[0]);
    let length_squared = squared_norm(delta);
    if length_squared < 1.0e-6 {
        return 0.0;
    }
    let length = length_squared.sqrt();
    let r = (control[1].y - control[0].y) / length;
    let s = (control[0].x - control[1].x) / length;
    let u = (control[1].x * control[0].y - control[0].x * control[1].y) / length;
    (r * control[2].x + s * control[2].y + u).abs()
}

fn inflection_points(control: [TextPoint; 4]) -> (u8, f64, f64) {
    let a = add(
        add(scale(-1.0, control[0]), scale(3.0, control[1])),
        add(scale(-3.0, control[2]), control[3]),
    );
    let b = add(
        scale(3.0, control[0]),
        add(scale(-6.0, control[1]), scale(3.0, control[2])),
    );
    let c = add(scale(-3.0, control[0]), scale(3.0, control[1]));
    let qa = 3.0 * cross(a, b);
    let qb = 3.0 * cross(a, c);
    let qc = cross(b, c);
    let root_term = qb * qb - 4.0 * qa * qc;
    if root_term < 0.0 || qa == 0.0 {
        return (0, 0.0, 0.0);
    }
    classify_inflection_roots(
        (-qb + root_term.sqrt()) / (2.0 * qa),
        (-qb - root_term.sqrt()) / (2.0 * qa),
    )
}

fn classify_inflection_roots(mut first: f64, mut second: f64) -> (u8, f64, f64) {
    let first_inside = 0.0 < first && first < 1.0;
    let second_inside = 0.0 < second && second < 1.0;
    if first_inside && second_inside {
        if first > second {
            std::mem::swap(&mut first, &mut second);
        }
        if second - first > 0.000_01 {
            (2, first, second)
        } else {
            (1, first, second)
        }
    } else if first_inside {
        (1, first, 0.0)
    } else if second_inside {
        (1, second, 0.0)
    } else {
        (0, 0.0, 0.0)
    }
}

fn add(left: TextPoint, right: TextPoint) -> TextPoint {
    TextPoint {
        x: left.x + right.x,
        y: left.y + right.y,
    }
}

fn sub(left: TextPoint, right: TextPoint) -> TextPoint {
    TextPoint {
        x: left.x - right.x,
        y: left.y - right.y,
    }
}

fn scale(value: f64, point: TextPoint) -> TextPoint {
    TextPoint {
        x: value * point.x,
        y: value * point.y,
    }
}

fn dot(left: TextPoint, right: TextPoint) -> f64 {
    left.x * right.x + left.y * right.y
}

fn cross(left: TextPoint, right: TextPoint) -> f64 {
    left.x * right.y - left.y * right.x
}

fn squared_norm(point: TextPoint) -> f64 {
    dot(point, point)
}

fn invalid_error(message: &'static str) -> TextBezierError {
    TextBezierError {
        kind: TextBezierErrorKind::InvalidInput,
        message,
    }
}

fn resource_error(message: &'static str) -> TextBezierError {
    TextBezierError {
        kind: TextBezierErrorKind::ResourceLimit,
        message,
    }
}
