//! The int32 parameter stream: one shape (and a hole's details) per
//! operation, in the order the layout table in `lib.rs` gives.

use crate::PackError;
use kicad_monkey_contracts::generated::materialization_document::{
    ArcSegment, Backdrill, BoardSide, Contour, CubicSegment, HoleProtection, LineSegment, Nm,
    NmPoint, NonNegativeNm, PathCap, PathJoin, PathSegment, PostMachining, PostMachiningMode,
    ProtectionState, RectCorner, Shape2d, SideProtection,
};

/// Path segment tags held by one parameter word.
const TAGS_PER_WORD: usize = 16;

/// Shape kind codes, in `Shape2d` declaration order.
pub(crate) const SHAPE_KINDS: [&str; 9] = [
    "disk_2d",
    "rect_2d",
    "rounded_rect_2d",
    "chamfered_rect_2d",
    "trapezoid_2d",
    "capsule_2d",
    "annulus_2d",
    "swept_path_2d",
    "planar_region_2d",
];

pub(crate) fn shape_code(shape: &Shape2d) -> u8 {
    match shape {
        Shape2d::Disk2d(_) => 0,
        Shape2d::Rect2d(_) => 1,
        Shape2d::RoundedRect2d(_) => 2,
        Shape2d::ChamferedRect2d(_) => 3,
        Shape2d::Trapezoid2d(_) => 4,
        Shape2d::Capsule2d(_) => 5,
        Shape2d::Annulus2d(_) => 6,
        Shape2d::SweptPath2d(_) => 7,
        Shape2d::PlanarRegion2d(_) => 8,
    }
}

fn corrupt(what: &'static str) -> PackError {
    PackError::Corrupt(what)
}

/// Appends parameters; lengths must already be valid (the writer validates
/// the document first).
#[derive(Default)]
pub(crate) struct ParamWriter {
    pub(crate) values: Vec<i32>,
}

impl ParamWriter {
    fn push(&mut self, value: i32) {
        self.values.push(value);
    }

    fn length(&mut self, value: &NonNegativeNm) -> Result<(), PackError> {
        let value = i32::try_from(value.0).map_err(|_| corrupt("length outside int32"))?;
        self.push(value);
        Ok(())
    }

    fn optional_length(&mut self, value: Option<&NonNegativeNm>) -> Result<(), PackError> {
        match value {
            Some(value) => self.length(value),
            None => {
                self.push(-1);
                Ok(())
            }
        }
    }

    fn count(&mut self, count: usize) -> Result<(), PackError> {
        self.push(i32::try_from(count).map_err(|_| corrupt("count outside int32"))?);
        Ok(())
    }

    fn point(&mut self, point: &NmPoint) {
        self.push(point.0[0].0);
        self.push(point.0[1].0);
    }

