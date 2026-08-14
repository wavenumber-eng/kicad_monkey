//! Bounded `.kicad_pro` JSON source model and exact owned writer.

mod json_preflight;

use serde::Serialize;
use serde_json::{Map, Value};
use std::fmt::{Display, Formatter};
use std::io::{Read, Write};

use json_preflight::preflight_json_structure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectLimits {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_json_nodes: usize,
    pub max_json_depth: usize,
    pub max_text_variables: usize,
    pub max_variants: usize,
    pub max_net_classes: usize,
    pub max_netclass_assignments: usize,
    pub max_netclass_assignment_references: usize,
    pub max_netclass_patterns: usize,
    pub max_net_colors: usize,
    pub max_diff_pair_dimensions: usize,
    pub max_typed_string_bytes: usize,
}

impl Default for ProjectLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_json_nodes: 8_000_000,
            max_json_depth: 256,
            max_text_variables: 1_000_000,
            max_variants: 1_000_000,
            max_net_classes: 1_000_000,
            max_netclass_assignments: 1_000_000,
            max_netclass_assignment_references: 4_000_000,
            max_netclass_patterns: 1_000_000,
            max_net_colors: 1_000_000,
            max_diff_pair_dimensions: 1_000_000,
            max_typed_string_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectErrorKind {
    InvalidUtf8,
    InvalidJson,
    RootNotObject,
    ResourceLimit,
    InvalidPath,
    Conflict,
    Io,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectError {
    pub kind: ProjectErrorKind,
    pub message: String,
}

impl ProjectError {
    fn new(kind: ProjectErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl Display for ProjectError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ProjectError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectVariant {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectNetClass {
    pub name: String,
    pub track_width: Option<f64>,
    pub clearance: Option<f64>,
    pub diff_pair_gap: Option<f64>,
    pub diff_pair_width: Option<f64>,
    pub diff_pair_via_gap: Option<f64>,
    pub via_diameter: Option<f64>,
    pub via_drill: Option<f64>,
    pub microvia_diameter: Option<f64>,
    pub microvia_drill: Option<f64>,
    pub bus_width: Option<f64>,
    pub wire_width: Option<f64>,
    pub pcb_color: String,
    pub schematic_color: String,
    pub line_style: Option<i64>,
    pub priority: Option<i64>,
    pub tuning_profile: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectNetClassPattern {
    pub pattern: String,
    pub netclass_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectNetSettings {
    pub classes: Vec<ProjectNetClass>,
    pub assignments: Vec<(String, Vec<String>)>,
    pub patterns: Vec<ProjectNetClassPattern>,
    pub colors: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectDiffPairDimensions {
    pub width: Option<f64>,
    pub gap: Option<f64>,
    pub via_gap: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectTuningDefaults {
    pub spacing: Option<f64>,
    pub min_amplitude: Option<f64>,
    pub max_amplitude: Option<f64>,
    pub corner_style: Option<i64>,
    pub corner_radius_percentage: Option<i64>,
    pub single_sided: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectTuningSettings {
    pub diff_pair_defaults: ProjectTuningDefaults,
    pub diff_pair_skew_defaults: ProjectTuningDefaults,
    pub single_track_defaults: ProjectTuningDefaults,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectBoardDesignSettings {
    pub diff_pair_dimensions: Vec<ProjectDiffPairDimensions>,
    pub tuning: ProjectTuningSettings,
}

#[derive(Clone, Copy, Debug)]
pub struct ProjectView<'a> {
    root: &'a Map<String, Value>,
    limits: ProjectLimits,
}

impl<'a> ProjectView<'a> {
    #[must_use]
    pub const fn raw(&self) -> &'a Map<String, Value> {
        self.root
    }

    pub fn text_variables(&self) -> Result<Vec<(String, String)>, ProjectError> {
        let Some(values) = self.root.get("text_variables").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        check_count(
            values.len(),
            self.limits.max_text_variables,
            "text variables",
        )?;
        let mut budget = StringBudget::new(self.limits.max_typed_string_bytes);
        values
            .iter()
            .map(|(name, value)| {
                let value = project_string(value);
                budget.pair(name, &value)?;
                Ok((name.clone(), value))
            })
            .collect()
    }

    pub fn variants(&self) -> Result<Vec<ProjectVariant>, ProjectError> {
        let values = path(self.root, &["schematic", "variants"])
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let object_count = values.iter().filter(|value| value.is_object()).count();
        check_count(object_count, self.limits.max_variants, "variants")?;
        let mut budget = StringBudget::new(self.limits.max_typed_string_bytes);
        values
            .iter()
            .filter_map(Value::as_object)
            .map(|value| {
                let name = object_string(value, "name");
                let description = match value.get("description") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(project_string(value)),
                };
                budget.string(&name)?;
                if let Some(description) = &description {
                    budget.string(description)?;
                }
                Ok(ProjectVariant { name, description })
            })
            .collect()
    }

    pub fn net_settings(&self) -> Result<ProjectNetSettings, ProjectError> {
        let settings = self.root.get("net_settings").and_then(Value::as_object);
        let mut budget = StringBudget::new(self.limits.max_typed_string_bytes);
        Ok(ProjectNetSettings {
            classes: net_classes(settings, self.limits, &mut budget)?,
            assignments: net_assignments(settings, self.limits, &mut budget)?,
            patterns: net_patterns(settings, self.limits, &mut budget)?,
            colors: net_colors(settings, self.limits, &mut budget)?,
        })
    }

    pub fn board_design_settings(&self) -> Result<ProjectBoardDesignSettings, ProjectError> {
        let settings = path(self.root, &["board", "design_settings"]).and_then(Value::as_object);
        let dimensions = settings
            .and_then(|value| value.get("diff_pair_dimensions"))
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let object_count = dimensions.iter().filter(|value| value.is_object()).count();
        check_count(
            object_count,
            self.limits.max_diff_pair_dimensions,
            "differential-pair dimensions",
        )?;
        let diff_pair_dimensions = dimensions
            .iter()
            .filter_map(Value::as_object)
            .map(|value| ProjectDiffPairDimensions {
                width: optional_number(value, "width"),
                gap: optional_number(value, "gap"),
                via_gap: optional_number(value, "via_gap"),
            })
            .collect();
        let tuning = settings
            .and_then(|value| value.get("tuning_pattern_settings"))
            .and_then(Value::as_object);
        Ok(ProjectBoardDesignSettings {
            diff_pair_dimensions,
            tuning: ProjectTuningSettings {
                diff_pair_defaults: tuning_defaults(tuning, "diff_pair_defaults"),
                diff_pair_skew_defaults: tuning_defaults(tuning, "diff_pair_skew_defaults"),
                single_track_defaults: tuning_defaults(tuning, "single_track_defaults"),
            },
        })
    }

    #[must_use]
    pub fn get_path(&self, dotted_path: &str) -> Option<&'a Value> {
        let mut parts = dotted_path.split('.');
        let first = parts.next()?;
        if first.is_empty() {
            return None;
        }
        let mut value = self.root.get(first)?;
        for part in parts {
            value = value.as_object()?.get(part)?;
        }
        Some(value)
    }
}

#[derive(Clone, Debug)]
pub struct ProjectDocument {
    source: String,
    root: Map<String, Value>,
    limits: ProjectLimits,
}

impl ProjectDocument {
    pub fn parse(source: String, limits: ProjectLimits) -> Result<Self, ProjectError> {
        if source.len() > limits.max_source_bytes {
            return Err(limit_error("project source exceeds max_source_bytes"));
        }
        preflight_json_structure(&source, limits)?;
        let value: Value = serde_json::from_str(&source)
            .map_err(|error| ProjectError::new(ProjectErrorKind::InvalidJson, error.to_string()))?;
        let Value::Object(root) = value else {
            return Err(ProjectError::new(
                ProjectErrorKind::RootNotObject,
                ".kicad_pro root must be a JSON object",
            ));
        };
        Ok(Self {
            source,
            root,
            limits,
        })
    }

    pub fn from_reader(mut reader: impl Read, limits: ProjectLimits) -> Result<Self, ProjectError> {
        let read_limit = limits
            .max_source_bytes
            .checked_add(1)
            .ok_or_else(|| limit_error("project source limit overflows"))?;
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(read_limit as u64)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() > limits.max_source_bytes {
            return Err(limit_error("project source exceeds max_source_bytes"));
        }
        let source = String::from_utf8(bytes).map_err(|error| {
            ProjectError::new(
                ProjectErrorKind::InvalidUtf8,
                format!(
                    "project source is not UTF-8 at byte {}",
                    error.utf8_error().valid_up_to()
                ),
            )
        })?;
        Self::parse(source, limits)
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn view(&self) -> ProjectView<'_> {
        ProjectView {
            root: &self.root,
            limits: self.limits,
        }
    }

    pub fn write_to(&self, mut writer: impl Write) -> Result<(), ProjectError> {
        self.check_output_source()?;
        writer.write_all(self.source.as_bytes()).map_err(io_error)
    }

    pub fn canonical_text(&self) -> Result<String, ProjectError> {
        serialize_project(&self.root, self.limits.max_output_bytes)
    }

    pub fn set_text_variable(&mut self, name: &str, value: &str) -> Result<bool, ProjectError> {
        self.check_output_source()?;
        check_mutation_string(name, self.limits)?;
        check_mutation_string(value, self.limits)?;
        let mut root = self.root.clone();
        let text_variables = object_field(&mut root, "text_variables", true)?;
        if text_variables.get(name).and_then(Value::as_str) == Some(value) {
            return Ok(false);
        }
        if !text_variables.contains_key(name)
            && text_variables.len() >= self.limits.max_text_variables
        {
            return Err(limit_error("project text variable count exceeds its limit"));
        }
        text_variables.insert(name.to_owned(), Value::String(value.to_owned()));
        self.commit(root)
    }

    pub fn remove_text_variable(&mut self, name: &str) -> Result<bool, ProjectError> {
        self.check_output_source()?;
        let mut root = self.root.clone();
        let Some(value) = root.get_mut("text_variables") else {
            return Ok(false);
        };
        let Some(values) = value.as_object_mut() else {
            return Ok(false);
        };
        if values.shift_remove(name).is_none() {
            return Ok(false);
        }
        self.commit(root)
    }

    pub fn add_variant(
        &mut self,
        name: &str,
        description: Option<&str>,
    ) -> Result<ProjectVariant, ProjectError> {
        self.check_output_source()?;
        if name.is_empty() {
            return Err(conflict_error("project variant name must not be empty"));
        }
        check_mutation_string(name, self.limits)?;
        if let Some(description) = description {
            check_mutation_string(description, self.limits)?;
        }
        let mut root = self.root.clone();
        let variants = variants_mut(&mut root, true)?;
        if variants.iter().any(|value| {
            value
                .as_object()
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                == Some(name)
        }) {
            return Err(conflict_error("project variant already exists"));
        }
        let object_count = variants.iter().filter(|value| value.is_object()).count();
        if object_count >= self.limits.max_variants {
            return Err(limit_error("project variant count exceeds its limit"));
        }
        let mut entry = Map::new();
        entry.insert("name".to_owned(), Value::String(name.to_owned()));
        if let Some(description) = description {
            entry.insert(
                "description".to_owned(),
                Value::String(description.to_owned()),
            );
        }
        variants.push(Value::Object(entry));
        self.commit(root)?;
        Ok(ProjectVariant {
            name: name.to_owned(),
            description: description.map(str::to_owned),
        })
    }

    pub fn remove_variant(&mut self, name: &str) -> Result<Option<ProjectVariant>, ProjectError> {
        self.check_output_source()?;
        if !variant_exists(&self.root, name)? {
            return Ok(None);
        }
        let mut root = self.root.clone();
        let variants = variants_mut(&mut root, false)?;
        let Some(index) = variants.iter().position(|value| {
            value
                .as_object()
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                == Some(name)
        }) else {
            return Ok(None);
        };
        let removed = variant_from_value(&variants.remove(index));
        self.commit(root)?;
        Ok(removed)
    }

    pub fn rename_variant(&mut self, old_name: &str, new_name: &str) -> Result<bool, ProjectError> {
        self.check_output_source()?;
        if new_name.is_empty() {
            return Err(conflict_error("project variant name must not be empty"));
        }
        check_mutation_string(new_name, self.limits)?;
        if variant_exists(&self.root, new_name)? {
            return Err(conflict_error("project variant already exists"));
        }
        if !variant_exists(&self.root, old_name)? {
            return Ok(false);
        }
        let mut root = self.root.clone();
        let variants = variants_mut(&mut root, false)?;
        let Some(entry) = variants.iter_mut().find_map(|value| {
            let object = value.as_object_mut()?;
            (object.get("name").and_then(Value::as_str) == Some(old_name)).then_some(object)
        }) else {
            return Ok(false);
        };
        entry.insert("name".to_owned(), Value::String(new_name.to_owned()));
        self.commit(root)
    }

    pub fn set_path(&mut self, dotted_path: &str, value: Value) -> Result<bool, ProjectError> {
        self.check_output_source()?;
        let parts = valid_path_parts(dotted_path)?;
        let mut root = self.root.clone();
        let mut current = &mut root;
        for part in &parts[..parts.len() - 1] {
            if !current.contains_key(*part) {
                current.insert((*part).to_owned(), Value::Object(Map::new()));
            }
            current = current
                .get_mut(*part)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| conflict_error("project path crosses a non-object value"))?;
        }
        let final_part = parts[parts.len() - 1];
        if current.get(final_part) == Some(&value) {
            return Ok(false);
        }
        current.insert(final_part.to_owned(), value);
        self.commit(root)
    }

    fn check_output_source(&self) -> Result<(), ProjectError> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(limit_error("project output exceeds max_output_bytes"));
        }
        Ok(())
    }

    fn commit(&mut self, root: Map<String, Value>) -> Result<bool, ProjectError> {
        let candidate = serialize_project(&root, self.limits.max_output_bytes)?;
        let parsed = Self::parse(candidate, self.limits)?;
        validate_promoted(&parsed)?;
        self.source = parsed.source;
        self.root = parsed.root;
        Ok(true)
    }
}

fn net_classes(
    settings: Option<&Map<String, Value>>,
    limits: ProjectLimits,
    budget: &mut StringBudget,
) -> Result<Vec<ProjectNetClass>, ProjectError> {
    let values = array_field(settings, "classes");
    let count = values.iter().filter(|value| value.is_object()).count();
    check_count(count, limits.max_net_classes, "net classes")?;
    values
        .iter()
        .filter_map(Value::as_object)
        .map(|value| {
            let result = ProjectNetClass {
                name: object_string(value, "name"),
                track_width: optional_number(value, "track_width"),
                clearance: optional_number(value, "clearance"),
                diff_pair_gap: optional_number(value, "diff_pair_gap"),
                diff_pair_width: optional_number(value, "diff_pair_width"),
                diff_pair_via_gap: optional_number(value, "diff_pair_via_gap"),
                via_diameter: optional_number(value, "via_diameter"),
                via_drill: optional_number(value, "via_drill"),
                microvia_diameter: optional_number(value, "microvia_diameter"),
                microvia_drill: optional_number(value, "microvia_drill"),
                bus_width: optional_number(value, "bus_width"),
                wire_width: optional_number(value, "wire_width"),
                pcb_color: object_string(value, "pcb_color"),
                schematic_color: object_string(value, "schematic_color"),
                line_style: optional_integer(value, "line_style"),
                priority: optional_integer(value, "priority"),
                tuning_profile: object_string(value, "tuning_profile"),
            };
            for item in [
                &result.name,
                &result.pcb_color,
                &result.schematic_color,
                &result.tuning_profile,
            ] {
                budget.string(item)?;
            }
            Ok(result)
        })
        .collect()
}

fn validate_promoted(document: &ProjectDocument) -> Result<(), ProjectError> {
    let view = document.view();
    view.text_variables()?;
    view.variants()?;
    view.net_settings()?;
    view.board_design_settings()?;
    Ok(())
}

fn net_assignments(
    settings: Option<&Map<String, Value>>,
    limits: ProjectLimits,
    budget: &mut StringBudget,
) -> Result<Vec<(String, Vec<String>)>, ProjectError> {
    let Some(values) = settings
        .and_then(|value| value.get("netclass_assignments"))
        .and_then(Value::as_object)
    else {
        return Ok(Vec::new());
    };
    check_count(
        values.len(),
        limits.max_netclass_assignments,
        "netclass assignments",
    )?;
    let mut reference_count = 0usize;
    let mut result = Vec::with_capacity(values.len());
    for (name, classes) in values {
        let classes = classes.as_array().map_or(&[][..], Vec::as_slice);
        let mut typed = Vec::new();
        for class in classes {
            let class = project_string(class);
            if class.is_empty() {
                continue;
            }
            reference_count = reference_count
                .checked_add(1)
                .ok_or_else(|| limit_error("assignment reference count overflows"))?;
            check_count(
                reference_count,
                limits.max_netclass_assignment_references,
                "netclass assignment references",
            )?;
            budget.string(&class)?;
            typed.push(class);
        }
        budget.string(name)?;
        result.push((name.clone(), typed));
    }
    Ok(result)
}

fn net_patterns(
    settings: Option<&Map<String, Value>>,
    limits: ProjectLimits,
    budget: &mut StringBudget,
) -> Result<Vec<ProjectNetClassPattern>, ProjectError> {
    let values = array_field(settings, "netclass_patterns");
    let count = values.iter().filter(|value| value.is_object()).count();
    check_count(count, limits.max_netclass_patterns, "netclass patterns")?;
    values
        .iter()
        .filter_map(Value::as_object)
        .map(|value| {
            let pattern = object_string(value, "pattern");
            let netclass_name = object_string(value, "netclass");
            budget.pair(&pattern, &netclass_name)?;
            Ok(ProjectNetClassPattern {
                pattern,
                netclass_name,
            })
        })
        .collect()
}

fn net_colors(
    settings: Option<&Map<String, Value>>,
    limits: ProjectLimits,
    budget: &mut StringBudget,
) -> Result<Vec<(String, String)>, ProjectError> {
    let Some(values) = settings
        .and_then(|value| value.get("net_colors"))
        .and_then(Value::as_object)
    else {
        return Ok(Vec::new());
    };
    check_count(values.len(), limits.max_net_colors, "net colors")?;
    values
        .iter()
        .map(|(name, value)| {
            let value = project_string(value);
            budget.pair(name, &value)?;
            Ok((name.clone(), value))
        })
        .collect()
}

fn tuning_defaults(settings: Option<&Map<String, Value>>, field: &str) -> ProjectTuningDefaults {
    let value = settings
        .and_then(|value| value.get(field))
        .and_then(Value::as_object);
    ProjectTuningDefaults {
        spacing: value.and_then(|value| optional_number(value, "spacing")),
        min_amplitude: value.and_then(|value| optional_number(value, "min_amplitude")),
        max_amplitude: value.and_then(|value| optional_number(value, "max_amplitude")),
        corner_style: value.and_then(|value| optional_integer(value, "corner_style")),
        corner_radius_percentage: value
            .and_then(|value| optional_integer(value, "corner_radius_percentage")),
        single_sided: value
            .and_then(|value| value.get("single_sided"))
            .and_then(Value::as_bool),
    }
}

fn variants_mut(
    root: &mut Map<String, Value>,
    create: bool,
) -> Result<&mut Vec<Value>, ProjectError> {
    let schematic = object_field(root, "schematic", create)?;
    if create && !schematic.contains_key("variants") {
        schematic.insert("variants".to_owned(), Value::Array(Vec::new()));
    }
    schematic
        .get_mut("variants")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| conflict_error("project schematic.variants field must be a JSON array"))
}

fn variant_exists(root: &Map<String, Value>, name: &str) -> Result<bool, ProjectError> {
    let Some(value) = path(root, &["schematic", "variants"]) else {
        return Ok(false);
    };
    if value.is_null() {
        return Ok(false);
    }
    let values = value
        .as_array()
        .ok_or_else(|| conflict_error("project schematic.variants field must be a JSON array"))?;
    Ok(values.iter().any(|value| {
        value
            .as_object()
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str)
            == Some(name)
    }))
}

fn object_field<'a>(
    root: &'a mut Map<String, Value>,
    field: &str,
    create: bool,
) -> Result<&'a mut Map<String, Value>, ProjectError> {
    if create && !root.contains_key(field) {
        root.insert(field.to_owned(), Value::Object(Map::new()));
    }
    root.get_mut(field)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| conflict_error("project field must be a JSON object"))
}

