use crate::{SchematicPoint, SourceBundleError, SourceBundleErrorKind};
use std::cmp::Ordering;

const LEAF_SEGMENTS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SchematicSegment {
    pub a: SchematicPoint,
    pub b: SchematicPoint,
    pub ordinal: usize,
}

impl SchematicSegment {
    pub(crate) fn new(a: SchematicPoint, b: SchematicPoint, ordinal: usize) -> Option<Self> {
        (a != b).then_some(Self { a, b, ordinal })
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

impl Bounds {
    fn from_segment(segment: SchematicSegment) -> Self {
        Self {
            min_x: segment.a.x_iu.min(segment.b.x_iu),
            min_y: segment.a.y_iu.min(segment.b.y_iu),
            max_x: segment.a.x_iu.max(segment.b.x_iu),
            max_y: segment.a.y_iu.max(segment.b.y_iu),
        }
    }

    fn include(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn contains(self, point: SchematicPoint) -> bool {
        self.min_x <= point.x_iu
            && point.x_iu <= self.max_x
            && self.min_y <= point.y_iu
            && point.y_iu <= self.max_y
    }

    fn split_on_x(self) -> bool {
        i128::from(self.max_x) - i128::from(self.min_x)
            >= i128::from(self.max_y) - i128::from(self.min_y)
    }
}

#[derive(Clone, Copy, Debug)]
enum NodeKind {
    Leaf { begin: usize, end: usize },
    Branch { left: usize, right: usize },
}

#[derive(Clone, Copy, Debug)]
struct Node {
    bounds: Bounds,
    kind: NodeKind,
}

/// Static bounding-volume index for exact point-on-segment queries.
///
/// Segment storage is partitioned in place, so each segment is retained once.
/// Queries prune non-containing bounding boxes before performing checked exact
/// integer collinearity tests. Pathological overlapping boxes are controlled by
/// the caller's aggregate query-work budget.
#[derive(Debug)]
pub(crate) struct SchematicSegmentIndex {
    segments: Vec<SchematicSegment>,
    nodes: Vec<Node>,
    root: Option<usize>,
    source_path: String,
    max_nodes: usize,
}

impl SchematicSegmentIndex {
    pub(crate) fn build(
        segments: Vec<SchematicSegment>,
        max_nodes: usize,
        source_path: &str,
    ) -> Result<Self, SourceBundleError> {
        let mut index = Self {
            segments,
            nodes: Vec::new(),
            root: None,
            source_path: source_path.to_owned(),
            max_nodes,
        };
        if !index.segments.is_empty() {
            index.root = Some(index.build_node(0, index.segments.len())?);
        }
        Ok(index)
    }

    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[cfg(test)]
    pub(crate) fn containing(
        &self,
        point: SchematicPoint,
        work: &mut usize,
        max_work: usize,
    ) -> Result<Vec<SchematicSegment>, SourceBundleError> {
        let mut matches = Vec::new();
        self.visit_containing(point, work, max_work, |segment| {
            matches.push(segment);
            true
        })?;
        Ok(matches)
    }

    pub(crate) fn any_containing(
        &self,
        point: SchematicPoint,
        work: &mut usize,
        max_work: usize,
    ) -> Result<bool, SourceBundleError> {
        let mut found = false;
        self.visit_containing(point, work, max_work, |_| {
            found = true;
            false
        })?;
        Ok(found)
    }

    pub(crate) fn first_containing(
        &self,
        point: SchematicPoint,
        work: &mut usize,
        max_work: usize,
    ) -> Result<Option<SchematicSegment>, SourceBundleError> {
        let mut first = None;
        self.visit_containing(point, work, max_work, |segment| {
            if first.is_none_or(|current: SchematicSegment| segment.ordinal < current.ordinal) {
                first = Some(segment);
            }
            true
        })?;
        Ok(first)
    }

    pub(crate) fn for_each_containing(
        &self,
        point: SchematicPoint,
        work: &mut usize,
        max_work: usize,
        visitor: impl FnMut(SchematicSegment) -> bool,
    ) -> Result<(), SourceBundleError> {
        self.visit_containing(point, work, max_work, visitor)
    }

    fn visit_containing(
        &self,
        point: SchematicPoint,
        work: &mut usize,
        max_work: usize,
        mut visitor: impl FnMut(SchematicSegment) -> bool,
    ) -> Result<(), SourceBundleError> {
        let Some(root) = self.root else {
            return Ok(());
        };
        let mut stack = vec![root];
        while let Some(node_index) = stack.pop() {
            charge_work(work, 1, max_work, &self.source_path)?;
            let node = self.nodes[node_index];
            if !node.bounds.contains(point) {
                continue;
            }
            match node.kind {
                NodeKind::Leaf { begin, end } => {
                    for segment in &self.segments[begin..end] {
                        charge_work(work, 1, max_work, &self.source_path)?;
                        if point_on_segment(point, segment.a, segment.b, &self.source_path)?
                            && !visitor(*segment)
                        {
                            return Ok(());
                        }
                    }
                }
                NodeKind::Branch { left, right } => {
                    stack.push(right);
                    stack.push(left);
                }
            }
        }
        Ok(())
    }

    fn build_node(&mut self, begin: usize, end: usize) -> Result<usize, SourceBundleError> {
        if self.nodes.len() >= self.max_nodes {
            return Err(limit_error(
                &self.source_path,
                "schematic segment index node count exceeds its limit",
            ));
        }
        let bounds = segment_bounds(&self.segments[begin..end]);
        let node_index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            kind: NodeKind::Leaf { begin, end },
        });
        if end - begin <= LEAF_SEGMENTS {
            return Ok(node_index);
        }
        let middle = begin + (end - begin) / 2;
        let split_on_x = bounds.split_on_x();
        self.segments[begin..end].select_nth_unstable_by(middle - begin, |left, right| {
            compare_midpoint(*left, *right, split_on_x)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });
        let left = self.build_node(begin, middle)?;
        let right = self.build_node(middle, end)?;
        self.nodes[node_index].kind = NodeKind::Branch { left, right };
        Ok(node_index)
    }
}

fn segment_bounds(segments: &[SchematicSegment]) -> Bounds {
    let mut bounds = Bounds::from_segment(segments[0]);
    for segment in &segments[1..] {
        bounds = bounds.include(Bounds::from_segment(*segment));
    }
    bounds
}

fn compare_midpoint(left: SchematicSegment, right: SchematicSegment, split_on_x: bool) -> Ordering {
    let midpoint = |segment: SchematicSegment| {
        if split_on_x {
            i128::from(segment.a.x_iu) + i128::from(segment.b.x_iu)
        } else {
            i128::from(segment.a.y_iu) + i128::from(segment.b.y_iu)
        }
    };
    midpoint(left).cmp(&midpoint(right))
}

fn point_on_segment(
    point: SchematicPoint,
    a: SchematicPoint,
    b: SchematicPoint,
    source_path: &str,
) -> Result<bool, SourceBundleError> {
    if point.x_iu < a.x_iu.min(b.x_iu)
        || point.x_iu > a.x_iu.max(b.x_iu)
        || point.y_iu < a.y_iu.min(b.y_iu)
        || point.y_iu > a.y_iu.max(b.y_iu)
    {
        return Ok(false);
    }
    let ab_x = i128::from(b.x_iu) - i128::from(a.x_iu);
    let ab_y = i128::from(b.y_iu) - i128::from(a.y_iu);
    let ap_x = i128::from(point.x_iu) - i128::from(a.x_iu);
    let ap_y = i128::from(point.y_iu) - i128::from(a.y_iu);
    let left = ab_x
        .checked_mul(ap_y)
        .ok_or_else(|| geometry_error(source_path, "segment cross product overflows"))?;
    let right = ab_y
        .checked_mul(ap_x)
        .ok_or_else(|| geometry_error(source_path, "segment cross product overflows"))?;
    Ok(left == right)
}

fn charge_work(
    work: &mut usize,
    amount: usize,
    max_work: usize,
    source_path: &str,
) -> Result<(), SourceBundleError> {
    *work = work
        .checked_add(amount)
        .ok_or_else(|| limit_error(source_path, "schematic segment query work overflows"))?;
    if *work > max_work {
        return Err(limit_error(
            source_path,
            "schematic segment query work exceeds its limit",
        ));
    }
    Ok(())
}

fn limit_error(source_path: &str, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        Some(source_path),
        message,
    )
}

