use super::SchematicWireDriverKind;
use crate::{SchematicDriverPriority, SchematicLabelScope};

pub(super) fn label_type(
    scope: SchematicLabelScope,
) -> (SchematicDriverPriority, SchematicWireDriverKind) {
    match scope {
        SchematicLabelScope::Local => (
            SchematicDriverPriority::LocalLabel,
            SchematicWireDriverKind::LocalLabel,
        ),
        SchematicLabelScope::Global => (
            SchematicDriverPriority::Global,
            SchematicWireDriverKind::GlobalLabel,
        ),
        SchematicLabelScope::Hierarchical => (
            SchematicDriverPriority::HierarchicalLabel,
            SchematicWireDriverKind::HierarchicalLabel,
        ),
    }
}

pub(super) fn pin_kind(priority: SchematicDriverPriority) -> SchematicWireDriverKind {
    match priority {
        SchematicDriverPriority::GlobalPowerPin => SchematicWireDriverKind::GlobalPowerPin,
        SchematicDriverPriority::LocalPowerPin => SchematicWireDriverKind::LocalPowerPin,
        _ => SchematicWireDriverKind::Pin,
    }
}