fn variant_from_value(value: &Value) -> Option<ProjectVariant> {
    let value = value.as_object()?;
    Some(ProjectVariant {
        name: object_string(value, "name"),
        description: match value.get("description") {
            None | Some(Value::Null) => None,
            Some(value) => Some(project_string(value)),
        },
    })
}

fn serialize_project(root: &Map<String, Value>, maximum: usize) -> Result<String, ProjectError> {
    let mut writer = LimitedWriter::new(maximum);
    let mut serializer = serde_json::Serializer::with_formatter(
        &mut writer,
        serde_json::ser::PrettyFormatter::with_indent(b"  "),
    );
    root.serialize(&mut serializer)
        .map_err(|error| writer_error(error, writer.exceeded))?;
    writer.write_all(b"\n").map_err(|error| {
        if writer.exceeded {
            limit_error("project output exceeds max_output_bytes")
        } else {
            io_error(error)
        }
    })?;
    String::from_utf8(writer.bytes).map_err(|_| {
        ProjectError::new(
            ProjectErrorKind::InvalidUtf8,
            "serialized project output is not UTF-8",
        )
    })
}

struct LimitedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl LimitedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            exceeded: false,
        }
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let Some(required) = self.bytes.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(std::io::Error::other("project output size overflows"));
        };
        if required > self.maximum {
            self.exceeded = true;
            return Err(std::io::Error::other("project output exceeds limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct StringBudget {
    used: usize,
    maximum: usize,
}

impl StringBudget {
    const fn new(maximum: usize) -> Self {
        Self { used: 0, maximum }
    }

    fn string(&mut self, value: &str) -> Result<(), ProjectError> {
        self.used = self
            .used
            .checked_add(value.len())
            .ok_or_else(|| limit_error("project typed string bytes overflow"))?;
        check_count(self.used, self.maximum, "typed string bytes")
    }

    fn pair(&mut self, first: &str, second: &str) -> Result<(), ProjectError> {
        self.string(first)?;
        self.string(second)
    }
}

fn path<'a>(root: &'a Map<String, Value>, parts: &[&str]) -> Option<&'a Value> {
    let mut value = root.get(*parts.first()?)?;
    for part in &parts[1..] {
        value = value.as_object()?.get(*part)?;
    }
    Some(value)
}

