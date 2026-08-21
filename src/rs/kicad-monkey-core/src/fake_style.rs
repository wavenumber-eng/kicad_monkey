//! Exact FreeType fixed-point port of KiCad's fake-bold/fake-italic pass.
//!
//! KiCad's outline font loads each hinted glyph through FreeType with an
//! optional fake-italic `FT_Set_Transform` shear and then runs
//! `FT_Outline_Embolden(outline, 1 << 6)` for fake bold, both on the glyph's
//! 26.6 fixed-point control points before decomposition. This module mirrors
//! that stage on the hinted outline commands: coordinates are recovered onto
//! the exact 26.6 grid, transformed with FreeType's integer arithmetic
//! (`FT_Vector_Transform`, `FT_Outline_EmboldenXY`, `FT_Vector_NormLen`,
//! `FT_Outline_Get_Orientation` from the pinned 2.13.3 sources with 32-bit
//! `FT_Long`, matching the Windows ABI KiCad ships), and written back.
//!
//! One deliberate representational difference is documented here: FreeType
//! emboldens the raw TrueType point array, where implied on-curve midpoints
//! between consecutive off-curve controls are not materialized, while the
//! hinted pen stream materializes them. A materialized midpoint is collinear
//! with its neighbouring controls, so it receives the same lateral shift as
//! the surrounding segment up to fixed-point normalization rounding; the live
//! kicad-cli oracle bounds the residual effect.

use crate::{TextContourError, TextContourErrorKind};
use kicad_monkey_contracts::FiniteFloat;
use kicad_monkey_contracts::generated::outline_vector::OutlineCommand;