    /// Start point, segment count, the segment tags packed two bits each
    /// (16 per word, first segment in the lowest bits), then each segment's
    /// points.
    fn path(&mut self, start: &NmPoint, segments: &[PathSegment]) -> Result<(), PackError> {
        self.point(start);
        self.count(segments.len())?;
        for tags in segments.chunks(TAGS_PER_WORD) {
            let word = tags
                .iter()
                .enumerate()
                .fold(0u32, |word, (position, segment)| {
                    let tag = match segment {
                        PathSegment::LineSegment(_) => 0,
                        PathSegment::ArcSegment(_) => 1,
                        PathSegment::CubicSegment(_) => 2,
                    };
                    word | (tag << (2 * position))
                });
            self.push(i32::from_le_bytes(word.to_le_bytes()));
        }
        for segment in segments {
            match segment {
                PathSegment::LineSegment(line) => self.point(&line.end_nm),
                PathSegment::ArcSegment(arc) => {
                    self.point(&arc.mid_nm);
                    self.point(&arc.end_nm);
                }
                PathSegment::CubicSegment(cubic) => {
                    self.point(&cubic.control1_nm);
                    self.point(&cubic.control2_nm);
                    self.point(&cubic.end_nm);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn shape(&mut self, shape: &Shape2d) -> Result<(), PackError> {
        match shape {
            Shape2d::Disk2d(disk) => {
                self.point(&disk.center_nm);
                self.length(&disk.radius_nm)?;
            }
            Shape2d::Rect2d(rect) => {
                self.point(&rect.center_nm);
                self.length(&rect.width_nm)?;
                self.length(&rect.height_nm)?;
            }
            Shape2d::RoundedRect2d(rect) => {
                self.point(&rect.center_nm);
                self.length(&rect.width_nm)?;
                self.length(&rect.height_nm)?;
                self.length(&rect.radius_nm)?;
            }
            Shape2d::ChamferedRect2d(rect) => {
                self.point(&rect.center_nm);
                self.length(&rect.width_nm)?;
                self.length(&rect.height_nm)?;
                self.length(&rect.chamfer_nm)?;
                self.length(&rect.radius_nm)?;
                self.count(rect.chamfered_corners.len())?;
                for corner in &rect.chamfered_corners {
                    self.push(corner_code(*corner));
                }
            }
            Shape2d::Trapezoid2d(trapezoid) => {
                self.point(&trapezoid.center_nm);
                self.length(&trapezoid.width_nm)?;
                self.length(&trapezoid.height_nm)?;
                self.push(trapezoid.delta_x_nm.0);
                self.push(trapezoid.delta_y_nm.0);
            }
            Shape2d::Capsule2d(capsule) => {
                self.point(&capsule.start_nm);
                self.point(&capsule.end_nm);
                self.length(&capsule.radius_nm)?;
            }
            Shape2d::Annulus2d(annulus) => {
                self.point(&annulus.center_nm);
                self.length(&annulus.outer_radius_nm)?;
                self.length(&annulus.inner_radius_nm)?;
            }
            Shape2d::SweptPath2d(swept) => self.swept(swept)?,
            Shape2d::PlanarRegion2d(region) => self.region(region)?,
        }
        Ok(())
    }

    fn swept(
        &mut self,
        swept: &kicad_monkey_contracts::generated::materialization_document::SweptPath2d,
    ) -> Result<(), PackError> {
        self.length(&swept.width_nm)?;
        self.push(i32::from(swept.cap == PathCap::Flat));
        self.push(i32::from(swept.join == PathJoin::Miter));
        self.path(&swept.start_nm, &swept.segments)
    }

    fn region(
        &mut self,
        region: &kicad_monkey_contracts::generated::materialization_document::PlanarRegion2d,
    ) -> Result<(), PackError> {
        self.count(region.holes.len())?;
        self.path(&region.outer.start_nm, &region.outer.segments)?;
        for hole in &region.holes {
            self.path(&hole.start_nm, &hole.segments)?;
        }
        Ok(())
    }

    /// A hole's details after its shape: end slice, plating thickness,
    /// protection, and back-drills.
    pub(crate) fn hole_details(
        &mut self,
        end_slice: i32,
        plating_thickness: Option<&NonNegativeNm>,
        protection: Option<&HoleProtection>,
        backdrills: &[(Backdrill, i32)],
    ) -> Result<(), PackError> {
        self.push(end_slice);
        self.optional_length(plating_thickness)?;
        match protection {
            None => self.push(-1),
            Some(protection) => {
                let states = [
                    protection.front.tented,
                    protection.front.covered,
                    protection.front.plugged,
                    protection.back.tented,
                    protection.back.covered,
                    protection.back.plugged,
                    protection.filled,
                    protection.capped,
                ];
                // Two bits per state, front tented in the lowest bits.
                let packed = states
                    .iter()
                    .enumerate()
                    .fold(0i32, |packed, (position, state)| {
                        packed | (state_code(*state) << (2 * position))
                    });
                self.push(packed);
            }
        }
        self.count(backdrills.len())?;
        for (drill, until_slice) in backdrills {
            self.push(i32::from(drill.side == BoardSide::Back));
            self.optional_length(drill.diameter_nm.as_ref())?;
            self.push(*until_slice);
        }
        Ok(())
    }

    /// A hole's post-machining and castellation, after its details: the
    /// count, then per entry the side, the mode, the optional diameter and
    /// depth, and the angle as a presence flag and two words of its `f64`
    /// bits; then the castellated tri-state (`-1` absent).
    pub(crate) fn hole_machining(
        &mut self,
        machining: &[PostMachining],
        castellated: Option<bool>,
    ) -> Result<(), PackError> {
        self.count(machining.len())?;
        for entry in machining {
            self.push(i32::from(entry.side == BoardSide::Back));
            self.push(i32::from(entry.mode == PostMachiningMode::Countersink));
            self.optional_length(entry.diameter_nm.as_ref())?;
            self.optional_length(entry.depth_nm.as_ref())?;
            match entry.angle_deg {
                None => self.push(0),
                Some(angle) => {
                    let bits = angle.to_bits();
                    self.push(1);
                    self.push((bits >> 32) as u32 as i32);
                    self.push(bits as u32 as i32);
                }
            }
        }
        self.push(castellated.map_or(-1, i32::from));
        Ok(())
    }
}

fn corner_code(corner: RectCorner) -> i32 {
    match corner {
        RectCorner::TopLeft => 0,
        RectCorner::TopRight => 1,
        RectCorner::BottomLeft => 2,
        RectCorner::BottomRight => 3,
    }
}

fn state_code(state: ProtectionState) -> i32 {
    match state {
        ProtectionState::Yes => 0,
        ProtectionState::No => 1,
        ProtectionState::Unspecified => 2,
    }
}

/// Reads parameters back, failing closed on truncation or bad codes.
pub(crate) struct ParamReader<'a> {
    values: &'a [i32],
    at: usize,
}

impl<'a> ParamReader<'a> {
    pub(crate) fn new(values: &'a [i32]) -> Self {
        Self { values, at: 0 }
    }

    pub(crate) fn finished(&self) -> bool {
        self.at == self.values.len()
    }

    fn next(&mut self) -> Result<i32, PackError> {
        let value = *self
            .values
            .get(self.at)
            .ok_or(corrupt("parameter stream ends early"))?;
        self.at += 1;
        Ok(value)
    }

    fn length(&mut self) -> Result<NonNegativeNm, PackError> {
        let value = self.next()?;
        if value < 0 {
            return Err(corrupt("negative length"));
        }
        Ok(NonNegativeNm(i64::from(value)))
    }

    fn optional_length(&mut self) -> Result<Option<NonNegativeNm>, PackError> {
        match self.next()? {
            -1 => Ok(None),
            value if value >= 0 => Ok(Some(NonNegativeNm(i64::from(value)))),
            _ => Err(corrupt("bad optional length")),
        }
    }

    /// A count of items each at least `per_item` parameters long, bounded by
    /// what remains so a corrupt count cannot force a large allocation.
    fn count(&mut self, per_item: usize) -> Result<usize, PackError> {
        let count = usize::try_from(self.next()?).map_err(|_| corrupt("negative count"))?;
        let remaining = self.values.len() - self.at;
        if count.saturating_mul(per_item.max(1)) > remaining {
            return Err(corrupt("count exceeds the parameter stream"));
        }
        Ok(count)
    }

    fn point(&mut self) -> Result<NmPoint, PackError> {
        Ok(NmPoint([Nm(self.next()?), Nm(self.next()?)]))
    }

    fn path(&mut self) -> Result<(NmPoint, Vec<PathSegment>), PackError> {
        let start = self.point()?;
        let count = self.count(2)?;
        let words = (0..count.div_ceil(TAGS_PER_WORD))
            .map(|_| {
                self.next()
                    .map(|word| u32::from_le_bytes(word.to_le_bytes()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut segments = Vec::with_capacity(count);
        for position in 0..count {
            let tag = (words[position / TAGS_PER_WORD] >> (2 * (position % TAGS_PER_WORD))) & 0b11;
            segments.push(match tag {
                0 => PathSegment::LineSegment(LineSegment {
                    kind: "line".to_owned(),
                    end_nm: self.point()?,
                }),
                1 => PathSegment::ArcSegment(ArcSegment {
                    kind: "arc".to_owned(),
                    mid_nm: self.point()?,
                    end_nm: self.point()?,
                }),
                2 => PathSegment::CubicSegment(CubicSegment {
                    kind: "cubic".to_owned(),
                    control1_nm: self.point()?,
                    control2_nm: self.point()?,
                    end_nm: self.point()?,
                }),
                _ => return Err(corrupt("unknown segment code")),
            });
        }
        Ok((start, segments))
    }

    fn contour(&mut self) -> Result<Contour, PackError> {
        let (start_nm, segments) = self.path()?;
        Ok(Contour { start_nm, segments })
    }

    pub(crate) fn shape(&mut self, code: u8) -> Result<Shape2d, PackError> {
        use kicad_monkey_contracts::generated::materialization_document as m;
        let kind = SHAPE_KINDS
            .get(usize::from(code))
            .ok_or(corrupt("unknown shape code"))?
            .to_string();
        Ok(match code {
            0 => Shape2d::Disk2d(m::Disk2d {
                kind,
                center_nm: self.point()?,
                radius_nm: self.length()?,
            }),
            1 => Shape2d::Rect2d(m::Rect2d {
                kind,
                center_nm: self.point()?,
                width_nm: self.length()?,
                height_nm: self.length()?,
            }),
            2 => Shape2d::RoundedRect2d(m::RoundedRect2d {
                kind,
                center_nm: self.point()?,
                width_nm: self.length()?,
                height_nm: self.length()?,
                radius_nm: self.length()?,
            }),
            3 => self.chamfered_rect(kind)?,
            4 => Shape2d::Trapezoid2d(m::Trapezoid2d {
                kind,
                center_nm: self.point()?,
                width_nm: self.length()?,
                height_nm: self.length()?,
                delta_x_nm: Nm(self.next()?),
                delta_y_nm: Nm(self.next()?),
            }),
            5 => Shape2d::Capsule2d(m::Capsule2d {
                kind,
                start_nm: self.point()?,
                end_nm: self.point()?,
                radius_nm: self.length()?,
            }),
            6 => Shape2d::Annulus2d(m::Annulus2d {
                kind,
                center_nm: self.point()?,
                outer_radius_nm: self.length()?,
                inner_radius_nm: self.length()?,
            }),
            7 => self.swept(kind)?,
            _ => self.region(kind)?,
        })
    }

    fn swept(&mut self, kind: String) -> Result<Shape2d, PackError> {
        let width_nm = self.length()?;
        let cap = if self.flag()? {
            PathCap::Flat
        } else {
            PathCap::Round
        };
        let join = if self.flag()? {
            PathJoin::Miter
        } else {
            PathJoin::Round
        };
        let (start_nm, segments) = self.path()?;
        Ok(Shape2d::SweptPath2d(
            kicad_monkey_contracts::generated::materialization_document::SweptPath2d {
                kind,
                start_nm,
                segments,
                width_nm,
                cap,
                join,
            },
        ))
    }

    fn region(&mut self, kind: String) -> Result<Shape2d, PackError> {
        let holes = self.count(3)?;
        let outer = self.contour()?;
        let holes = (0..holes)
            .map(|_| self.contour())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Shape2d::PlanarRegion2d(
            kicad_monkey_contracts::generated::materialization_document::PlanarRegion2d {
                kind,
                outer,
                holes,
            },
        ))
    }

    fn chamfered_rect(&mut self, kind: String) -> Result<Shape2d, PackError> {
        let center_nm = self.point()?;
        let width_nm = self.length()?;
        let height_nm = self.length()?;
        let chamfer_nm = self.length()?;
        let radius_nm = self.length()?;
        let count = self.count(1)?;
        let chamfered_corners = (0..count)
            .map(|_| {
                Ok(match self.next()? {
                    0 => RectCorner::TopLeft,
                    1 => RectCorner::TopRight,
                    2 => RectCorner::BottomLeft,
                    3 => RectCorner::BottomRight,
                    _ => return Err(corrupt("unknown corner code")),
                })
            })
            .collect::<Result<Vec<_>, PackError>>()?;
        Ok(Shape2d::ChamferedRect2d(
            kicad_monkey_contracts::generated::materialization_document::ChamferedRect2d {
                kind,
                center_nm,
                width_nm,
                height_nm,
                chamfer_nm,
                chamfered_corners,
                radius_nm,
            },
        ))
    }

    fn flag(&mut self) -> Result<bool, PackError> {
        match self.next()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(corrupt("bad flag")),
        }
    }

    /// A hole's details: end slice index, plating thickness, protection, and
    /// back-drills with their until-slice index (`-1` for none).
    pub(crate) fn hole_details(&mut self) -> Result<HoleDetails, PackError> {
        let end_slice = self.next()?;
        let plating_thickness = self.optional_length()?;
        let protection = match self.next()? {
            -1 => None,
            packed if packed >= 0 => Some(protection(packed)?),
            _ => return Err(corrupt("bad protection")),
        };
        let count = self.count(3)?;
        let mut backdrills = Vec::with_capacity(count);
        for _ in 0..count {
            let side = if self.flag()? {
                BoardSide::Back
            } else {
                BoardSide::Front
            };
            let diameter_nm = self.optional_length()?;
            let until_slice = self.next()?;
            backdrills.push((side, diameter_nm, until_slice));
        }
        Ok(HoleDetails {
            end_slice,
            plating_thickness,
            protection,
            backdrills,
        })
    }
}

impl ParamReader<'_> {
    /// A hole's post-machining and castellation (see the writer).
    pub(crate) fn hole_machining(
        &mut self,
    ) -> Result<(Vec<PostMachining>, Option<bool>), PackError> {
        let count = self.count(5)?;
        let mut machining = Vec::with_capacity(count);
        for _ in 0..count {
            let side = if self.flag()? {
                BoardSide::Back
            } else {
                BoardSide::Front
            };
            let mode = if self.flag()? {
                PostMachiningMode::Countersink
            } else {
                PostMachiningMode::Counterbore
            };
            let diameter_nm = self.optional_length()?;
            let depth_nm = self.optional_length()?;
            let angle_deg = if self.flag()? {
                let high = u64::from(self.next()? as u32);
                let low = u64::from(self.next()? as u32);
                Some(f64::from_bits((high << 32) | low))
            } else {
                None
            };
            machining.push(PostMachining {
                side,
                mode,
                diameter_nm,
                depth_nm,
                angle_deg,
            });
        }
        let castellated = match self.next()? {
            -1 => None,
            0 => Some(false),
            1 => Some(true),
            _ => return Err(corrupt("bad castellated flag")),
        };
        Ok((machining, castellated))
    }
}

/// A hole's details as read, with slice indices still unresolved.
pub(crate) struct HoleDetails {
    pub(crate) end_slice: i32,
    pub(crate) plating_thickness: Option<NonNegativeNm>,
    pub(crate) protection: Option<HoleProtection>,
    pub(crate) backdrills: Vec<(BoardSide, Option<NonNegativeNm>, i32)>,
}

fn protection(packed: i32) -> Result<HoleProtection, PackError> {
    let state = |position: usize| -> Result<ProtectionState, PackError> {
        match (packed >> (2 * position)) & 0b11 {
            0 => Ok(ProtectionState::Yes),
            1 => Ok(ProtectionState::No),
            2 => Ok(ProtectionState::Unspecified),
            _ => Err(corrupt("bad protection state")),
        }
    };
    if packed >> 16 != 0 {
        return Err(corrupt("bad protection"));
    }
    Ok(HoleProtection {
        front: SideProtection {
            tented: state(0)?,
            covered: state(1)?,
            plugged: state(2)?,
        },
        back: SideProtection {
            tented: state(3)?,
            covered: state(4)?,
            plugged: state(5)?,
        },
        filled: state(6)?,
        capped: state(7)?,
    })
}
