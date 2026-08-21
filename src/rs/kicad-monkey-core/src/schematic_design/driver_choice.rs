use crate::{SchematicDriverPriority, SchematicWireDriverKind};

pub(super) struct DriverChoice {
    pub(super) priority: SchematicDriverPriority,
    pub(super) depth: usize,
    pub(super) shape_rank: usize,
    pub(super) implicit: bool,
    pub(super) full_name: String,
    pub(super) sheet_path: String,
    pub(super) order: usize,
    pub(super) kind: SchematicWireDriverKind,
    pub(super) raw_name: String,
    pub(super) sheet_pin_key: Option<(usize, usize)>,
}

impl DriverChoice {
    fn precedes(&self, other: &Self) -> bool {
        (
            std::cmp::Reverse(self.priority),
            self.depth,
            self.shape_rank,
            self.implicit,
            &self.full_name,
            &self.sheet_path,
            self.order,
        ) < (
            std::cmp::Reverse(other.priority),
            other.depth,
            other.shape_rank,
            other.implicit,
            &other.full_name,
            &other.sheet_path,
            other.order,
        )
    }
}

pub(super) fn consider_choice(best: &mut Option<DriverChoice>, candidate: DriverChoice) {
    if best
        .as_ref()
        .is_none_or(|current| candidate.precedes(current))
    {
        *best = Some(candidate);
    }
}

pub(super) fn sheet_depth(path: &str) -> usize {
    path.bytes().filter(|value| *value == b'/').count()
}

pub(super) fn checked_join(left: &str, right: &str, maximum: usize) -> Option<String> {
    let bytes = left.len().checked_add(right.len())?;
    if bytes > maximum {
        return None;
    }
    let mut value = String::with_capacity(bytes);
    value.push_str(left);
    value.push_str(right);
    Some(value)
}
