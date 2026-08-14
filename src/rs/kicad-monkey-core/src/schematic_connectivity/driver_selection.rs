use super::{
    SchematicDriverPriority, SchematicLabelDriver, SchematicPinDriver, SchematicWireDriverKind,
};

pub(super) fn resolve_driver(
    labels: &[SchematicLabelDriver],
    pins: &[SchematicPinDriver],
) -> (
    String,
    SchematicDriverPriority,
    Option<SchematicWireDriverKind>,
) {
    let mut best: Option<DriverChoice> = None;
    for (index, label) in labels.iter().enumerate() {
        consider(
            &mut best,
            DriverChoice {
                priority: label.priority,
                implicit: false,
                name: label.text.clone(),
                order: index,
                kind: label.kind,
            },
        );
    }
    let pin_offset = labels.len();
    for (index, pin) in pins.iter().enumerate() {
        let display = if pin.is_power && !pin.power_value.is_empty() {
            pin.power_value.clone()
        } else {
            format!("{}-{}", pin.reference, pin.pin_number)
        };
        consider(
            &mut best,
            DriverChoice {
                priority: pin.priority,
                implicit: pin.is_implicit_hidden_power,
                name: display,
                order: pin_offset + index,
                kind: pin.kind,
            },
        );
    }
    best.map_or_else(
        || (String::new(), SchematicDriverPriority::None, None),
        |choice| (choice.name, choice.priority, Some(choice.kind)),
    )
}

struct DriverChoice {
    priority: SchematicDriverPriority,
    implicit: bool,
    name: String,
    order: usize,
    kind: SchematicWireDriverKind,
}

fn consider(best: &mut Option<DriverChoice>, candidate: DriverChoice) {
    if best
        .as_ref()
        .is_none_or(|current| candidate_precedes(&candidate, current))
    {
        *best = Some(candidate);
    }
}

fn candidate_precedes(candidate: &DriverChoice, current: &DriverChoice) -> bool {
    candidate.priority > current.priority
        || (candidate.priority == current.priority
            && ((!candidate.implicit && current.implicit)
                || (candidate.implicit == current.implicit
                    && (candidate.name < current.name
                        || (candidate.name == current.name && candidate.order < current.order)))))
}
