use crate::{SchematicDefinition, SchematicPoint, SourceBundleError, SourceBundleErrorKind};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Default)]
pub(super) struct WirePointUnion {
    index_by_point: HashMap<SchematicPoint, usize>,
    points: Vec<SchematicPoint>,
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl WirePointUnion {
    pub(super) fn point_count(&self) -> usize {
        self.points.len()
    }

    pub(super) fn add(
        &mut self,
        point: SchematicPoint,
        max_points: usize,
        definition: &SchematicDefinition,
    ) -> Result<usize, SourceBundleError> {
        if let Some(index) = self.index_by_point.get(&point) {
            return Ok(*index);
        }
        if self.points.len() >= max_points {
            return Err(limit_error(
                definition,
                "schematic wire graph point count exceeds its limit",
            ));
        }
        let index = self.points.len();
        self.index_by_point.insert(point, index);
        self.points.push(point);
        self.parent.push(index);
        self.rank.push(0);
        Ok(index)
    }

    fn root(&mut self, index: usize) -> usize {
        let mut root = index;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut current = index;
        while self.parent[current] != root {
            let next = self.parent[current];
            self.parent[current] = root;
            current = next;
        }
        root
    }

    pub(super) fn root_for_point(&mut self, point: SchematicPoint) -> Option<usize> {
        self.index_by_point
            .get(&point)
            .copied()
            .map(|index| self.root(index))
    }

    pub(super) fn union_points(&mut self, left: SchematicPoint, right: SchematicPoint) {
        if let (Some(left), Some(right)) = (
            self.index_by_point.get(&left).copied(),
            self.index_by_point.get(&right).copied(),
        ) {
            self.union(left, right);
        }
    }

    pub(super) fn union(&mut self, left: usize, right: usize) {
        let mut left_root = self.root(left);
        let mut right_root = self.root(right);
        if left_root == right_root {
            return;
        }
        if self.rank[left_root] < self.rank[right_root] {
            std::mem::swap(&mut left_root, &mut right_root);
        }
        self.parent[right_root] = left_root;
        if self.rank[left_root] == self.rank[right_root] {
            self.rank[left_root] = self.rank[left_root].saturating_add(1);
        }
    }

    pub(super) fn ensure_group_limit(
        &mut self,
        max_groups: usize,
        definition: &SchematicDefinition,
    ) -> Result<(), SourceBundleError> {
        let mut roots = HashSet::new();
        for index in 0..self.points.len() {
            let root = self.root(index);
            if !roots.contains(&root) && roots.len() >= max_groups {
                return Err(limit_error(
                    definition,
                    "schematic wire subgraph count exceeds its limit",
                ));
            }
            roots.insert(root);
        }
        Ok(())
    }

    pub(super) fn groups(
        &mut self,
        max_groups: usize,
        definition: &SchematicDefinition,
    ) -> Result<Vec<(usize, Vec<SchematicPoint>)>, SourceBundleError> {
        let mut groups = BTreeMap::<usize, Vec<SchematicPoint>>::new();
        for index in 0..self.points.len() {
            let root = self.root(index);
            if !groups.contains_key(&root) && groups.len() >= max_groups {
                return Err(limit_error(
                    definition,
                    "schematic wire subgraph count exceeds its limit",
                ));
            }
            groups.entry(root).or_default().push(self.points[index]);
        }
        Ok(groups.into_iter().collect())
    }
}

fn limit_error(definition: &SchematicDefinition, message: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        Some(&definition.source_path),
        message,
    )
}
