//! Validated presentation policy shared by every direct typed SVG renderer.

use std::collections::BTreeMap;

use crate::{SvgError as SvgErrorType, SvgErrorKind};

type SvgError = SvgErrorType;

#[allow(
    non_snake_case,
    reason = "local constructor preserves concise context error call sites"
)]
fn SvgError(message: String) -> SvgErrorType {
    SvgErrorType::new(SvgErrorKind::InvalidContext, message)
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum SvgProfile {
    #[default]
    PlotterBaseA0,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SvgColor(String);

impl SvgColor {
    pub fn parse(value: impl Into<String>) -> Result<Self, SvgError> {
        let value = value.into();
        let bytes = value.as_bytes();
        if !matches!(bytes.len(), 7 | 9)
            || bytes.first() != Some(&b'#')
            || !bytes[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(SvgError(format!(
                "SVG context color must be #RRGGBB or #RRGGBBAA, got {value:?}"
            )));
        }
        Ok(Self(value.to_ascii_uppercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SvgBackground {
    Opaque(SvgColor),
    Transparent,
}

impl Default for SvgBackground {
    fn default() -> Self {
        Self::Opaque(SvgColor("#FFFFFFFF".to_owned()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SvgSemanticRole {
    Copper,
    Drill,
    Mask,
    Silkscreen,
    Fabrication,
    Courtyard,
    BoardEdge,
    Worksheet,
    SchematicWire,
    SchematicBus,
    Junction,
    Label,
    Pin,
    SymbolBody,
    HierarchicalSheet,
    Text,
    Image,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlotterOperationKind {
    ThickSegment,
    ArcThreePoint,
    Circle,
    Rect,
    PlotPoly,
    BezierCurve,
    Text,
    PlotImage,
    FlashPadCircle,
    FlashPadOval,
    FlashPadRect,
    FlashPadRoundRect,
    FlashPadCustom,
    FlashPadTrapez,
    StartBlock,
    EndBlock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SvgLineStyle {
    Source,
    Solid,
    Dash,
    Dot,
    DashDot,
    DashDotDot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SvgFillMode {
    Source,
    None,
    Solid,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SvgStyleOverride {
    stroke: Option<SvgColor>,
    fill: Option<SvgColor>,
    stroke_width_nm: Option<u64>,
    line_style: Option<SvgLineStyle>,
    fill_mode: Option<SvgFillMode>,
    opacity: Option<f64>,
    visible: Option<bool>,
}

impl SvgStyleOverride {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stroke(mut self, color: SvgColor) -> Self {
        self.stroke = Some(color);
        self
    }

    pub fn with_fill(mut self, color: SvgColor) -> Self {
        self.fill = Some(color);
        self
    }

    pub const fn with_stroke_width_nm(mut self, width_nm: u64) -> Self {
        self.stroke_width_nm = Some(width_nm);
        self
    }

    pub const fn with_line_style(mut self, line_style: SvgLineStyle) -> Self {
        self.line_style = Some(line_style);
        self
    }

    pub const fn with_fill_mode(mut self, fill_mode: SvgFillMode) -> Self {
        self.fill_mode = Some(fill_mode);
        self
    }

    pub const fn with_opacity(mut self, opacity: f64) -> Self {
        self.opacity = Some(opacity);
        self
    }

    pub const fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = Some(visible);
        self
    }

    pub fn stroke(&self) -> Option<&SvgColor> {
        self.stroke.as_ref()
    }

    pub fn fill(&self) -> Option<&SvgColor> {
        self.fill.as_ref()
    }

    pub const fn stroke_width_nm(&self) -> Option<u64> {
        self.stroke_width_nm
    }

    pub const fn line_style(&self) -> Option<SvgLineStyle> {
        self.line_style
    }

    pub const fn fill_mode(&self) -> Option<SvgFillMode> {
        self.fill_mode
    }

    pub const fn opacity(&self) -> Option<f64> {
        self.opacity
    }

    pub const fn visible(&self) -> Option<bool> {
        self.visible
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LayerPattern {
    All,
    Exact(String),
    Suffix(String),
    Unlayered,
}

impl LayerPattern {
    pub fn parse(value: impl Into<String>) -> Result<Self, SvgError> {
        let value = value.into();
        if value == "*" {
            return Ok(Self::All);
        }
        if value == "<unlayered>" {
            return Ok(Self::Unlayered);
        }
        if value.is_empty()
            || value.bytes().any(|byte| byte.is_ascii_whitespace())
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(SvgError(
                "SVG layer patterns cannot be empty or contain whitespace/control bytes".to_owned(),
            ));
        }
        let stars = value.bytes().filter(|byte| *byte == b'*').count();
        if stars == 0 {
            return Ok(Self::Exact(value));
        }
        if stars == 1 && value.starts_with("*.") && value.len() > 2 {
            return Ok(Self::Suffix(value[1..].to_owned()));
        }
        Err(SvgError(format!(
            "unsupported SVG layer pattern {value:?}; use exact, *, or one leading-star suffix"
        )))
    }

    pub fn matches(&self, layer: Option<&str>) -> bool {
        match (self, layer) {
            (Self::All, _) => true,
            (Self::Unlayered, None) => true,
            (Self::Exact(pattern), Some(layer)) => pattern == layer,
            (Self::Suffix(suffix), Some(layer)) => layer.ends_with(suffix),
            _ => false,
        }
    }

    fn text_bytes(&self) -> usize {
        match self {
            Self::All => 1,
            Self::Exact(value) | Self::Suffix(value) => value.len(),
            Self::Unlayered => "<unlayered>".len(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum LayerSelection {
    #[default]
    All,
    Include {
        patterns: Vec<LayerPattern>,
        strict: bool,
    },
}

impl LayerSelection {
    pub fn include(patterns: Vec<LayerPattern>, strict: bool) -> Self {
        Self::Include { patterns, strict }
    }

    pub fn matches(&self, layer: Option<&str>) -> bool {
        match self {
            Self::All => true,
            Self::Include { patterns, .. } => patterns.iter().any(|pattern| pattern.matches(layer)),
        }
    }

    pub const fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgVisibility {
    pin_names: bool,
    pin_numbers: bool,
}

impl Default for SvgVisibility {
    fn default() -> Self {
        Self {
            pin_names: true,
            pin_numbers: true,
        }
    }
}

impl SvgVisibility {
    pub const fn new(pin_names: bool, pin_numbers: bool) -> Self {
        Self {
            pin_names,
            pin_numbers,
        }
    }

    pub const fn pin_names(self) -> bool {
        self.pin_names
    }

    pub const fn pin_numbers(self) -> bool {
        self.pin_numbers
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SvgIdentityMode {
    #[default]
    Full,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgContextLimits {
    pub max_semantic_styles: usize,
    pub max_layer_styles: usize,
    pub max_operation_styles: usize,
    pub max_color_remaps: usize,
    pub max_layer_patterns: usize,
    pub max_total_entries: usize,
    pub max_text_bytes: usize,
}

impl Default for SvgContextLimits {
    fn default() -> Self {
        Self {
            max_semantic_styles: 256,
            max_layer_styles: 1024,
            max_operation_styles: 256,
            max_color_remaps: 1024,
            max_layer_patterns: 1024,
            max_total_entries: 4096,
            max_text_bytes: 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SvgRenderContextA1 {
    profile: SvgProfile,
    background: SvgBackground,
    fallback_style: SvgStyleOverride,
    semantic_styles: BTreeMap<SvgSemanticRole, SvgStyleOverride>,
    layer_styles: Vec<(LayerPattern, SvgStyleOverride)>,
    operation_styles: BTreeMap<PlotterOperationKind, SvgStyleOverride>,
    raw_color_remap: BTreeMap<SvgColor, SvgColor>,
    layer_selection: LayerSelection,
    visibility: SvgVisibility,
    identity_mode: SvgIdentityMode,
    font_face_override: Option<String>,
}

impl Default for SvgRenderContextA1 {
    fn default() -> Self {
        SvgRenderContextBuilder::new().build()
    }
}

impl SvgRenderContextA1 {
    pub fn builder() -> SvgRenderContextBuilder {
        SvgRenderContextBuilder::new()
    }

    pub fn validate(
        self,
        limits: SvgContextLimits,
    ) -> Result<ValidatedSvgRenderContextA1, SvgError> {
        validate_context(&self, limits)?;
        Ok(ValidatedSvgRenderContextA1(self))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedSvgRenderContextA1(SvgRenderContextA1);

impl ValidatedSvgRenderContextA1 {
    pub fn defaults() -> Self {
        SvgRenderContextA1::default()
            .validate(SvgContextLimits::default())
            .expect("built-in SVG context is valid")
    }

    pub const fn profile(&self) -> SvgProfile {
        self.0.profile
    }

    pub fn background(&self) -> &SvgBackground {
        &self.0.background
    }

    pub fn fallback_style(&self) -> &SvgStyleOverride {
        &self.0.fallback_style
    }

    pub fn semantic_style(&self, role: SvgSemanticRole) -> Option<&SvgStyleOverride> {
        self.0.semantic_styles.get(&role)
    }

    pub fn layer_styles(&self) -> &[(LayerPattern, SvgStyleOverride)] {
        &self.0.layer_styles
    }

    pub fn operation_style(&self, kind: PlotterOperationKind) -> Option<&SvgStyleOverride> {
        self.0.operation_styles.get(&kind)
    }

    pub fn remap_color<'a>(&'a self, color: &'a SvgColor) -> &'a SvgColor {
        self.0.raw_color_remap.get(color).unwrap_or(color)
    }

    pub fn layer_selection(&self) -> &LayerSelection {
        &self.0.layer_selection
    }

    pub const fn visibility(&self) -> SvgVisibility {
        self.0.visibility
    }

    pub const fn identity_mode(&self) -> SvgIdentityMode {
        self.0.identity_mode
    }

    pub fn font_face_override(&self) -> Option<&str> {
        self.0.font_face_override.as_deref()
    }

    pub(crate) fn has_style_overrides(&self) -> bool {
        self.0.fallback_style != SvgStyleOverride::default()
            || !self.0.semantic_styles.is_empty()
            || !self.0.layer_styles.is_empty()
            || !self.0.operation_styles.is_empty()
            || !self.0.raw_color_remap.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct SvgRenderContextBuilder(SvgRenderContextA1);

impl Default for SvgRenderContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SvgRenderContextBuilder {
    pub fn new() -> Self {
        Self(SvgRenderContextA1 {
            profile: SvgProfile::PlotterBaseA0,
            background: SvgBackground::default(),
            fallback_style: SvgStyleOverride::default(),
            semantic_styles: BTreeMap::new(),
            layer_styles: Vec::new(),
            operation_styles: BTreeMap::new(),
            raw_color_remap: BTreeMap::new(),
            layer_selection: LayerSelection::All,
            visibility: SvgVisibility::default(),
            identity_mode: SvgIdentityMode::Full,
            font_face_override: None,
        })
    }

    pub fn profile(mut self, profile: SvgProfile) -> Self {
        self.0.profile = profile;
        self
    }

    pub fn background(mut self, background: SvgBackground) -> Self {
        self.0.background = background;
        self
    }

    pub fn fallback_style(mut self, style: SvgStyleOverride) -> Self {
        self.0.fallback_style = style;
        self
    }

    pub fn semantic_style(mut self, role: SvgSemanticRole, style: SvgStyleOverride) -> Self {
        self.0.semantic_styles.insert(role, style);
        self
    }

    pub fn layer_style(mut self, pattern: LayerPattern, style: SvgStyleOverride) -> Self {
        self.0.layer_styles.push((pattern, style));
        self
    }

    pub fn operation_style(mut self, kind: PlotterOperationKind, style: SvgStyleOverride) -> Self {
        self.0.operation_styles.insert(kind, style);
        self
    }

    pub fn raw_color_remap(mut self, source: SvgColor, target: SvgColor) -> Self {
        self.0.raw_color_remap.insert(source, target);
        self
    }

    pub fn layer_selection(mut self, selection: LayerSelection) -> Self {
        self.0.layer_selection = selection;
        self
    }

    pub fn visibility(mut self, visibility: SvgVisibility) -> Self {
        self.0.visibility = visibility;
        self
    }

    pub fn identity_mode(mut self, mode: SvgIdentityMode) -> Self {
        self.0.identity_mode = mode;
        self
    }

    pub fn font_face_override(mut self, font_face: impl Into<String>) -> Self {
        self.0.font_face_override = Some(font_face.into());
        self
    }

    pub fn build(self) -> SvgRenderContextA1 {
        self.0
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "all bounded context invariants are audited together"
)]
fn validate_context(
    context: &SvgRenderContextA1,
    limits: SvgContextLimits,
) -> Result<(), SvgError> {
    for (pattern, _) in &context.layer_styles {
        validate_layer_pattern(pattern)?;
    }
    ensure_count(
        context.semantic_styles.len(),
        limits.max_semantic_styles,
        "semantic styles",
    )?;
    ensure_count(
        context.layer_styles.len(),
        limits.max_layer_styles,
        "layer styles",
    )?;
    ensure_unique_patterns(
        &context
            .layer_styles
            .iter()
            .map(|(pattern, _)| pattern.clone())
            .collect::<Vec<_>>(),
    )?;
    ensure_count(
        context.operation_styles.len(),
        limits.max_operation_styles,
        "operation styles",
    )?;
    ensure_count(
        context.raw_color_remap.len(),
        limits.max_color_remaps,
        "color remaps",
    )?;
    let patterns = match &context.layer_selection {
        LayerSelection::All => 0,
        LayerSelection::Include { patterns, .. } => {
            if patterns.is_empty() {
                return Err(SvgError(
                    "SVG context layer selection cannot be empty".to_owned(),
                ));
            }
            for pattern in patterns {
                validate_layer_pattern(pattern)?;
            }
            ensure_unique_patterns(patterns)?;
            patterns.len()
        }
    };
    ensure_count(patterns, limits.max_layer_patterns, "layer patterns")?;
    let total_entries = context
        .semantic_styles
        .len()
        .checked_add(context.layer_styles.len())
        .and_then(|value| value.checked_add(context.operation_styles.len()))
        .and_then(|value| value.checked_add(context.raw_color_remap.len()))
        .and_then(|value| value.checked_add(patterns))
        .ok_or_else(|| SvgError("SVG context entry count overflowed".to_owned()))?;
    ensure_count(total_entries, limits.max_total_entries, "total entries")?;

    validate_style(&context.fallback_style)?;
    for style in context
        .semantic_styles
        .values()
        .chain(context.layer_styles.iter().map(|(_, style)| style))
        .chain(context.operation_styles.values())
    {
        validate_style(style)?;
    }
    let mut text_bytes = context.font_face_override.as_ref().map_or(0, String::len);
    if let SvgBackground::Opaque(color) = &context.background {
        text_bytes = checked_add(text_bytes, color.as_str().len(), "context text bytes")?;
    }
    text_bytes = checked_add(
        text_bytes,
        style_text_bytes(&context.fallback_style)?,
        "context text bytes",
    )?;
    for style in context
        .semantic_styles
        .values()
        .chain(context.layer_styles.iter().map(|(_, style)| style))
        .chain(context.operation_styles.values())
    {
        text_bytes = checked_add(text_bytes, style_text_bytes(style)?, "context text bytes")?;
    }
    for (source, target) in &context.raw_color_remap {
        text_bytes = checked_add(text_bytes, source.as_str().len(), "context text bytes")?;
        text_bytes = checked_add(text_bytes, target.as_str().len(), "context text bytes")?;
    }
    for (pattern, _) in &context.layer_styles {
        text_bytes = checked_add(text_bytes, pattern.text_bytes(), "context text bytes")?;
    }
    if let LayerSelection::Include { patterns, .. } = &context.layer_selection {
        for pattern in patterns {
            text_bytes = checked_add(text_bytes, pattern.text_bytes(), "context text bytes")?;
        }
    }
    ensure_count(text_bytes, limits.max_text_bytes, "text bytes")?;
    if context
        .font_face_override
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.chars().any(char::is_control))
    {
        return Err(SvgError(
            "SVG context font override cannot be empty or contain control characters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_layer_pattern(pattern: &LayerPattern) -> Result<(), SvgError> {
    let valid_text = |value: &str| {
        !value.is_empty()
            && !value.bytes().any(|byte| byte.is_ascii_whitespace())
            && !value.bytes().any(|byte| byte.is_ascii_control())
    };
    match pattern {
        LayerPattern::All | LayerPattern::Unlayered => Ok(()),
        LayerPattern::Exact(value) if valid_text(value) && !value.contains('*') => Ok(()),
        LayerPattern::Suffix(value)
            if valid_text(value)
                && value.starts_with('.')
                && value.len() > 1
                && !value.contains('*') =>
        {
            Ok(())
        }
        _ => Err(SvgError(
            "SVG context contains a malformed layer pattern; construct patterns with LayerPattern::parse"
                .to_owned(),
        )),
    }
}

fn validate_style(style: &SvgStyleOverride) -> Result<(), SvgError> {
    if style
        .opacity
        .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        return Err(SvgError(
            "SVG context style opacity must be finite and in [0,1]".to_owned(),
        ));
    }
    if style
        .stroke_width_nm
        .is_some_and(|width| i64::try_from(width).is_err())
    {
        return Err(SvgError(
            "SVG context stroke width exceeds the renderer numeric range".to_owned(),
        ));
    }
    Ok(())
}

fn style_text_bytes(style: &SvgStyleOverride) -> Result<usize, SvgError> {
    checked_add(
        style
            .stroke
            .as_ref()
            .map_or(0, |color| color.as_str().len()),
        style.fill.as_ref().map_or(0, |color| color.as_str().len()),
        "style color bytes",
    )
}

fn ensure_unique_patterns(patterns: &[LayerPattern]) -> Result<(), SvgError> {
    let mut sorted = patterns.to_vec();
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SvgError(
            "SVG context layer patterns must be unique".to_owned(),
        ));
    }
    Ok(())
}

fn checked_add(left: usize, right: usize, label: &str) -> Result<usize, SvgError> {
    left.checked_add(right)
        .ok_or_else(|| SvgError(format!("SVG context {label} overflowed")))
}

fn ensure_count(actual: usize, maximum: usize, label: &str) -> Result<(), SvgError> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(SvgError(format!(
            "SVG context {label} exceeds the configured limit"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_exact_layer_patterns_are_case_sensitive() {
        let context = SvgRenderContextA1::default()
            .validate(SvgContextLimits::default())
            .unwrap();
        assert!(context.layer_selection().is_all());
        assert!(LayerPattern::parse("*.Cu").unwrap().matches(Some("F.Cu")));
        assert!(!LayerPattern::parse("*.Cu").unwrap().matches(Some("F.cu")));
        assert!(LayerPattern::parse("<unlayered>").unwrap().matches(None));
        assert!(LayerPattern::parse("F.*.Cu").is_err());
    }

    #[test]
    fn maps_patterns_text_and_opacity_are_bounded_before_use() {
        let style = SvgStyleOverride::new().with_opacity(0.5);
        let context = SvgRenderContextA1::builder()
            .semantic_style(SvgSemanticRole::Copper, style.clone())
            .layer_style(LayerPattern::parse("F.Cu").unwrap(), style.clone())
            .operation_style(PlotterOperationKind::Circle, style)
            .layer_selection(LayerSelection::include(
                vec![LayerPattern::parse("*.Cu").unwrap()],
                true,
            ))
            .font_face_override("KiCad Font")
            .build();
        assert!(
            context
                .clone()
                .validate(SvgContextLimits::default())
                .is_ok()
        );
        assert!(
            context
                .clone()
                .validate(SvgContextLimits {
                    max_total_entries: 3,
                    ..SvgContextLimits::default()
                })
                .is_err()
        );
        assert!(
            context
                .validate(SvgContextLimits {
                    max_text_bytes: 1,
                    ..SvgContextLimits::default()
                })
                .is_err()
        );
        assert!(
            SvgRenderContextA1::builder()
                .fallback_style(SvgStyleOverride::new().with_opacity(f64::NAN))
                .build()
                .validate(SvgContextLimits::default())
                .is_err()
        );
    }

    #[test]
    fn colors_are_canonical_and_duplicate_selectors_fail_closed() {
        assert_eq!(SvgColor::parse("#aabbcc80").unwrap().as_str(), "#AABBCC80");
        assert!(SvgColor::parse("blue").is_err());
        let exact = LayerPattern::parse("F.Cu").unwrap();
        let context = SvgRenderContextA1::builder()
            .layer_selection(LayerSelection::include(vec![exact.clone(), exact], false))
            .build();
        assert!(context.validate(SvgContextLimits::default()).is_err());
        assert!(
            SvgRenderContextA1::builder()
                .layer_style(LayerPattern::Suffix(String::new()), SvgStyleOverride::new(),)
                .build()
                .validate(SvgContextLimits::default())
                .is_err()
        );
    }
}
