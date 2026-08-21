use super::*;

#[derive(Default)]
struct SourceGraphicalIds<'a> {
    wires: BorrowedGraphicalIds<'a>,
    junctions: BorrowedGraphicalIds<'a>,
}

#[derive(Default)]
struct BorrowedGraphicalIds<'a> {
    values: Vec<&'a str>,
    seen: HashSet<&'a str>,
}

impl<'a> BorrowedGraphicalIds<'a> {
    fn insert(&mut self, value: &'a str) {
        if !value.is_empty() && self.seen.insert(value) {
            self.values.push(value);
        }
    }
}

impl OccurrenceBuilder<'_> {
    pub(super) fn index_source_graphics(
        &mut self,
        union: &mut WirePointUnion,
    ) -> Result<HashMap<usize, SchematicGraphicalIds>, SourceBundleError> {
        let mut borrowed = HashMap::<usize, SourceGraphicalIds<'_>>::new();
        for wire in &self.definition.wires {
            let Some(point) = wire.points.first().copied() else {
                continue;
            };
            let Some(root) = union.root_for_point(point) else {
                continue;
            };
            borrowed.entry(root).or_default().wires.insert(&wire.uuid);
        }
        for junction in &self.definition.junctions {
            let Some(root) = union.root_for_point(junction.at) else {
                continue;
            };
            borrowed
                .entry(root)
                .or_default()
                .junctions
                .insert(&junction.uuid);
        }
        let mut by_root = HashMap::with_capacity(borrowed.len());
        for (root, source) in borrowed {
            let mut graphical = SchematicGraphicalIds::default();
            self.append_graphical_ids(&mut graphical.wires, source.wires.values)?;
            self.append_graphical_ids(&mut graphical.junctions, source.junctions.values)?;
            by_root.insert(root, graphical);
        }
        Ok(by_root)
    }

    pub(super) fn attach_driver_graphics(
        &mut self,
        graphical: &mut SchematicGraphicalIds,
        pins: &[SchematicPinDriver],
        labels: &[SchematicLabelDriver],
    ) -> Result<(), SourceBundleError> {
        let mut sheet_entries = BorrowedGraphicalIds::default();
        let mut ports = BorrowedGraphicalIds::default();
        let mut label_ids = BorrowedGraphicalIds::default();
        for label in labels {
            let bucket = match label.kind {
                SchematicWireDriverKind::SheetPin => &mut sheet_entries,
                SchematicWireDriverKind::HierarchicalLabel => &mut ports,
                SchematicWireDriverKind::LocalLabel | SchematicWireDriverKind::GlobalLabel => {
                    &mut label_ids
                }
                _ => continue,
            };
            bucket.insert(label.render_id.as_str());
        }
        let mut power_ports = BorrowedGraphicalIds::default();
        for pin in pins
            .iter()
            .filter(|pin| pin.is_power && pin.reference.starts_with('#'))
        {
            power_ports.insert(&pin.symbol_uuid);
        }
        self.append_graphical_ids(&mut graphical.sheet_entries, sheet_entries.values)?;
        self.append_graphical_ids(&mut graphical.ports, ports.values)?;
        self.append_graphical_ids(&mut graphical.labels, label_ids.values)?;
        self.append_graphical_ids(&mut graphical.power_ports, power_ports.values)?;
        Ok(())
    }

    fn append_graphical_ids(
        &mut self,
        bucket: &mut Vec<String>,
        values: Vec<&str>,
    ) -> Result<(), SourceBundleError> {
        let existing = bucket.iter().map(String::as_str).collect::<HashSet<_>>();
        let additions = values
            .into_iter()
            .filter(|value| !existing.contains(value))
            .collect::<Vec<_>>();
        let bytes = additions.iter().try_fold(0_usize, |total, value| {
            total
                .checked_add(value.len())
                .ok_or_else(|| self.limit_error("connectivity retained string bytes overflow"))
        })?;
        self.retain_string_bytes(bytes)?;
        bucket.extend(additions.into_iter().map(str::to_owned));
        Ok(())
    }
}
