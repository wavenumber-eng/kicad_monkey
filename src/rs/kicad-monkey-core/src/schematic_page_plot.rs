//! Complete occurrence-aware schematic page Plotter-IR composition.

use std::fmt;

use kicad_monkey_contracts::generated::schematic_plot_document::SchematicPlotDocumentA0;

use crate::{
    PlotDocumentProjectionLimits, PlotProjectionError, PlotterTextCacheResources,
    SchematicBundleIndex, SchematicDrawingSettings, SchematicOccurrence, SchematicPlotContext,
    SchematicPlotDocument, SchematicPlotLimits, SchematicPlotVariables, SourceBundle,
    schematic_plot_document_with_sheets,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchematicOccurrenceSelector {
    Address(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchematicPageContextOverrides {
    pub source_path: Option<String>,
    pub document_id: Option<String>,
    pub sheet_name: Option<String>,
}

pub struct SchematicPagePlotRequest<'resources, 'font> {
    pub selector: SchematicOccurrenceSelector,
    pub context_overrides: SchematicPageContextOverrides,
    pub drawing_settings: SchematicDrawingSettings,
    pub worksheet_source: Option<Vec<u8>>,
    pub variables: SchematicPlotVariables,
    pub text_resources: Option<&'resources PlotterTextCacheResources<'font>>,
    pub plot_limits: SchematicPlotLimits,
}

impl<'resources, 'font> SchematicPagePlotRequest<'resources, 'font> {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            selector: SchematicOccurrenceSelector::Address(address.into()),
            context_overrides: SchematicPageContextOverrides::default(),
            drawing_settings: SchematicDrawingSettings::default(),
            worksheet_source: None,
            variables: SchematicPlotVariables::default(),
            text_resources: None,
            plot_limits: SchematicPlotLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicPagePlotErrorKind {
    BundleMismatch,
    MalformedSelector,
    MissingOccurrence,
    DuplicateOccurrence,
    MissingSource,
    Source,
    Producer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicPagePlotError {
    pub kind: SchematicPagePlotErrorKind,
    message: String,
}

impl SchematicPagePlotError {
    fn new(kind: SchematicPagePlotErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SchematicPagePlotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SchematicPagePlotError {}

#[derive(Clone, Debug)]
pub struct SchematicPagePlotSourceArtifact {
    document: SchematicPlotDocument,
    occurrence_address: String,
    source_path: String,
}

impl SchematicPagePlotSourceArtifact {
    pub fn document(&self) -> &SchematicPlotDocument {
        &self.document
    }

    pub fn occurrence_address(&self) -> &str {
        &self.occurrence_address
    }

    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub(crate) fn into_parts(self) -> (SchematicPlotDocument, String, String) {
        (self.document, self.occurrence_address, self.source_path)
    }
}

#[derive(Clone, Debug)]
pub struct ProjectedSchematicPagePlotArtifact {
    document: SchematicPlotDocumentA0,
    occurrence_address: String,
    source_path: String,
}

impl ProjectedSchematicPagePlotArtifact {
    pub fn document(&self) -> &SchematicPlotDocumentA0 {
        &self.document
    }

    pub fn occurrence_address(&self) -> &str {
        &self.occurrence_address
    }

    pub fn source_path(&self) -> &str {
        &self.source_path
    }
}

pub fn schematic_page_plot_document(
    bundle: &SourceBundle,
    index: &SchematicBundleIndex,
    request: SchematicPagePlotRequest<'_, '_>,
) -> Result<SchematicPagePlotSourceArtifact, SchematicPagePlotError> {
    if !index.bundle_belongs_to(bundle) {
        return Err(SchematicPagePlotError::new(
            SchematicPagePlotErrorKind::BundleMismatch,
            "schematic bundle index was built from a different source bundle",
        ));
    }
    let address = match request.selector {
        SchematicOccurrenceSelector::Address(ref value) => normalize_selector(value)?,
    };
    let mut matches = index
        .occurrences()
        .filter(|occurrence| occurrence.occurrence_address == address);
    let occurrence = matches.next().ok_or_else(|| {
        SchematicPagePlotError::new(
            SchematicPagePlotErrorKind::MissingOccurrence,
            format!("schematic occurrence address {address:?} is absent"),
        )
    })?;
    if matches.next().is_some() {
        return Err(SchematicPagePlotError::new(
            SchematicPagePlotErrorKind::DuplicateOccurrence,
            format!("schematic occurrence address {address:?} is ambiguous"),
        ));
    }
    build_occurrence_document(bundle, index, occurrence, request)
}

pub fn project_schematic_page_plot_artifact_a0(
    source: SchematicPagePlotSourceArtifact,
    limits: PlotDocumentProjectionLimits,
) -> Result<ProjectedSchematicPagePlotArtifact, PlotProjectionError> {
    let (document, occurrence_address, source_path) = source.into_parts();
    let document = crate::project_schematic_plot_document_a0(&document, limits)?;
    Ok(ProjectedSchematicPagePlotArtifact {
        document,
        occurrence_address,
        source_path,
    })
}

fn build_occurrence_document(
    bundle: &SourceBundle,
    index: &SchematicBundleIndex,
    occurrence: &SchematicOccurrence,
    request: SchematicPagePlotRequest<'_, '_>,
) -> Result<SchematicPagePlotSourceArtifact, SchematicPagePlotError> {
    let source = bundle
        .source(&occurrence.source_path)
        .map_err(|error| {
            SchematicPagePlotError::new(SchematicPagePlotErrorKind::Source, error.to_string())
        })?
        .ok_or_else(|| {
            SchematicPagePlotError::new(
                SchematicPagePlotErrorKind::MissingSource,
                format!("schematic source {:?} is absent", occurrence.source_path),
            )
        })?;
    let definition = index.definition(&occurrence.source_path).ok_or_else(|| {
        SchematicPagePlotError::new(
            SchematicPagePlotErrorKind::MissingSource,
            format!(
                "schematic definition {:?} is absent from the index",
                occurrence.source_path
            ),
        )
    })?;
    let source_path = request
        .context_overrides
        .source_path
        .unwrap_or_else(|| occurrence.source_path.clone());
    let document_id = request.context_overrides.document_id.unwrap_or_else(|| {
        definition
            .uuid
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| source_stem(&occurrence.source_path, occurrence.index))
    });
    let sheet_name = request.context_overrides.sheet_name.unwrap_or_else(|| {
        if occurrence.parent_index.is_none() {
            source_stem(&occurrence.source_path, occurrence.index)
        } else {
            occurrence.sheet_name.clone()
        }
    });
    let context = SchematicPlotContext {
        source_path: Some(source_path.clone()),
        document_id: Some(document_id),
        sheet_index: sheet_number(index, occurrence),
        sheet_count: index.occurrences().len(),
        sheet_path: occurrence.human_address.clone(),
        sheet_instance_path: occurrence.occurrence_address.clone(),
        sheet_name,
        project_variables: request.variables,
        worksheet_source: request.worksheet_source,
    };
    let source_text = source.text().map_err(|error| {
        SchematicPagePlotError::new(SchematicPagePlotErrorKind::Source, error.to_string())
    })?;
    let document = schematic_plot_document_with_sheets(
        source_text,
        request.plot_limits,
        &context,
        request.drawing_settings,
        request.text_resources,
    )
    .map_err(|error| {
        SchematicPagePlotError::new(SchematicPagePlotErrorKind::Producer, error.to_string())
    })?;
    Ok(SchematicPagePlotSourceArtifact {
        document,
        occurrence_address: occurrence.occurrence_address.clone(),
        source_path,
    })
}

fn normalize_selector(value: &str) -> Result<String, SchematicPagePlotError> {
    if value == "/" {
        return Ok(value.to_owned());
    }
    if value.is_empty() || !value.starts_with('/') {
        return Err(SchematicPagePlotError::new(
            SchematicPagePlotErrorKind::MalformedSelector,
            "schematic occurrence address must be an absolute non-empty path",
        ));
    }
    Ok(value.strip_suffix('/').unwrap_or(value).to_owned())
}

fn sheet_number(index: &SchematicBundleIndex, occurrence: &SchematicOccurrence) -> usize {
    if let (Some(parent_index), Some(sheet_index)) =
        (occurrence.parent_index, occurrence.parent_sheet_index)
        && let Some(parent) = index.occurrence(parent_index)
        && let Some(definition) = index.definition(&parent.source_path)
        && let Some(number) = definition
            .sheets
            .get(sheet_index)
            .and_then(|sheet| page_number(&sheet.page_instances, &occurrence.occurrence_address))
    {
        return if number == 0 {
            occurrence.index
        } else {
            number
        };
    }
    index
        .definition(&occurrence.source_path)
        .and_then(|definition| {
            page_number(
                &definition.root_page_instances,
                &occurrence.occurrence_address,
            )
        })
        .filter(|number| *number != 0)
        .unwrap_or(occurrence.index)
}

fn page_number(instances: &[crate::SchematicPageInstance], target_path: &str) -> Option<usize> {
    instances
        .iter()
        .find(|instance| normalize_authored_path(&instance.path) == target_path)
        .or_else(|| instances.first())
        .map(|instance| instance.page_number)
}

fn normalize_authored_path(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() { "/" } else { trimmed }
}

fn source_stem(path: &str, index: usize) -> String {
    let filename = path.rsplit('/').next().unwrap_or(path);
    filename
        .strip_suffix(".kicad_sch")
        .filter(|value| !value.is_empty())
        .map_or_else(|| format!("sheet_{index}"), str::to_owned)
}
