use super::*;

impl Validation {
    pub(super) fn groups(&mut self, board: &AuthoredPcb) -> Result<(), Error> {
        if board.groups.is_empty() {
            return Ok(());
        }
        let roots = root_identities(board);
        let mut groups = BTreeSet::new();
        for group in &board.groups {
            self.object()?;
            self.identity(&group.uuid)?;
            self.text(&group.name)?;
            if group.members.is_empty() {
                return Err(invalid("KiCad discards empty editor groups"));
            }
            if group.members.len() > self.limits.pcb_limits.max_members {
                return Err(limit());
            }
            groups.insert(group.uuid.to_ascii_lowercase());
        }
        let mut parents = BTreeMap::new();
        for group in &board.groups {
            let parent = group.uuid.to_ascii_lowercase();
            for member in &group.members {
                self.text(member)?;
                let member = member.to_ascii_lowercase();
                if !roots.contains(&member) && !groups.contains(&member) {
                    return Err(invalid(
                        "group member must identify a board-root item or group",
                    ));
                }
                if parents.insert(member, parent.clone()).is_some() {
                    return Err(invalid("an item may occur in only one group, once"));
                }
            }
        }
        reject_cycles(&groups, &parents)
    }
}

fn root_identities(board: &AuthoredPcb) -> BTreeSet<String> {
    board
        .profile
        .iter()
        .chain(&board.graphics)
        .map(|v| &v.uuid)
        .chain(board.texts.iter().map(|v| &v.uuid))
        .chain(board.text_boxes.iter().map(|v| &v.uuid))
        .chain(board.footprints.iter().map(|v| &v.uuid))
        .chain(board.vias.iter().map(|v| &v.uuid))
        .chain(board.segments.iter().map(|v| &v.uuid))
        .chain(board.arcs.iter().map(|v| &v.uuid))
        .chain(board.zones.iter().map(|v| &v.uuid))
        .chain(board.rule_areas.iter().map(|v| &v.uuid))
        .map(|id| id.to_ascii_lowercase())
        .collect()
}

// Every item has at most one parent. Walk that forest without recursion and
// memoize completed paths, avoiding repeated walks of shared ancestry.
fn reject_cycles(
    groups: &BTreeSet<String>,
    parents: &BTreeMap<String, String>,
) -> Result<(), Error> {
    let mut done = BTreeSet::new();
    for group in groups {
        let mut path = BTreeSet::new();
        let mut current = group;
        while !done.contains(current) {
            if !path.insert(current) {
                return Err(invalid("editor group membership must not contain cycles"));
            }
            let Some(parent) = parents.get(current) else {
                break;
            };
            current = parent;
        }
        done.extend(path);
    }
    Ok(())
}
