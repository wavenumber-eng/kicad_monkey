use super::SchematicSheet;

#[derive(Clone, Debug, Default)]
pub(super) struct SheetParseState {
    pub(super) property_count: usize,
    pub(super) uuid_seen: bool,
    pub(super) sheet_name_seen: bool,
    pub(super) sheet_file_seen: bool,
    pub(super) in_bom_seen: bool,
    pub(super) on_board_seen: bool,
    pub(super) dnp_seen: bool,
    pub(super) exclude_from_sim_seen: bool,
}

pub(super) fn default_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: String::new(),
        sheet_name: String::new(),
        sheet_file: String::new(),
        in_bom: true,
        on_board: true,
        dnp: false,
        exclude_from_sim: false,
        properties: Vec::new(),
        pins: Vec::new(),
        page_instances: Vec::new(),
    }
}
