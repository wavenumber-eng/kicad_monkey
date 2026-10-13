//! Operation sink shared by every product emitter.
//!
//! Each emitted operation receives a provisional index and a binding entry;
//! final indices are assigned by chunking.

use super::geometry::board_bounds;
use super::holes::HoleSpec;
use kicad_monkey_contracts::generated::materialization_document::{
    Affine2D, Backdrill, BindingEntry, BindingSourceKind, DiagnosticSeverity, HoleOperation,
    HolePlating, HoleProtection, MaterialAction, MaterialOperation, MaterialProduct,
    MaterializationDiagnostic, Nm, NmBounds, NonNegativeNm, PostMachining, Shape2d,
    SurfaceMaterial, SurfaceOperation,
};

/// KiCad object an operation came from.
#[derive(Clone)]
pub(crate) struct Origin {
    pub kind: BindingSourceKind,
    pub uid: String,
    pub refdes: Option<String>,
    pub pad_number: Option<String>,
    pub net_key: String,
    /// Pad-to-die length and delay of a pad's pin.
    pub die_length_nm: Option<i64>,
    pub die_delay_ps: Option<f64>,
    /// Nets a net-tie bridge joins.
    pub tied_net_keys: Vec<String>,
}

impl Origin {
    pub(crate) fn routed(kind: BindingSourceKind, uid: String, net_key: String) -> Self {
        Self {
            kind,
            uid,
            refdes: None,
            pad_number: None,
            net_key,
            die_length_nm: None,
            die_delay_ps: None,
            tied_net_keys: Vec::new(),
        }
    }
}

/// Physical facts of one hole besides its shape and placement.
#[derive(Clone)]
pub(crate) struct HoleFacts {
    pub plating: HolePlating,
    pub start_slice_key: String,
    pub end_slice_key: String,
    pub plating_thickness_nm: Option<i64>,
    pub protection: Option<HoleProtection>,
    pub backdrills: Vec<Backdrill>,
    pub post_machining: Vec<PostMachining>,
    pub castellated: Option<bool>,
}

/// Material, action, owning net, and cut group of a surface operation.
pub(crate) struct Material {
    pub material: SurfaceMaterial,
    pub action: MaterialAction,
    pub net_key: Option<String>,
    /// Item whose subtracts are confined to its own adds.
    pub group_key: Option<String>,
}

#[derive(Default)]
pub(crate) struct Emitter {
    pub operations: Vec<MaterialOperation>,
    pub binding: Vec<BindingEntry>,
    pub diagnostics: Vec<MaterializationDiagnostic>,
    /// Product that warnings raised now belong to.
    pub product: Option<MaterialProduct>,
    /// Stop recording operations past this count (see [`Self::overflowed`]).
    pub cap: Option<usize>,
    /// Set when an operation was refused because of [`Self::cap`].
    pub overflowed: bool,
}

/// Stable source identity: the KiCad UUID, or `kind@byte_offset` when the
/// object has none.
pub(crate) fn source_uid(kind: &str, uuid: Option<&str>, source_start: usize) -> String {
    uuid.filter(|value| !value.is_empty())
        .map_or_else(|| format!("{kind}@{source_start}"), str::to_owned)
}

