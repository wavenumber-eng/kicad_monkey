//! Dependency-aware PCB family selection.

/// One independently requestable native PCB model family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PcbFamily {
    Layers,
    Setup,
    Nets,
    Properties,
    Footprints,
    FootprintProperties,
    FootprintGraphics,
    Pads,
    Models,
    Segments,
    Vias,
    Zones,
    Arcs,
    Graphics,
    Dimensions,
    Groups,
    GeneratedItems,
    EmbeddedFiles,
    Variants,
    Images,
    Barcodes,
    Tables,
    Holes,
    FootprintTransforms,
    Profile,
}

/// A compact, allocation-free set of requested PCB families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcbSelection(u32);

impl PcbSelection {
    const FAMILY_COUNT: u32 = PcbFamily::Profile as u32 + 1;
    const ALL_MASK: u32 = (1 << Self::FAMILY_COUNT) - 1;

    pub const fn none() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::ALL_MASK)
    }

    pub const fn only(family: PcbFamily) -> Self {
        Self(family_mask(family))
    }

    #[must_use]
    pub const fn with(self, family: PcbFamily) -> Self {
        Self(self.0 | family_mask(family))
    }

    pub const fn contains(self, family: PcbFamily) -> bool {
        self.0 & family_mask(family) != 0
    }

    pub(super) const fn dependencies(self) -> Self {
        let mut result = self;
        if self.contains(PcbFamily::Pads)
            || self.contains(PcbFamily::Models)
            || self.contains(PcbFamily::FootprintProperties)
            || self.contains(PcbFamily::FootprintGraphics)
            || self.contains(PcbFamily::Holes)
            || self.contains(PcbFamily::FootprintTransforms)
            || self.contains(PcbFamily::Profile)
        {
            result = result.with(PcbFamily::Footprints);
        }
        if self.contains(PcbFamily::Holes) {
            result = result.with(PcbFamily::Pads).with(PcbFamily::Vias);
        }
        if self.contains(PcbFamily::Profile) {
            result = result.with(PcbFamily::Graphics);
        }
        if self.contains(PcbFamily::Pads)
            || self.contains(PcbFamily::Segments)
            || self.contains(PcbFamily::Vias)
            || self.contains(PcbFamily::Zones)
            || self.contains(PcbFamily::Arcs)
        {
            result = result.with(PcbFamily::Nets);
        }
        result
    }
}

impl Default for PcbSelection {
    fn default() -> Self {
        Self::all()
    }
}

const fn family_mask(family: PcbFamily) -> u32 {
    1 << family as u32
}