/// KiCad's fake-italic tilt in degrees (`OUTLINE_FONT` oblique slant).
const FAKE_ITALIC_TILT_DEGREES: f64 = 12.0;
/// FreeType 26.6 units per hinted internal unit: hinted commands carry
/// `pixels * 4` internal units and one FreeType pixel is 64 26.6 units.
const FIXED_PER_INTERNAL_UNIT: f64 = 16.0;
/// KiCad's `FT_Outline_Embolden` strength: `1 << 6`, one pixel.
const EMBOLDEN_STRENGTH: i32 = 1 << 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FtVector {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Orientation {
    TrueType,
    PostScript,
    None,
}

/// Which coordinate pair of one outline command a collected point maps to.
#[derive(Clone, Copy)]
enum Slot {
    Move,
    Line,
    QuadControl,
    QuadEnd,
    CurveControl1,
    CurveControl2,
    CurveEnd,
}

/// One collected point's home in the command stream.
#[derive(Clone, Copy)]
struct Locator {
    command: usize,
    slot: Slot,
}

/// Apply KiCad's fake italic shear and fake bold embolden to one hinted glyph.
///
/// The work and the collected point array are bounded by the caller's already
/// enforced outline command ceiling (at most three points per command).
pub(crate) fn apply_fake_style(
    commands: &mut [OutlineCommand],
    fake_bold: bool,
    fake_italic: bool,
) -> Result<(), TextContourError> {
    let mut outline = collect_outline(commands)?;
    if fake_italic {
        let (xx, xy) = fake_italic_matrix();
        for point in &mut outline.points {
            // `FT_Vector_Transform` with `yx = 0`, `yy = 0x10000` leaves y as
            // `mul_fix(y, 0x10000) = y` exactly.
            point.x = mul_fix(point.x, xx).wrapping_add(mul_fix(point.y, xy));
        }
    }
    if fake_bold {
        embolden_xy(
            &mut outline.points,
            &outline.contour_ends,
            EMBOLDEN_STRENGTH,
            EMBOLDEN_STRENGTH,
        );
    }
    write_back(commands, &outline);
    Ok(())
}

/// The glyph's FreeType-style point array plus write-back bookkeeping.
struct CollectedOutline {
    points: Vec<FtVector>,
    /// Index of each contour's last point, FreeType `contours` semantics.
    contour_ends: Vec<usize>,
    locators: Vec<Locator>,
    /// Closing pen points that alias a contour's first point in the raw array.
    aliases: Vec<(Locator, usize)>,
}

fn collect_outline(commands: &[OutlineCommand]) -> Result<CollectedOutline, TextContourError> {
    let mut outline = CollectedOutline {
        points: Vec::with_capacity(commands.len().saturating_mul(3).min(4096)),
        contour_ends: Vec::new(),
        locators: Vec::new(),
        aliases: Vec::new(),
    };
    let mut contour_start = 0usize;
    for (index, command) in commands.iter().enumerate() {
        match command {
            OutlineCommand::MoveTo(command) => {
                close_contour(&mut outline, contour_start)?;
                contour_start = outline.points.len();
                push_point(
                    &mut outline,
                    index,
                    Slot::Move,
                    command.x.get(),
                    command.y.get(),
                )?;
            }
            OutlineCommand::LineTo(command) => {
                push_point(
                    &mut outline,
                    index,
                    Slot::Line,
                    command.x.get(),
                    command.y.get(),
                )?;
            }
            OutlineCommand::QuadTo(command) => {
                push_point(
                    &mut outline,
                    index,
                    Slot::QuadControl,
                    command.control_x.get(),
                    command.control_y.get(),
                )?;
                push_point(
                    &mut outline,
                    index,
                    Slot::QuadEnd,
                    command.x.get(),
                    command.y.get(),
                )?;
            }
            OutlineCommand::CurveTo(command) => {
                push_point(
                    &mut outline,
                    index,
                    Slot::CurveControl1,
                    command.control1_x.get(),
                    command.control1_y.get(),
                )?;
                push_point(
                    &mut outline,
                    index,
                    Slot::CurveControl2,
                    command.control2_x.get(),
                    command.control2_y.get(),
                )?;
                push_point(
                    &mut outline,
                    index,
                    Slot::CurveEnd,
                    command.x.get(),
                    command.y.get(),
                )?;
            }
            OutlineCommand::Close(_) => {
                close_contour(&mut outline, contour_start)?;
                contour_start = outline.points.len();
            }
        }
    }
    close_contour(&mut outline, contour_start)?;
    Ok(outline)
}

/// Finish one pen contour, aliasing the closing point onto the raw first point.
///
/// FreeType's raw point array never repeats a contour's first point at the
/// end, while the pen stream returns to it, so one exactly equal trailing
/// point folds back onto the contour start before FreeType arithmetic runs.
fn close_contour(
    outline: &mut CollectedOutline,
    contour_start: usize,
) -> Result<(), TextContourError> {
    if outline.points.len() == contour_start {
        return Ok(());
    }
    let last = outline.points.len() - 1;
    if last > contour_start && outline.points[last] == outline.points[contour_start] {
        outline.points.pop();
        let locator = outline
            .locators
            .pop()
            .expect("locators parallel the point array");
        outline.aliases.push((locator, contour_start));
    }
    outline.contour_ends.push(outline.points.len() - 1);
    Ok(())
}

fn push_point(
    outline: &mut CollectedOutline,
    command: usize,
    slot: Slot,
    x: f64,
    y: f64,
) -> Result<(), TextContourError> {
    outline.points.push(FtVector {
        x: to_fixed(x)?,
        y: to_fixed(y)?,
    });
    outline.locators.push(Locator { command, slot });
    Ok(())
}

fn to_fixed(value: f64) -> Result<i32, TextContourError> {
    let scaled = (value * FIXED_PER_INTERNAL_UNIT).round();
    if !scaled.is_finite() || scaled < f64::from(i32::MIN) || scaled > f64::from(i32::MAX) {
        return Err(TextContourError {
            kind: TextContourErrorKind::InvalidInput,
            path: "$.fake_style".to_owned(),
            message: "hinted outline coordinate exceeds the FreeType 26.6 range",
        });
    }
    Ok(scaled as i32)
}

fn write_back(commands: &mut [OutlineCommand], outline: &CollectedOutline) {
    for (point, locator) in outline.points.iter().zip(&outline.locators) {
        write_point(&mut commands[locator.command], locator.slot, *point);
    }
    for (locator, target) in &outline.aliases {
        write_point(
            &mut commands[locator.command],
            locator.slot,
            outline.points[*target],
        );
    }
}

fn write_point(command: &mut OutlineCommand, slot: Slot, point: FtVector) {
    let x = to_internal(point.x);
    let y = to_internal(point.y);
    match (command, slot) {
        (OutlineCommand::MoveTo(command), Slot::Move) => {
            command.x = x;
            command.y = y;
        }
        (OutlineCommand::LineTo(command), Slot::Line) => {
            command.x = x;
            command.y = y;
        }
        (OutlineCommand::QuadTo(command), Slot::QuadControl) => {
            command.control_x = x;
            command.control_y = y;
        }
        (OutlineCommand::QuadTo(command), Slot::QuadEnd) => {
            command.x = x;
            command.y = y;
        }
        (OutlineCommand::CurveTo(command), Slot::CurveControl1) => {
            command.control1_x = x;
            command.control1_y = y;
        }
        (OutlineCommand::CurveTo(command), Slot::CurveControl2) => {
            command.control2_x = x;
            command.control2_y = y;
        }
        (OutlineCommand::CurveTo(command), Slot::CurveEnd) => {
            command.x = x;
            command.y = y;
        }
        _ => unreachable!("locators are built from the same command stream"),
    }
}

fn to_internal(value: i32) -> FiniteFloat {
    FiniteFloat::try_from(f64::from(value) / FIXED_PER_INTERNAL_UNIT)
        .expect("26.6 fixed-point coordinates are finite")
}

/// The truncated 16.16 shear KiCad installs via `FT_Set_Transform`.
fn fake_italic_matrix() -> (i32, i32) {
    let angle = -std::f64::consts::PI * FAKE_ITALIC_TILT_DEGREES / 180.0;
    // The reference casts the double products straight to `FT_Fixed`,
    // truncating toward zero exactly like Rust's `as i32`.
    let xx = (angle.cos() * 65536.0) as i32;
    let xy = (-angle.sin() * 65536.0) as i32;
    (xx, xy)
}

/// `FT_MulFix` from the pinned FreeType sources.
fn mul_fix(a: i32, b: i32) -> i32 {
    let ab = i64::from(a) * i64::from(b);
    ((ab + 0x8000 - i64::from(ab < 0)) >> 16) as i32
}

fn move_sign_u64(value: i32, sign: &mut i64) -> u64 {
    if value < 0 {
        *sign = -*sign;
    }
    i64::from(value).unsigned_abs()
}

/// `FT_MulDiv` from the pinned FreeType sources with 32-bit `FT_Long`.
fn mul_div(a: i32, b: i32, c: i32) -> i32 {
    let mut sign = 1_i64;
    let a = move_sign_u64(a, &mut sign);
    let b = move_sign_u64(b, &mut sign);
    let c = move_sign_u64(c, &mut sign);
    // A zero divisor yields FreeType's saturated `0x7FFFFFFF`.
    let d = (a * b + (c >> 1)).checked_div(c).unwrap_or(0x7FFF_FFFF);
    let d = d as i32;
    if sign < 0 { d.wrapping_neg() } else { d }
}

/// `FT_MSB` from the pinned FreeType sources.
fn ft_msb(value: u32) -> i32 {
    31 - value.leading_zeros() as i32
}

/// `FT_Vector_NormLen` from the pinned FreeType sources.
///
/// Normalizes the vector to 16.16 unit length in place and returns the
/// original length, reproducing FreeType's Newton iterations including their
/// deliberate 32-bit wrap-around arithmetic.
fn vector_norm_len(vector: &mut FtVector) -> u32 {
    let (sx, mut x) = split_sign(vector.x);
    let (sy, mut y) = split_sign(vector.y);

    if x == 0 {
        if y > 0 {
            vector.y = sy * 0x10000;
        }
        return y;
    }
    if y == 0 {
        vector.x = sx * 0x10000;
        return x;
    }

    let (l, shift) = prenormalize(&mut x, &mut y);
    let (u, v) = newton_unit(x, y, l);

    vector.x = if sx < 0 { -(u as i32) } else { u as i32 };
    vector.y = if sy < 0 { -(v as i32) } else { v as i32 };

    let mut length = 0x10000_i32
        .wrapping_add((u.wrapping_mul(x).wrapping_add(v.wrapping_mul(y)) as i32) / 0x10000)
        as u32;
    if shift > 0 {
        length = (length + (1 << (shift - 1))) >> shift;
    } else {
        length <<= -shift;
    }
    length
}

/// Split one coordinate into its sign and magnitude, C `FT_ABS` style.
fn split_sign(value: i32) -> (i32, u32) {
    if value < 0 {
        (-1, i64::from(value).unsigned_abs() as u32)
    } else {
        (1, value as u32)
    }
}

/// FreeType's cheap `max + min/2` length approximation.
fn approximate_length(x: u32, y: u32) -> u32 {
    if x > y { x + (y >> 1) } else { y + (x >> 1) }
}

/// Prenormalize so the approximate length lands between 2/3 and 4/3.
fn prenormalize(x: &mut u32, y: &mut u32) -> (u32, i32) {
    let mut l = approximate_length(*x, *y);
    let mut shift: i32 = 31 - ft_msb(l);
    shift -= 15 + i32::from(l >= (0xAAAA_AAAA_u32 >> shift));
    if shift > 0 {
        *x <<= shift;
        *y <<= shift;
        l = approximate_length(*x, *y);
    } else {
        *x >>= -shift;
        *y >>= -shift;
        l >>= -shift;
    }
    (l, shift)
}

/// FreeType's Newton iterations toward the 16.16 unit vector.
fn newton_unit(x: u32, y: u32, l: u32) -> (u32, u32) {
    // Lower linear approximation for the reciprocal length minus one.
    let mut b: i32 = 0x10000 - l as i32;
    let x_signed = x as i32;
    let y_signed = y as i32;
    loop {
        let u = x_signed.wrapping_add(x_signed.wrapping_mul(b) >> 16) as u32;
        let v = y_signed.wrapping_add(y_signed.wrapping_mul(b) >> 16) as u32;
        // The squared length intentionally wraps around 2^32; converting to
        // signed recovers the difference with 2^32 exactly as in C.
        let mut z =
            (u.wrapping_mul(u).wrapping_add(v.wrapping_mul(v)) as i32).wrapping_neg() / 0x200;
        z = z.wrapping_mul((0x10000 + b) >> 8) / 0x10000;
        b += z;
        if z <= 0 {
            return (u, v);
        }
    }
}

/// `FT_Outline_Get_Orientation` from the pinned FreeType sources.
fn outline_orientation(points: &[FtVector], contour_ends: &[usize]) -> Orientation {
    if points.is_empty() {
        return Orientation::TrueType;
    }
    let (x_min, x_max, y_min, y_max) = control_box(points);
    if unusable_control_box(x_min, x_max, y_min, y_max) {
        return Orientation::None;
    }
    let xshift = (ft_msb(x_max.unsigned_abs() | x_min.unsigned_abs()) - 14).max(0);
    let yshift = (ft_msb((y_max - y_min) as u32) - 14).max(0);

    let mut area: i32 = 0;
    let mut first = 0usize;
    for &last in contour_ends {
        area = area.wrapping_add(contour_area(&points[first..=last], xshift, yshift));
        first = last + 1;
    }
    match area.cmp(&0) {
        std::cmp::Ordering::Greater => Orientation::PostScript,
        std::cmp::Ordering::Less => Orientation::TrueType,
        std::cmp::Ordering::Equal => Orientation::None,
    }
}

/// The outline's control box over every collected point.
fn control_box(points: &[FtVector]) -> (i32, i32, i32, i32) {
    let mut x_min = points[0].x;
    let mut x_max = points[0].x;
    let mut y_min = points[0].y;
    let mut y_max = points[0].y;
    for point in &points[1..] {
        x_min = x_min.min(point.x);
        x_max = x_max.max(point.x);
        y_min = y_min.min(point.y);
        y_max = y_max.max(point.y);
    }
    (x_min, x_max, y_min, y_max)
}

/// FreeType's collapsed-or-huge control-box guard for orientation analysis.
fn unusable_control_box(x_min: i32, x_max: i32, y_min: i32, y_max: i32) -> bool {
    x_min == x_max
        || y_min == y_max
        || x_min < -0x0100_0000
        || y_min < -0x0100_0000
        || x_max > 0x0100_0000
        || y_max > 0x0100_0000
}

/// One contour's shifted wrapping signed-area sum.
fn contour_area(contour: &[FtVector], xshift: i32, yshift: i32) -> i32 {
    let mut area: i32 = 0;
    let last = contour[contour.len() - 1];
    let mut prev = FtVector {
        x: last.x >> xshift,
        y: last.y >> yshift,
    };
    for point in contour {
        let current = FtVector {
            x: point.x >> xshift,
            y: point.y >> yshift,
        };
        area = area.wrapping_add(
            current
                .y
                .wrapping_sub(prev.y)
                .wrapping_mul(current.x.wrapping_add(prev.x)),
        );
        prev = current;
    }
    area
}

/// `FT_Outline_EmboldenXY` from the pinned FreeType sources.
///
/// An indeterminate orientation leaves the outline unchanged; KiCad ignores
/// the corresponding FreeType error return, and no point has moved yet.
fn embolden_xy(points: &mut [FtVector], contour_ends: &[usize], xstrength: i32, ystrength: i32) {
    let strength = FtVector {
        x: xstrength / 2,
        y: ystrength / 2,
    };
    if strength.x == 0 && strength.y == 0 {
        return;
    }
    let orientation = outline_orientation(points, contour_ends);
    if orientation == Orientation::None {
        return;
    }

    let mut first = 0usize;
    for &last in contour_ends {
        embolden_contour(&mut points[first..=last], orientation, strength);
        first = last + 1;
    }
}

/// One contour's anchor-cycle walk from `FT_Outline_EmboldenXY`.
fn embolden_contour(points: &mut [FtVector], orientation: Orientation, strength: FtVector) {
    let first: isize = 0;
    let last = points.len() as isize - 1;

    let mut in_vector = FtVector::default();
    let mut anchor = FtVector::default();
    let mut l_in: i32 = 0;
    let mut l_anchor: i32 = 0;

    // Counter j cycles through the points; counter i advances only when
    // points are moved; anchor k marks the first moved point.
    let mut i = last;
    let mut j = first;
    let mut k: isize = -1;
    while j != i && i != k {
        let out_vector;
        let l_out;
        if j != k {
            let mut out = FtVector {
                x: points[j as usize].x - points[i as usize].x,
                y: points[j as usize].y - points[i as usize].y,
            };
            let length = vector_norm_len(&mut out) as i32;
            if length == 0 {
                j = if j < last { j + 1 } else { first };
                continue;
            }
            out_vector = out;
            l_out = length;
        } else {
            out_vector = anchor;
            l_out = l_anchor;
        }

        if l_in != 0 {
            if k < 0 {
                k = i;
                anchor = in_vector;
                l_anchor = l_in;
            }

            let shift = corner_shift(
                in_vector,
                out_vector,
                l_in.min(l_out),
                orientation,
                strength,
            );
            let delta = FtVector {
                x: strength.x + shift.x,
                y: strength.y + shift.y,
            };
            i = move_points_until(points, i, j, delta);
        } else {
            i = j;
        }

        in_vector = out_vector;
        l_in = l_out;
        j = if j < last { j + 1 } else { first };
    }
}

/// Move points cyclically from `i` up to (excluding) `j` by `delta`.
fn move_points_until(points: &mut [FtVector], mut i: isize, j: isize, delta: FtVector) -> isize {
    let last = points.len() as isize - 1;
    while i != j {
        points[i as usize].x += delta.x;
        points[i as usize].y += delta.y;
        i = if i < last { i + 1 } else { 0 };
    }
    i
}

/// The corner shift for one turn of `FT_Outline_EmboldenXY`.
fn corner_shift(
    in_vector: FtVector,
    out_vector: FtVector,
    l: i32,
    orientation: Orientation,
    strength: FtVector,
) -> FtVector {
    let d = mul_fix(in_vector.x, out_vector.x) + mul_fix(in_vector.y, out_vector.y);
    // Shift only if the turn is less than roughly 160 degrees.
    if d <= -0xF000 {
        return FtVector::default();
    }
    let d = d + 0x10000;

    // Shift along the lateral bisector in proper orientation.
    let mut bisector = FtVector {
        x: in_vector.y + out_vector.y,
        y: in_vector.x + out_vector.x,
    };
    if orientation == Orientation::TrueType {
        bisector.x = -bisector.x;
    } else {
        bisector.y = -bisector.y;
    }

    // Restrict the shift magnitude for collapsing segments.
    let mut q = mul_fix(out_vector.x, in_vector.y) - mul_fix(out_vector.y, in_vector.x);
    if orientation == Orientation::TrueType {
        q = -q;
    }

    // Non-strict inequalities avoid dividing by zero when both q and l are
    // zero.
    FtVector {
        x: if mul_fix(strength.x, q) <= mul_fix(l, d) {
            mul_div(bisector.x, strength.x, d)
        } else {
            mul_div(bisector.x, l, q)
        },
        y: if mul_fix(strength.y, q) <= mul_fix(l, d) {
            mul_div(bisector.y, strength.y, d)
        } else {
            mul_div(bisector.y, l, q)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_italic_matrix_matches_the_reference_truncated_fixed_values() {
        let (xx, xy) = fake_italic_matrix();
        let angle = -std::f64::consts::PI * 12.0 / 180.0;
        assert_eq!(xx, (angle.cos() * 65536.0).trunc() as i32);
        assert_eq!(xy, (-angle.sin() * 65536.0).trunc() as i32);
        assert!(
            xx > 0x0F000 && xx < 0x10000,
            "cos(12 deg) is just below one"
        );
        assert!(xy > 0 && xy < 0x4000, "sin(12 deg) shears right");
    }

    #[test]
    fn mul_fix_rounds_half_away_from_zero_like_the_64_bit_freetype_macro() {
        assert_eq!(mul_fix(0x10000, 0x10000), 0x10000);
        assert_eq!(mul_fix(2, 0x8000), 1);
        assert_eq!(mul_fix(1, 0x8000), 1);
        // `(ab + 0x8000 - (ab < 0)) >> 16` sends -0.5 to -1 and -1.5 to -2.
        assert_eq!(mul_fix(-1, 0x8000), -1);
        assert_eq!(mul_fix(-2, 0x8000), -1);
        assert_eq!(mul_fix(-3, 0x8000), -2);
    }

    #[test]
    fn mul_div_rounds_half_away_and_saturates_on_zero_divisor() {
        assert_eq!(mul_div(3, 4, 2), 6);
        assert_eq!(mul_div(1, 1, 2), 1);
        assert_eq!(mul_div(-1, 1, 2), -1);
        assert_eq!(mul_div(5, 5, 0), 0x7FFF_FFFF);
    }

    #[test]
    fn norm_len_normalizes_axis_aligned_and_diagonal_vectors() {
        let mut v = FtVector { x: 64, y: 0 };
        assert_eq!(vector_norm_len(&mut v), 64);
        assert_eq!(v, FtVector { x: 0x10000, y: 0 });

        let mut v = FtVector { x: 0, y: -64 };
        assert_eq!(vector_norm_len(&mut v), 64);
        assert_eq!(v, FtVector { x: 0, y: -0x10000 });

        let mut v = FtVector { x: 300, y: 400 };
        let length = vector_norm_len(&mut v);
        assert!((i64::from(length) - 500).abs() <= 1, "length {length}");
        assert!((i64::from(v.x) - 0x10000 * 3 / 5).abs() <= 2);
        assert!((i64::from(v.y) - 0x10000 * 4 / 5).abs() <= 2);
    }

    #[test]
    fn orientation_reads_the_fill_side_and_flags_collapsed_outlines() {
        // Clockwise square in a y-up coordinate system: TrueType fill.
        let clockwise = [
            FtVector { x: 0, y: 0 },
            FtVector { x: 0, y: 64 },
            FtVector { x: 64, y: 64 },
            FtVector { x: 64, y: 0 },
        ];
        assert_eq!(outline_orientation(&clockwise, &[3]), Orientation::TrueType);
        let counter: Vec<FtVector> = clockwise.iter().rev().copied().collect();
        assert_eq!(outline_orientation(&counter, &[3]), Orientation::PostScript);
        let collapsed = [FtVector { x: 5, y: 0 }, FtVector { x: 5, y: 64 }];
        assert_eq!(outline_orientation(&collapsed, &[1]), Orientation::None);
    }

    #[test]
    fn embolden_grows_a_truetype_square_by_the_strength_up_and_right() {
        // TrueType-oriented square: every point moves by the halved strength
        // plus an outward corner shift, so the origin-side edges stay put and
        // the outline grows by the full strength toward +x/+y, matching
        // FreeType's documented widening with `advance += xstr`.
        let mut points = vec![
            FtVector { x: 0, y: 0 },
            FtVector { x: 0, y: 640 },
            FtVector { x: 640, y: 640 },
            FtVector { x: 640, y: 0 },
        ];
        embolden_xy(&mut points, &[3], 64, 64);
        assert_eq!(
            points,
            vec![
                FtVector { x: 0, y: 0 },
                FtVector { x: 0, y: 704 },
                FtVector { x: 704, y: 704 },
                FtVector { x: 704, y: 0 },
            ]
        );

        // A collapsed outline stays untouched, mirroring the ignored error.
        let mut degenerate = vec![FtVector { x: 0, y: 0 }, FtVector { x: 0, y: 640 }];
        let before = degenerate.clone();
        embolden_xy(&mut degenerate, &[1], 64, 64);
        assert_eq!(degenerate, before);
    }
}
