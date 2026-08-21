use super::*;

impl PcbCounts {
    pub(super) fn retain_selection(&mut self, selection: PcbSelection) {
        self.retain_primary_selection(selection);
        self.retain_extended_selection(selection);
    }

    fn retain_primary_selection(&mut self, selection: PcbSelection) {
        self.retain_board_selection(selection);
        self.retain_footprint_selection(selection);
        self.retain_routing_selection(selection);
    }

    fn retain_board_selection(&mut self, selection: PcbSelection) {
        if !selection.contains(PcbFamily::Layers) {
            self.layers = 0;
        }
        if !selection.contains(PcbFamily::Nets) {
            self.nets = 0;
        }
        if !selection.contains(PcbFamily::Properties) {
            self.properties = 0;
        }
        if !selection.contains(PcbFamily::Footprints) {
            self.footprints = 0;
        }
    }

    fn retain_footprint_selection(&mut self, selection: PcbSelection) {
        if !selection.contains(PcbFamily::Pads) {
            self.pads = 0;
        }
        if !selection.contains(PcbFamily::Models) {
            self.models = 0;
        }
        if !selection.contains(PcbFamily::FootprintProperties) {
            self.footprint_properties = 0;
        }
        if !selection.contains(PcbFamily::FootprintGraphics) {
            self.footprint_graphics = 0;
        }
        if !selection.contains(PcbFamily::FootprintTexts) {
            self.footprint_texts = 0;
        }
        if !selection.contains(PcbFamily::FootprintTextBoxes) {
            self.footprint_text_boxes = 0;
        }
    }

    fn retain_routing_selection(&mut self, selection: PcbSelection) {
        if !selection.contains(PcbFamily::Segments) {
            self.segments = 0;
        }
        if !selection.contains(PcbFamily::Vias) {
            self.vias = 0;
        }
        if !selection.contains(PcbFamily::Zones) {
            self.zones = 0;
        }
        if !selection.contains(PcbFamily::Arcs) {
            self.arcs = 0;
        }
    }

    fn retain_extended_selection(&mut self, selection: PcbSelection) {
        if !selection.contains(PcbFamily::Graphics) {
            self.graphics = 0;
            self.gr_texts = 0;
            self.gr_lines = 0;
            self.gr_rects = 0;
            self.gr_arcs = 0;
            self.gr_circles = 0;
            self.gr_polys = 0;
            self.gr_curves = 0;
            self.gr_text_boxes = 0;
        }
        if !selection.contains(PcbFamily::Images) {
            self.images = 0;
        }
        if !selection.contains(PcbFamily::Barcodes) {
            self.barcodes = 0;
        }
        if !selection.contains(PcbFamily::Tables) {
            self.tables = 0;
            self.table_cells = 0;
        }
        if !selection.contains(PcbFamily::Groups) {
            self.groups = 0;
        }
        if !selection.contains(PcbFamily::Dimensions) {
            self.dimensions = 0;
        }
        if !selection.contains(PcbFamily::GeneratedItems) {
            self.generated_items = 0;
        }
        if !selection.contains(PcbFamily::EmbeddedFiles) {
            self.embedded_files = 0;
        }
        if !selection.contains(PcbFamily::Variants) {
            self.variants = 0;
        }
    }
}