/// Bounds grown by a positive local offset scaled into the board frame.
/// A negative offset only shrinks the shape, so the bounds stay valid.
fn grown(bounds: NmBounds, offset_nm: i32, affine: &Affine2D) -> NmBounds {
    if offset_nm <= 0 {
        return bounds;
    }
    let scale = affine
        .m11
        .hypot(affine.m21)
        .max(affine.m12.hypot(affine.m22));
    let grow = (f64::from(offset_nm) * scale).ceil() as i64;
    let clamp = |value: i64| Nm(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
    NmBounds {
        min_x_nm: clamp(i64::from(*bounds.min_x_nm) - grow),
        min_y_nm: clamp(i64::from(*bounds.min_y_nm) - grow),
        max_x_nm: clamp(i64::from(*bounds.max_x_nm) + grow),
        max_y_nm: clamp(i64::from(*bounds.max_y_nm) + grow),
    }
}

impl Emitter {
    /// Whether one more operation fits under the cap; records overflow.
    fn admit(&mut self) -> bool {
        let full = self.cap.is_some_and(|cap| self.operations.len() >= cap);
        self.overflowed |= full;
        !full
    }

    fn next_index(&self) -> u32 {
        u32::try_from(self.operations.len()).unwrap_or(u32::MAX)
    }

    fn bind(&mut self, index: u32, origin: &Origin, net_key: Option<&str>) {
        self.binding.push(BindingEntry {
            operation_index: index,
            source_kind: origin.kind,
            source_uid: origin.uid.clone(),
            refdes: origin.refdes.clone(),
            pad_number: origin.pad_number.clone(),
            net_key: net_key.map(str::to_owned),
            tied_net_keys: origin.tied_net_keys.clone(),
            die_length_nm: origin.die_length_nm.map(NonNegativeNm),
            die_delay_ps: origin.die_delay_ps,
        });
    }

    /// Add copper of `origin`'s net on `slice_key`.
    pub(crate) fn copper(
        &mut self,
        origin: &Origin,
        slice_key: &str,
        shape: Shape2d,
        affine: Affine2D,
    ) {
        let material = Material {
            material: SurfaceMaterial::Copper,
            action: MaterialAction::Add,
            net_key: Some(origin.net_key.clone()),
            group_key: None,
        };
        self.surface(origin, slice_key, material, shape, affine);
    }

    /// Add or subtract `material` on `slice_key`.
    pub(crate) fn surface(
        &mut self,
        origin: &Origin,
        slice_key: &str,
        material: Material,
        shape: Shape2d,
        affine: Affine2D,
    ) {
        self.offset_surface(origin, slice_key, material, shape, affine, 0);
    }

    /// Add or subtract `material` on `slice_key`, with `shape` offset by
    /// `offset_nm` in its local frame (recorded only when nonzero).
    pub(crate) fn offset_surface(
        &mut self,
        origin: &Origin,
        slice_key: &str,
        material: Material,
        shape: Shape2d,
        affine: Affine2D,
        offset_nm: i32,
    ) {
        if !self.admit() {
            return;
        }
        let index = self.next_index();
        let bounds = grown(board_bounds(&shape, &affine), offset_nm, &affine);
        self.bind(index, origin, material.net_key.as_deref());
        self.operations
            .push(MaterialOperation::SurfaceOperation(SurfaceOperation {
                kind: "surface".to_owned(),
                index,
                action: material.action,
                material: material.material,
                slice_key: slice_key.to_owned(),
                net_key: material.net_key,
                group_key: material.group_key,
                shape,
                offset_nm: (offset_nm != 0).then_some(Nm(offset_nm)),
                affine,
                bounds,
            }));
    }

    /// Add a hole. Plated holes carry the origin's net; unplated holes none.
    pub(crate) fn hole(&mut self, spec: &HoleSpec) {
        if !self.admit() {
            return;
        }
        let index = self.next_index();
        let bounds = board_bounds(&spec.shape, &spec.affine);
        let facts = spec.facts.clone();
        let net_key =
            matches!(facts.plating, HolePlating::Plated).then(|| spec.origin.net_key.clone());
        self.bind(index, &spec.origin, net_key.as_deref());
        self.operations
            .push(MaterialOperation::HoleOperation(HoleOperation {
                kind: "hole".to_owned(),
                index,
                shape: spec.shape.clone(),
                affine: spec.affine.clone(),
                plating: facts.plating,
                start_slice_key: facts.start_slice_key,
                end_slice_key: facts.end_slice_key,
                net_key,
                plating_thickness_nm: facts.plating_thickness_nm.map(NonNegativeNm),
                protection: facts.protection,
                backdrills: facts.backdrills,
                post_machining: facts.post_machining,
                castellated: facts.castellated,
                bounds,
            }));
    }

    /// Report material on copper layers the board does not declare.
    pub(crate) fn unknown_layers(&mut self, layers: &[&str], source_uid: &str) {
        self.warn(
            "layer_not_in_stack",
            format!(
                "{} not a copper layer of this board; the item is not materialized there",
                layers.join(", ")
            ),
            Some(source_uid.to_owned()),
        );
    }

    pub(crate) fn warn(&mut self, code: &str, message: String, source_uid: Option<String>) {
        self.diagnostics.push(MaterializationDiagnostic {
            severity: DiagnosticSeverity::Warning,
            code: code.to_owned(),
            message,
            product: self.product,
            source_uid,
        });
    }
}