fn geometry_error(source_path: &str, message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::Schematic, Some(source_path), message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: i64, y: i64) -> SchematicPoint {
        SchematicPoint { x_iu: x, y_iu: y }
    }

    #[test]
    fn exact_index_handles_axis_aligned_diagonal_and_source_ordinals() {
        let index = SchematicSegmentIndex::build(
            vec![
                SchematicSegment::new(point(0, 0), point(10, 0), 2).expect("horizontal"),
                SchematicSegment::new(point(0, 0), point(10, 10), 1).expect("diagonal"),
                SchematicSegment::new(point(5, -5), point(5, 5), 0).expect("vertical"),
            ],
            8,
            "test.kicad_sch",
        )
        .expect("segment index");
        let mut work = 0;
        let mut ordinals = index
            .containing(point(5, 0), &mut work, 32)
            .expect("point query")
            .into_iter()
            .map(|segment| segment.ordinal)
            .collect::<Vec<_>>();
        ordinals.sort_unstable();
        assert_eq!(ordinals, [0, 2]);
        assert!(
            index
                .containing(point(5, 5), &mut work, 64)
                .expect("diagonal query")
                .iter()
                .any(|segment| segment.ordinal == 1)
        );
    }

    #[test]
    fn index_node_and_query_work_limits_fail_closed() {
        let segments =
            vec![SchematicSegment::new(point(0, 0), point(10, 0), 0).expect("segment"); 9];
        assert_eq!(
            SchematicSegmentIndex::build(segments.clone(), 1, "test.kicad_sch")
                .expect_err("node limit")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
        let index =
            SchematicSegmentIndex::build(segments, 3, "test.kicad_sch").expect("bounded index");
        let mut work = 0;
        assert!(
            index
                .any_containing(point(5, 0), &mut work, 3)
                .expect("exact query work")
        );
        assert_eq!(work, 3);
        let mut work = 0;
        assert_eq!(
            index
                .any_containing(point(5, 0), &mut work, 2)
                .expect_err("query limit")
                .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }

    #[test]
    fn exact_cross_product_overflow_is_reported_instead_of_wrapping() {
        let index = SchematicSegmentIndex::build(
            vec![
                SchematicSegment::new(point(i64::MIN, i64::MIN), point(i64::MAX, i64::MAX - 1), 0)
                    .expect("extreme segment"),
            ],
            1,
            "test.kicad_sch",
        )
        .expect("index");
        let mut work = 0;
        assert_eq!(
            index
                .containing(point(i64::MAX - 1, i64::MAX - 1), &mut work, 4)
                .expect_err("checked cross product")
                .kind,
            SourceBundleErrorKind::Schematic
        );
    }
}
