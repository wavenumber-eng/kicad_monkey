use crate::{SchematicLabelDriver, SchematicPinDriver, SchematicWireSubgraph};

pub(super) struct MergedDriverShape {
    pub(super) pins: usize,
    pub(super) labels: usize,
    pub(super) bytes: usize,
}

pub(super) fn merged_driver_shape<'a>(
    subgraphs: impl Iterator<Item = &'a SchematicWireSubgraph>,
    choice_strings: Option<[&str; 3]>,
) -> Option<MergedDriverShape> {
    let mut pins = 0_usize;
    let mut labels = 0_usize;
    let mut string_bytes = 0_usize;
    for subgraph in subgraphs {
        pins = pins.checked_add(subgraph.pin_drivers.len())?;
        labels = labels.checked_add(subgraph.label_drivers.len())?;
        for pin in &subgraph.pin_drivers {
            string_bytes = [
                pin.symbol_uuid.len(),
                pin.reference.len(),
                pin.pin_number.len(),
                pin.pin_name.len(),
                pin.electrical_type.len(),
                pin.power_value.len(),
                pin.designator_with_unit.len(),
                pin.source_pin_uuid.len(),
                pin.pin_svg_id.len(),
            ]
            .into_iter()
            .try_fold(string_bytes, usize::checked_add)?;
        }
        for label in &subgraph.label_drivers {
            string_bytes = [
                label.text.len(),
                label.shape.len(),
                label.source_uuid.len(),
                label.render_id.len(),
            ]
            .into_iter()
            .try_fold(string_bytes, usize::checked_add)?;
        }
    }
    if let Some(choice_strings) = choice_strings {
        string_bytes = choice_strings
            .into_iter()
            .map(str::len)
            .try_fold(string_bytes, usize::checked_add)?;
    }
    let bytes = pins
        .checked_mul(std::mem::size_of::<SchematicPinDriver>())?
        .checked_add(labels.checked_mul(std::mem::size_of::<SchematicLabelDriver>())?)?
        .checked_add(string_bytes)?;
    Some(MergedDriverShape {
        pins,
        labels,
        bytes,
    })
}