fn array_field<'a>(settings: Option<&'a Map<String, Value>>, field: &str) -> &'a [Value] {
    settings
        .and_then(|value| value.get(field))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn optional_number(value: &Map<String, Value>, field: &str) -> Option<f64> {
    value.get(field).and_then(Value::as_f64)
}

fn optional_integer(value: &Map<String, Value>, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}

fn object_string(value: &Map<String, Value>, field: &str) -> String {
    value.get(field).map_or_else(String::new, project_string)
}

fn project_string(value: &Value) -> String {
    match value {
        Value::Null | Value::Bool(false) => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) if value.as_f64() == Some(0.0) => String::new(),
        _ => value.to_string(),
    }
}

fn valid_path_parts(dotted_path: &str) -> Result<Vec<&str>, ProjectError> {
    let parts = dotted_path.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(ProjectError::new(
            ProjectErrorKind::InvalidPath,
            "project dotted path must contain only nonempty components",
        ));
    }
    Ok(parts)
}

fn check_mutation_string(value: &str, limits: ProjectLimits) -> Result<(), ProjectError> {
    check_count(
        value.len(),
        limits.max_typed_string_bytes,
        "mutation string bytes",
    )
}

fn check_count(actual: usize, maximum: usize, label: &str) -> Result<(), ProjectError> {
    if actual > maximum {
        return Err(limit_error(format!(
            "project {label} exceed configured limit"
        )));
    }
    Ok(())
}

fn writer_error(error: serde_json::Error, exceeded: bool) -> ProjectError {
    if exceeded {
        limit_error("project output exceeds max_output_bytes")
    } else {
        ProjectError::new(ProjectErrorKind::InvalidJson, error.to_string())
    }
}

fn limit_error(message: impl Into<String>) -> ProjectError {
    ProjectError::new(ProjectErrorKind::ResourceLimit, message)
}

fn conflict_error(message: impl Into<String>) -> ProjectError {
    ProjectError::new(ProjectErrorKind::Conflict, message)
}

fn io_error(error: std::io::Error) -> ProjectError {
    ProjectError::new(ProjectErrorKind::Io, format!("project I/O failed: {error}"))
}
