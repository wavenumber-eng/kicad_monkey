//! Direct Rust assembly of the nonvisual design-review foundations.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_contracts::generated::source_bundle_manifest::{
    CanonicalUint64Decimal, SourceBundleManifestA0, SourceBundleSource, SourceKind, SourceSlot,
};
use kicad_monkey_core::schematic_embedded::{
    SchematicEmbeddedFile, SchematicEmbeddedLimits, schematic_embedded_files,
};
use kicad_monkey_core::{
    KiCadDesignJsonPaths, KiCadDesignPcb, KiCadDesignSourcePath, KiCadNetlist,
    KiCadNetlistJsonMetadata, KiCadNetlistLimits, KiCadSchematicInstance, PcbLimits, PcbView,
    PlotterTextCacheLimits, PlotterTextCacheResources, PlotterTextFont, ProjectDocument,
    ProjectLimits, SchematicBundleIndex, SchematicBundleLimits, SchematicDocument,
    SchematicDocumentLimits, SchematicDrawingSettings, SchematicPlotContext,
    SchematicPlotContractBudget, SchematicPlotContractLimits, SchematicPlotLimits,
    SchematicPlotVariables, SourceBundle, SourceBundleLimits, TokenKind, build_kicad_design_facts,
    build_kicad_design_json, build_kicad_netlist_json, emit_kicad_netlist,
    schematic_plot_document_budget, schematic_plot_document_json,
    schematic_plot_document_with_sheets, validate_compiled_schematic_graph,
};
use sha2::{Digest, Sha256};

const GRAPH_TOOL: &str = "kicad_cruncher";
const KICAD_STROKE_REGULAR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke.ttf"
));
const KICAD_STROKE_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke-italic.ttf"
));
const KICAD_STROKE_BOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke-bold.ttf"
));
const KICAD_STROKE_BOLD_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../assets/fonts/kicad-stroke-bold-italic.ttf"
));

struct SchematicFontStyle<'a> {
    face: String,
    bold: bool,
    italic: bool,
    name: String,
    bytes: Cow<'a, [u8]>,
    sha256: String,
    fake_bold: bool,
    fake_italic: bool,
}

struct EmbeddedFontCandidate<'a> {
    file: &'a SchematicEmbeddedFile,
    families: Vec<String>,
    bold: bool,
    italic: bool,
    sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicPlotDocumentsLimits {
    pub max_documents: usize,
    pub max_total_derived_items: usize,
    pub max_total_materialized_bytes: usize,
    pub max_total_output_bytes: usize,
    pub per_document: SchematicPlotContractLimits,
}

impl Default for SchematicPlotDocumentsLimits {
    fn default() -> Self {
        Self {
            max_documents: 4_096,
            max_total_derived_items: 64_000_000,
            max_total_materialized_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
            max_total_output_bytes: 512 * 1024 * 1024,
            per_document: SchematicPlotContractLimits::default(),
        }
    }
}

#[derive(Debug)]
pub struct DesignError {
    message: String,
}

impl DesignError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn context(context: &str, error: impl fmt::Display) -> Self {
        Self::new(format!("{context}: {error}"))
    }
}

impl fmt::Display for DesignError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DesignError {}

#[derive(Debug)]
pub struct LoadedDesignSources {
    pub input_path: PathBuf,
    pub bundle_root: PathBuf,
    pub bundle: SourceBundle,
    pub pcb_path: Option<PathBuf>,
    pub pcb_source: Option<String>,
}

#[derive(Debug)]
pub struct StructuredDesignFacts {
    pub compiled_schematic_graph: CompiledSchematicGraphA0,
    pub netlist: KiCadNetlist,
    pub netlist_json: serde_json::Value,
    pub design_json: serde_json::Value,
    pub kicad_netlist: String,
    pub schematic_instances: Vec<KiCadSchematicInstance>,
}

struct DesignPaths {
    input: PathBuf,
    root: PathBuf,
    project: Option<PathBuf>,
    root_schematic: PathBuf,
    pcb: Option<PathBuf>,
}

fn resolve_design_paths(input: &Path) -> Result<DesignPaths, DesignError> {
    let input_path = input
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve design input", error))?;
    if !input_path.is_file() {
        return Err(DesignError::new("design input is not a regular file"));
    }

    let suffix = input_path.extension().and_then(|value| value.to_str());
    let (project_path, root_schematic_path, pcb_path) = match suffix {
        Some("kicad_pro") => {
            let schematic = input_path.with_extension("kicad_sch");
            if !schematic.is_file() {
                return Err(DesignError::new(format!(
                    "project root schematic was not found: {}",
                    schematic.display()
                )));
            }
            let pcb = input_path.with_extension("kicad_pcb");
            (
                Some(input_path.clone()),
                schematic,
                pcb.is_file().then_some(pcb),
            )
        }
        Some("kicad_sch") => (
            find_adjacent_project(&input_path)?,
            input_path.clone(),
            None,
        ),
        _ => {
            return Err(DesignError::new(
                "design input must end in .kicad_pro or .kicad_sch",
            ));
        }
    };

    let bundle_root = input_path
        .parent()
        .ok_or_else(|| DesignError::new("design input has no parent directory"))?
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve bundle root", error))?;
    let root_schematic_path = root_schematic_path
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve root schematic", error))?;
    ensure_contained(&bundle_root, &root_schematic_path)?;
    Ok(DesignPaths {
        input: input_path,
        root: bundle_root,
        project: project_path,
        root_schematic: root_schematic_path,
        pcb: pcb_path,
    })
}

fn find_adjacent_project(source: &Path) -> Result<Option<PathBuf>, DesignError> {
    let exact = source.with_extension("kicad_pro");
    if exact.is_file() {
        return Ok(Some(exact));
    }
    let parent = source
        .parent()
        .ok_or_else(|| DesignError::new("design input has no parent directory"))?;
    let mut siblings = Vec::new();
    for entry in parent
        .read_dir()
        .map_err(|error| DesignError::context("could not inspect design directory", error))?
    {
        let path = entry
            .map_err(|error| DesignError::context("could not inspect design directory", error))?
            .path();
        if path.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("kicad_pro"))
        {
            if siblings.len() >= 4_096 {
                return Err(DesignError::new(
                    "design directory exceeds adjacent-project candidate limit",
                ));
            }
            siblings.push(path);
        }
    }
    siblings.sort();
    if siblings.len() == 1 {
        return Ok(siblings.pop());
    }
    let source_stem = source.file_stem().and_then(|stem| stem.to_str());
    Ok(source_stem.and_then(|stem| {
        siblings.into_iter().find(|candidate| {
            candidate
                .file_stem()
                .and_then(|candidate_stem| candidate_stem.to_str())
                .is_some_and(|candidate_stem| candidate_stem.eq_ignore_ascii_case(stem))
        })
    }))
}

pub fn load_design_sources(input: &Path) -> Result<LoadedDesignSources, DesignError> {
    let paths = resolve_design_paths(input)?;

    let limits = SourceBundleLimits::default();
    let schematic_limits = SchematicDocumentLimits {
        parse: SchematicBundleLimits::default(),
        max_output_bytes: limits.max_source_bytes,
    };
    let mut source_rows = Vec::new();
    let mut buffers = Vec::new();
    let mut total_bytes = 0_usize;

    if let Some(project_path) = paths.project.as_ref() {
        let relative = portable_relative(&paths.root, project_path)?;
        let bytes = read_bounded(project_path, limits.max_source_bytes)?;
        push_source(
            &mut source_rows,
            &mut buffers,
            &mut total_bytes,
            relative,
            SourceKind::Project,
            bytes,
            limits,
        )?;
    }

    let root_relative = portable_relative(&paths.root, &paths.root_schematic)?;
    let mut pending = VecDeque::from([paths.root_schematic]);
    let mut seen = HashSet::new();
    while let Some(source_path) = pending.pop_front() {
        let relative = portable_relative(&paths.root, &source_path)?;
        if !seen.insert(relative.clone()) {
            continue;
        }
        let source = SchematicDocument::from_named_reader(
            &relative,
            File::open(&source_path)
                .map_err(|error| DesignError::context("could not open schematic", error))?,
            schematic_limits,
        )
        .map_err(|error| DesignError::context("could not parse schematic", error))?;
        let definition = source
            .definition()
            .map_err(|error| DesignError::context("could not inspect schematic sheets", error))?;
        let source_parent = source_path
            .parent()
            .ok_or_else(|| DesignError::new("schematic has no parent directory"))?;
        for sheet in definition.sheets {
            let child = resolve_child_schematic(&paths.root, source_parent, &sheet.sheet_file)?;
            pending.push_back(child);
        }
        push_source(
            &mut source_rows,
            &mut buffers,
            &mut total_bytes,
            relative,
            SourceKind::Schematic,
            source.into_source().into_bytes(),
            limits,
        )?;
    }

    let project_relative = paths
        .project
        .as_ref()
        .map(|path| portable_relative(&paths.root, path))
        .transpose()?;
    let manifest = SourceBundleManifestA0 {
        project_path: project_relative,
        root_schematic_path: root_relative,
        schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
        sources: source_rows,
        type_: "kicad_monkey.source_bundle_manifest".to_owned(),
        version: "a0".to_owned(),
    };
    let bundle = SourceBundle::from_manifest(manifest, buffers, limits)
        .map_err(|error| DesignError::context("source bundle is invalid", error))?;
    let (pcb_path, pcb_source) = paths
        .pcb
        .map(|path| {
            let bytes = read_bounded(&path, limits.max_source_bytes)?;
            let source = String::from_utf8(bytes)
                .map_err(|error| DesignError::context("PCB source is not UTF-8", error))?;
            Ok::<_, DesignError>((path, source))
        })
        .transpose()?
        .map_or((None, None), |(path, source)| (Some(path), Some(source)));
    Ok(LoadedDesignSources {
        input_path: paths.input,
        bundle_root: paths.root,
        bundle,
        pcb_path,
        pcb_source,
    })
}

pub fn build_structured_design_facts(
    loaded: &LoadedDesignSources,
) -> Result<StructuredDesignFacts, DesignError> {
    build_structured_design_facts_with_options(loaded, true)
}

pub fn build_structured_design_facts_with_options(
    loaded: &LoadedDesignSources,
    include_indexes: bool,
) -> Result<StructuredDesignFacts, DesignError> {
    let index = SchematicBundleIndex::build(&loaded.bundle, SchematicBundleLimits::default())
        .map_err(|error| DesignError::context("could not index schematic hierarchy", error))?;
    let design_facts = build_kicad_design_facts(
        &index,
        &loaded.bundle,
        ProjectLimits::default(),
        Default::default(),
        KiCadNetlistLimits::default(),
    )
    .map_err(|error| DesignError::context("could not build structured design facts", error))?;
    validate_compiled_schematic_graph(design_facts.graph())
        .map_err(|error| DesignError::context("compiled schematic graph is invalid", error))?;
    let netlist = design_facts.netlist();
    let kicad_netlist = emit_kicad_netlist(
        netlist,
        loaded.bundle.root_schematic_path(),
        "",
        GRAPH_TOOL,
        KiCadNetlistLimits::default().max_output_bytes,
    )
    .map_err(|error| DesignError::context("could not emit KiCad netlist", error))?;
    let netlist_json = build_kicad_netlist_json(
        netlist,
        KiCadNetlistJsonMetadata {
            source: "",
            date: "",
            tool: "kicad_monkey",
        },
    );
    let pcb_view = loaded
        .pcb_source
        .as_deref()
        .map(|source| PcbView::parse(source, PcbLimits::default()))
        .transpose()
        .map_err(|error| DesignError::context("could not parse PCB", error))?;
    let pcb_filename = loaded
        .pcb_path
        .as_deref()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let pcb = pcb_view
        .as_ref()
        .zip(pcb_filename)
        .map(|(view, filename)| KiCadDesignPcb {
            source_filename: filename,
            view,
        });
    let design_json = build_kicad_design_json(
        &index,
        &design_facts,
        &design_json_paths(loaded)?,
        pcb,
        include_indexes,
    )
    .map_err(|error| DesignError::context("could not build KiCad design JSON", error))?;
    let schematic_instances = design_facts
        .schematic_instances()
        .map_err(|error| DesignError::context("could not enumerate schematic instances", error))?;
    let (compiled_schematic_graph, netlist) = design_facts.into_parts();
    Ok(StructuredDesignFacts {
        compiled_schematic_graph,
        netlist,
        netlist_json,
        design_json,
        kicad_netlist,
        schematic_instances,
    })
}

pub fn build_schematic_plot_documents(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
) -> Result<Vec<serde_json::Value>, DesignError> {
    build_schematic_plot_documents_with_limits(
        loaded,
        instances,
        SchematicPlotDocumentsLimits::default(),
    )
}

pub fn build_schematic_plot_documents_with_limits(
    loaded: &LoadedDesignSources,
    instances: &[KiCadSchematicInstance],
    limits: SchematicPlotDocumentsLimits,
) -> Result<Vec<serde_json::Value>, DesignError> {
    if instances.len() > limits.max_documents {
        return Err(DesignError::new(format!(
            "schematic plot document count exceeds its limit: {} > {}",
            instances.len(),
            limits.max_documents
        )));
    }
    if limits.max_total_output_bytes < 2 {
        return Err(DesignError::new(
            "schematic plot aggregate output limit is smaller than an empty JSON array",
        ));
    }
    let embedded_files = design_embedded_files(loaded)?;
    let (variables, drawing_settings, worksheet_source) =
        project_plot_sidecars(loaded, &embedded_files)?;
    let font_faces = requested_font_faces(loaded, worksheet_source.as_deref())?;
    let font_styles = schematic_font_styles(&font_faces, &embedded_files)?;
    let mut fonts = Vec::with_capacity(font_styles.len());
    for style in &font_styles {
        fonts.push(PlotterTextFont {
            face: &style.face,
            bold: style.bold,
            italic: style.italic,
            font_bytes: style.bytes.as_ref(),
            shaping: shaping_template(
                &format!("schematic_{}_{}", style.name, style.sha256),
                &style.sha256,
            )?,
            fake_bold: style.fake_bold,
            fake_italic: style.fake_italic,
        });
    }
    fonts.sort_by(|left, right| {
        (left.face, left.bold, left.italic).cmp(&(right.face, right.bold, right.italic))
    });
    let text_resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let mut documents = Vec::with_capacity(instances.len());
    let mut total_output_bytes = 2_usize;
    let mut batch_budget = SchematicPlotContractBudget {
        derived_items: 0,
        materialized_bytes: 0,
    };
    for (document_index, instance) in instances.iter().enumerate() {
        if document_index != 0 {
            total_output_bytes = total_output_bytes.checked_add(1).ok_or_else(|| {
                DesignError::new("schematic plot aggregate output byte count overflowed")
            })?;
        }
        let source = loaded
            .bundle
            .source(&instance.source_path)
            .map_err(|error| DesignError::context("could not resolve schematic source", error))?
            .ok_or_else(|| {
                DesignError::new(format!(
                    "schematic instance source is missing: {}",
                    instance.source_path
                ))
            })?;
        let source_path = loaded.bundle_root.join(source.path());
        let context = SchematicPlotContext {
            source_path: Some(display_source_path(&source_path)),
            document_id: Some(instance.document_id.clone()),
            sheet_index: instance.sheet_number,
            sheet_count: instance.sheet_count,
            sheet_path: instance.sheet_path.clone(),
            sheet_instance_path: instance.sheet_instance_path.clone(),
            sheet_name: instance.sheet_name.clone(),
            project_variables: variables.clone(),
            worksheet_source: worksheet_source.clone(),
        };
        let document = schematic_plot_document_with_sheets(
            source
                .text()
                .map_err(|error| DesignError::context("schematic source is not UTF-8", error))?,
            SchematicPlotLimits::default(),
            &context,
            drawing_settings,
            Some(&text_resources),
        )
        .map_err(|error| DesignError::context("could not build schematic plot document", error))?;
        let document_budget = schematic_plot_document_budget(&document).map_err(|error| {
            DesignError::context("could not budget schematic plot document", error)
        })?;
        charge_plot_batch_budget(&mut batch_budget, document_budget, limits)?;
        let value =
            schematic_plot_document_json(&document, limits.per_document).map_err(|error| {
                DesignError::context("could not project schematic plot document", error)
            })?;
        let mut writer =
            AggregateLimitedWriter::new(&mut total_output_bytes, limits.max_total_output_bytes);
        serde_json::to_writer(&mut writer, &value).map_err(|error| {
            DesignError::context("schematic plot aggregate output limit exceeded", error)
        })?;
        documents.push(value);
    }
    Ok(documents)
}

fn charge_plot_batch_budget(
    total: &mut SchematicPlotContractBudget,
    document: SchematicPlotContractBudget,
    limits: SchematicPlotDocumentsLimits,
) -> Result<(), DesignError> {
    let derived_items = total
        .derived_items
        .checked_add(document.derived_items)
        .ok_or_else(|| DesignError::new("schematic plot aggregate item count overflowed"))?;
    let materialized_bytes = total
        .materialized_bytes
        .checked_add(document.materialized_bytes)
        .ok_or_else(|| DesignError::new("schematic plot aggregate byte count overflowed"))?;
    if derived_items > limits.max_total_derived_items {
        return Err(DesignError::new(format!(
            "schematic plot aggregate derived item limit exceeded: {derived_items} > {}",
            limits.max_total_derived_items
        )));
    }
    if materialized_bytes > limits.max_total_materialized_bytes {
        return Err(DesignError::new(format!(
            "schematic plot aggregate materialized byte limit exceeded: {materialized_bytes} > {}",
            limits.max_total_materialized_bytes
        )));
    }
    total.derived_items = derived_items;
    total.materialized_bytes = materialized_bytes;
    Ok(())
}

fn project_plot_sidecars(
    loaded: &LoadedDesignSources,
    embedded_files: &[SchematicEmbeddedFile],
) -> Result<
    (
        SchematicPlotVariables,
        SchematicDrawingSettings,
        Option<Vec<u8>>,
    ),
    DesignError,
> {
    let project = loaded
        .bundle
        .project()
        .map(|source| ProjectDocument::from_reader(source.bytes(), ProjectLimits::default()))
        .transpose()
        .map_err(|error| {
            DesignError::context("could not parse schematic project settings", error)
        })?;
    let project = project.as_ref().map(ProjectDocument::view);
    let variables = project
        .map(|view| view.text_variables())
        .transpose()
        .map_err(|error| DesignError::context("could not read schematic text variables", error))?
        .unwrap_or_default();
    let drawing_settings = project.map_or_else(SchematicDrawingSettings::default, |view| {
        view.schematic_drawing_settings()
    });
    let worksheet_source = project
        .and_then(|view| view.get_path("schematic.page_layout_descr_file"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| load_project_worksheet(loaded, embedded_files, value))
        .transpose()?
        .flatten();
    Ok((
        SchematicPlotVariables::from_entries(variables),
        drawing_settings,
        worksheet_source,
    ))
}

fn design_embedded_files(
    loaded: &LoadedDesignSources,
) -> Result<Vec<SchematicEmbeddedFile>, DesignError> {
    let limits = SchematicEmbeddedLimits::default();
    let mut files = Vec::new();
    let mut decoded_bytes = 0_usize;
    for source in loaded.bundle.sources() {
        if source.kind() != SourceKind::Schematic {
            continue;
        }
        let extracted = schematic_embedded_files(
            source
                .text()
                .map_err(|error| DesignError::context("schematic source is not UTF-8", error))?,
            limits,
        )
        .map_err(|error| DesignError::context("could not extract schematic sidecars", error))?;
        for file in extracted {
            decoded_bytes = decoded_bytes
                .checked_add(file.bytes.len())
                .ok_or_else(|| DesignError::new("design embedded sidecar byte count overflowed"))?;
            if files.len() >= limits.max_files || decoded_bytes > limits.max_decoded_bytes {
                return Err(DesignError::new(
                    "design embedded sidecars exceed their aggregate limit",
                ));
            }
            files.push(file);
        }
    }
    Ok(files)
}

struct AggregateLimitedWriter<'a> {
    written: &'a mut usize,
    limit: usize,
}

impl<'a> AggregateLimitedWriter<'a> {
    const fn new(written: &'a mut usize, limit: usize) -> Self {
        Self { written, limit }
    }
}

impl io::Write for AggregateLimitedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(buffer.len())
            .filter(|next| *next <= self.limit)
            .ok_or_else(|| io::Error::other(format!("output exceeds {} bytes", self.limit)))?;
        *self.written = next;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn load_project_worksheet(
    loaded: &LoadedDesignSources,
    embedded_files: &[SchematicEmbeddedFile],
    worksheet: &str,
) -> Result<Option<Vec<u8>>, DesignError> {
    if let Some(name) = worksheet.strip_prefix("kicad-embed://") {
        return Ok(embedded_files
            .iter()
            .find(|file| file.file_type == "worksheet" && file.name == name)
            .map(|file| file.bytes.clone()));
    }
    let relative = Path::new(worksheet);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, Component::Prefix(_)))
    {
        return Err(DesignError::new(
            "project worksheet path must remain inside the design bundle",
        ));
    }
    let unresolved = loaded.bundle_root.join(relative);
    if !unresolved.exists() {
        return Ok(None);
    }
    let path = unresolved.canonicalize().map_err(|error| {
        DesignError::context(
            &format!("could not resolve project worksheet {worksheet}"),
            error,
        )
    })?;
    ensure_contained(&loaded.bundle_root, &path)?;
    read_bounded(&path, SchematicPlotLimits::default().max_worksheet_bytes).map(Some)
}

fn requested_font_faces(
    loaded: &LoadedDesignSources,
    worksheet_source: Option<&[u8]>,
) -> Result<BTreeSet<String>, DesignError> {
    let mut faces = BTreeSet::from([String::new(), "Arial".to_owned()]);
    for source in loaded.bundle.sources() {
        if source.kind() == SourceKind::Schematic {
            collect_font_faces(
                source.text().map_err(|error| {
                    DesignError::context("schematic source is not UTF-8", error)
                })?,
                &mut faces,
            )?;
        }
    }
    if let Some(source) = worksheet_source {
        let source = std::str::from_utf8(source)
            .map_err(|error| DesignError::context("worksheet source is not UTF-8", error))?;
        collect_font_faces(source, &mut faces)?;
    }
    Ok(faces)
}

fn collect_font_faces(source: &str, faces: &mut BTreeSet<String>) -> Result<(), DesignError> {
    const MAX_FACES: usize = 64;
    const MAX_FACE_BYTES: usize = 64 * 1024;
    let mut lexer = kicad_monkey_core::Lexer::new(source);
    let mut expecting_head = false;
    let mut expecting_face = false;
    while let Some(token) = lexer
        .next()
        .transpose()
        .map_err(|error| DesignError::context("could not scan plot font faces", error))?
    {
        match token.kind {
            TokenKind::Left => {
                expecting_head = true;
                expecting_face = false;
            }
            TokenKind::Right => {
                expecting_head = false;
                expecting_face = false;
            }
            _ if expecting_head => {
                expecting_face = token.lexeme == "face";
                expecting_head = false;
            }
            TokenKind::QuotedString if expecting_face => {
                expecting_face = false;
                let face = serde_json::from_str::<String>(token.lexeme).map_err(|error| {
                    DesignError::context("could not decode plot font face", error)
                })?;
                if face.len() > MAX_FACE_BYTES {
                    return Err(DesignError::new("plot font face exceeds its byte limit"));
                }
                faces.insert(face);
                if faces.len() > MAX_FACES {
                    return Err(DesignError::new("plot font face count exceeds its limit"));
                }
                if faces.iter().map(String::len).sum::<usize>() > MAX_FACE_BYTES {
                    return Err(DesignError::new(
                        "plot font face names exceed their aggregate byte limit",
                    ));
                }
            }
            _ => expecting_face = false,
        }
    }
    Ok(())
}

fn schematic_font_styles<'a>(
    faces: &BTreeSet<String>,
    embedded_files: &'a [SchematicEmbeddedFile],
) -> Result<Vec<SchematicFontStyle<'a>>, DesignError> {
    schematic_font_styles_with_limits(faces, embedded_files, PlotterTextCacheLimits::default())
}

fn schematic_font_styles_with_limits<'a>(
    faces: &BTreeSet<String>,
    embedded_files: &'a [SchematicEmbeddedFile],
    font_limits: PlotterTextCacheLimits,
) -> Result<Vec<SchematicFontStyle<'a>>, DesignError> {
    let embedded_fonts = embedded_font_index(embedded_files)?;
    let mut styles = Vec::with_capacity(faces.len().saturating_mul(4));
    let mut retained_font_bytes = 0_usize;
    for face in faces {
        for (bold, italic, name) in [
            (false, false, "regular"),
            (false, true, "italic"),
            (true, false, "bold"),
            (true, true, "bold_italic"),
        ] {
            let style = resolve_schematic_font(face, bold, italic, name, &embedded_fonts)?;
            retained_font_bytes = retained_font_bytes
                .checked_add(style.bytes.len())
                .filter(|bytes| *bytes <= font_limits.max_font_bytes)
                .ok_or_else(|| DesignError::new("plot font bytes exceed their aggregate limit"))?;
            styles.push(style);
        }
    }
    Ok(styles)
}

fn resolve_schematic_font<'a>(
    face: &str,
    bold: bool,
    italic: bool,
    name: &str,
    embedded_fonts: &[EmbeddedFontCandidate<'a>],
) -> Result<SchematicFontStyle<'a>, DesignError> {
    if let Some((candidate, selected_bold, selected_italic)) =
        embedded_font_selection(face, bold, italic, embedded_fonts)
    {
        let file = candidate.file;
        return Ok(SchematicFontStyle {
            face: face.to_owned(),
            bold,
            italic,
            name: name.to_owned(),
            bytes: Cow::Borrowed(&file.bytes),
            sha256: candidate.sha256.clone(),
            fake_bold: bold && !selected_bold,
            fake_italic: italic && !selected_italic,
        });
    }
    let system_root = std::env::var_os("SystemRoot").map(PathBuf::from);
    if let Some(font_dir) = system_root.map(|root| root.join("Fonts")) {
        let (filename, selected_bold, selected_italic) =
            windows_font_selection(face, bold, italic, &font_dir);
        let path = font_dir.join(filename);
        if path.is_file() {
            let bytes = read_bounded(&path, PlotterTextCacheLimits::default().max_face_bytes)?;
            let sha256 = sha256_hex(&bytes);
            return Ok(SchematicFontStyle {
                face: face.to_owned(),
                bold,
                italic,
                name: name.to_owned(),
                bytes: Cow::Owned(bytes),
                sha256,
                fake_bold: bold && !selected_bold,
                fake_italic: italic && !selected_italic,
            });
        }
    }
    let source = match (bold, italic) {
        (false, false) => KICAD_STROKE_REGULAR,
        (false, true) => KICAD_STROKE_ITALIC,
        (true, false) => KICAD_STROKE_BOLD,
        (true, true) => KICAD_STROKE_BOLD_ITALIC,
    };
    let sha256 = sha256_hex(source);
    Ok(SchematicFontStyle {
        face: face.to_owned(),
        bold,
        italic,
        name: name.to_owned(),
        bytes: Cow::Borrowed(source),
        sha256,
        fake_bold: false,
        fake_italic: false,
    })
}

fn embedded_font_selection<'index, 'files>(
    requested_face: &str,
    bold: bool,
    italic: bool,
    embedded_fonts: &'index [EmbeddedFontCandidate<'files>],
) -> Option<(&'index EmbeddedFontCandidate<'files>, bool, bool)> {
    let requested = normalized_font_name(requested_face);
    if requested.is_empty() {
        return None;
    }
    for (candidate_bold, candidate_italic) in font_style_lookup_order(bold, italic) {
        if let Some(candidate) = embedded_fonts.iter().rev().find(|candidate| {
            candidate.bold == candidate_bold
                && candidate.italic == candidate_italic
                && candidate.families.iter().any(|family| family == &requested)
        }) {
            return Some((candidate, candidate_bold, candidate_italic));
        }
    }
    None
}

fn embedded_font_index(
    embedded_files: &[SchematicEmbeddedFile],
) -> Result<Vec<EmbeddedFontCandidate<'_>>, DesignError> {
    const MAX_NAME_RECORDS: usize = 65_536;
    const MAX_FAMILY_ALIASES: usize = 4_096;
    const MAX_FAMILY_BYTES: usize = 256 * 1024;
    let mut name_records = 0_usize;
    let mut family_aliases = 0_usize;
    let mut family_bytes = 0_usize;
    let mut candidates = Vec::new();
    for file in embedded_files
        .iter()
        .filter(|file| file.file_type.eq_ignore_ascii_case("font"))
    {
        if file.bytes.len() > PlotterTextCacheLimits::default().max_face_bytes {
            return Err(DesignError::new(
                "embedded font face exceeds its byte limit",
            ));
        }
        let Ok(face) = ttf_parser::Face::parse(&file.bytes, 0) else {
            continue;
        };
        let mut families = BTreeSet::new();
        for name in face.names() {
            name_records = name_records
                .checked_add(1)
                .filter(|count| *count <= MAX_NAME_RECORDS)
                .ok_or_else(|| DesignError::new("embedded font name records exceed their limit"))?;
            if !matches!(
                name.name_id,
                ttf_parser::name_id::FAMILY | ttf_parser::name_id::TYPOGRAPHIC_FAMILY
            ) {
                continue;
            }
            if let Some(name) = name.to_string() {
                push_font_family(
                    &mut families,
                    &name,
                    &mut family_aliases,
                    &mut family_bytes,
                    MAX_FAMILY_ALIASES,
                    MAX_FAMILY_BYTES,
                )?;
            }
        }
        if let Some(stem) = Path::new(&file.name)
            .file_stem()
            .and_then(|stem| stem.to_str())
        {
            push_font_family(
                &mut families,
                strip_font_style_suffix(stem),
                &mut family_aliases,
                &mut family_bytes,
                MAX_FAMILY_ALIASES,
                MAX_FAMILY_BYTES,
            )?;
        }
        if !families.is_empty() {
            candidates.push(EmbeddedFontCandidate {
                file,
                families: families.into_iter().collect(),
                bold: face.is_bold(),
                italic: face.is_italic(),
                sha256: sha256_hex(&file.bytes),
            });
        }
    }
    Ok(candidates)
}

fn push_font_family(
    families: &mut BTreeSet<String>,
    value: &str,
    aliases: &mut usize,
    bytes: &mut usize,
    max_aliases: usize,
    max_bytes: usize,
) -> Result<(), DesignError> {
    let normalized = normalized_font_name(value);
    if normalized.is_empty() || families.contains(&normalized) {
        return Ok(());
    }
    *aliases = aliases
        .checked_add(1)
        .filter(|count| *count <= max_aliases)
        .ok_or_else(|| DesignError::new("embedded font family aliases exceed their limit"))?;
    *bytes = bytes
        .checked_add(normalized.len())
        .filter(|count| *count <= max_bytes)
        .ok_or_else(|| DesignError::new("embedded font family names exceed their byte limit"))?;
    families.insert(normalized);
    Ok(())
}

fn font_style_lookup_order(bold: bool, italic: bool) -> Vec<(bool, bool)> {
    let mut order = vec![(bold, italic)];
    if bold && italic {
        order.extend([(true, false), (false, true)]);
    }
    order.push((false, false));
    order.dedup();
    order
}

fn normalized_font_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn strip_font_style_suffix(value: &str) -> &str {
    const SUFFIXES: [&str; 7] = [
        "regular", "medium", "bold", "italic", "semibold", "demibold", "black",
    ];
    let lower = value.to_ascii_lowercase();
    for suffix in SUFFIXES {
        if let Some(prefix) = lower.strip_suffix(suffix) {
            let cut = prefix.trim_end_matches(['-', '_', ' ']).len();
            return &value[..cut];
        }
    }
    value
}

fn windows_font_selection(
    face: &str,
    bold: bool,
    italic: bool,
    font_dir: &Path,
) -> (&'static str, bool, bool) {
    let normalized = face.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.eq_ignore_ascii_case("Fragment Mono") {
        return match (bold, italic) {
            (false, false) if font_dir.join("CascadiaCode.ttf").is_file() => {
                ("CascadiaCode.ttf", false, false)
            }
            (true, false | true) if font_dir.join("BOOKOSB.TTF").is_file() => {
                ("BOOKOSB.TTF", true, false)
            }
            _ => ("arial.ttf", false, false),
        };
    }
    if normalized.eq_ignore_ascii_case("Berkeley Mono") {
        let candidate = if bold {
            "BerkeleyMono-Bold.ttf"
        } else {
            "BerkeleyMono-Regular.ttf"
        };
        if font_dir.join(candidate).is_file() {
            return (candidate, bold, false);
        }
    }
    if bold {
        ("arialbd.ttf", true, false)
    } else {
        ("arial.ttf", false, false)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len().saturating_mul(2));
    for byte in digest {
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}

fn shaping_template(font_id: &str, sha256: &str) -> Result<ShapingInput, DesignError> {
    serde_json::from_value(serde_json::json!({
        "font_id": font_id,
        "font_sha256": sha256,
        "face_index": 0,
        "variations": [],
        "text": "",
        "text_index_unit": "utf8_byte_offset",
        "scale_x": 1000,
        "scale_y": 1000,
        "direction": "left_to_right",
        "script": "Latn",
        "language": "en",
        "features": [],
        "buffer_properties": {
            "cluster_level": "monotone_graphemes",
            "beginning_of_text": true,
            "end_of_text": true,
            "default_ignorables": "normal",
            "do_not_insert_dotted_circle": false,
            "produce_unsafe_to_concat": false,
            "produce_safe_to_insert_tatweel": false
        }
    }))
    .map_err(|error| DesignError::context("could not build default shaping template", error))
}

fn design_json_paths(loaded: &LoadedDesignSources) -> Result<KiCadDesignJsonPaths, DesignError> {
    let mut paths = KiCadDesignJsonPaths::default();
    for source in loaded.bundle.sources() {
        let absolute = loaded.bundle_root.join(source.path());
        let absolute = absolute.canonicalize().map_err(|error| {
            DesignError::context("could not resolve design JSON source path", error)
        })?;
        ensure_contained(&loaded.bundle_root, &absolute)?;
        let filename = absolute
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| DesignError::new("design source filename is not valid Unicode"))?
            .to_owned();
        let path = display_source_path(&absolute);
        match source.kind() {
            SourceKind::Project => {
                paths.project_name = absolute
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned);
                paths.project_filename = Some(filename);
                paths.project_path = Some(path);
            }
            SourceKind::Schematic => {
                paths.schematic_paths.insert(
                    source.path().to_owned(),
                    KiCadDesignSourcePath { filename, path },
                );
            }
            _ => {}
        }
    }
    Ok(paths)
}

fn display_source_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(value) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{value}")
    } else if let Some(value) = value.strip_prefix(r"\\?\") {
        value.to_owned()
    } else {
        value.into_owned()
    }
}

fn resolve_child_schematic(
    bundle_root: &Path,
    source_parent: &Path,
    sheet_file: &str,
) -> Result<PathBuf, DesignError> {
    if sheet_file.is_empty() {
        return Err(DesignError::new("schematic sheet has an empty file path"));
    }
    let raw = Path::new(sheet_file);
    if raw.is_absolute()
        || raw
            .components()
            .any(|part| matches!(part, Component::Prefix(_)))
    {
        return Err(DesignError::new(format!(
            "schematic sheet path is not relative: {sheet_file}"
        )));
    }
    let child = source_parent.join(raw).canonicalize().map_err(|error| {
        DesignError::context(
            &format!("could not resolve child schematic {sheet_file}"),
            error,
        )
    })?;
    ensure_contained(bundle_root, &child)?;
    if !child.is_file() {
        return Err(DesignError::new(format!(
            "child schematic is not a regular file: {}",
            child.display()
        )));
    }
    Ok(child)
}

fn ensure_contained(root: &Path, path: &Path) -> Result<(), DesignError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(DesignError::new(format!(
            "design source escapes bundle root: {}",
            path.display()
        )))
    }
}

fn portable_relative(root: &Path, path: &Path) -> Result<String, DesignError> {
    ensure_contained(root, path)?;
    let relative = path
        .strip_prefix(root)
        .map_err(|error| DesignError::context("could not make source path relative", error))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => parts.push(
                value
                    .to_str()
                    .ok_or_else(|| DesignError::new("source path is not valid Unicode"))?,
            ),
            _ => return Err(DesignError::new("source path is not portable")),
        }
    }
    if parts.is_empty() {
        return Err(DesignError::new(
            "source path is empty relative to bundle root",
        ));
    }
    Ok(parts.join("/"))
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, DesignError> {
    let mut file = File::open(path)
        .map_err(|error| DesignError::context("could not open design source", error))?;
    let read_limit = maximum
        .checked_add(1)
        .ok_or_else(|| DesignError::new("source byte limit overflowed"))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(read_limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| DesignError::context("could not read design source", error))?;
    if bytes.len() > maximum {
        return Err(DesignError::new(format!(
            "design source exceeds byte limit: {}",
            path.display()
        )));
    }
    Ok(bytes)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one manifest row and byte slot are appended atomically"
)]
fn push_source(
    rows: &mut Vec<SourceBundleSource>,
    buffers: &mut Vec<Vec<u8>>,
    total_bytes: &mut usize,
    path: String,
    kind: SourceKind,
    bytes: Vec<u8>,
    limits: SourceBundleLimits,
) -> Result<(), DesignError> {
    if rows.len() >= limits.max_sources {
        return Err(DesignError::new("design source count exceeds bundle limit"));
    }
    *total_bytes = total_bytes
        .checked_add(bytes.len())
        .filter(|total| *total <= limits.max_total_bytes)
        .ok_or_else(|| DesignError::new("design source bytes exceed bundle limit"))?;
    let slot = u32::try_from(rows.len())
        .map_err(|_| DesignError::new("design source slot does not fit u32"))?;
    rows.push(SourceBundleSource {
        kind,
        path,
        slot: SourceSlot(slot),
        source_bytes: CanonicalUint64Decimal(bytes.len().to_string()),
    });
    buffers.push(bytes);
    Ok(())
}

#[cfg(test)]
mod plot_batch_budget_tests {
    use super::*;

    #[test]
    fn aggregate_budget_accepts_exact_and_rejects_one_under() {
        let document = SchematicPlotContractBudget {
            derived_items: 41,
            materialized_bytes: 73,
        };
        let exact = SchematicPlotDocumentsLimits {
            max_total_derived_items: document.derived_items,
            max_total_materialized_bytes: document.materialized_bytes,
            ..SchematicPlotDocumentsLimits::default()
        };
        let mut total = SchematicPlotContractBudget {
            derived_items: 0,
            materialized_bytes: 0,
        };
        charge_plot_batch_budget(&mut total, document, exact).expect("exact batch budget");
        assert_eq!(total, document);

        for limits in [
            SchematicPlotDocumentsLimits {
                max_total_derived_items: document.derived_items - 1,
                ..exact
            },
            SchematicPlotDocumentsLimits {
                max_total_materialized_bytes: document.materialized_bytes - 1,
                ..exact
            },
        ] {
            let mut total = SchematicPlotContractBudget {
                derived_items: 0,
                materialized_bytes: 0,
            };
            assert!(charge_plot_batch_budget(&mut total, document, limits).is_err());
        }
    }

    #[test]
    fn embedded_font_selection_uses_family_style_and_regular_fallback() {
        let files = vec![
            SchematicEmbeddedFile {
                name: "renamed-bold.ttf".to_owned(),
                file_type: "font".to_owned(),
                bytes: KICAD_STROKE_BOLD.to_vec(),
            },
            SchematicEmbeddedFile {
                name: "renamed-regular.ttf".to_owned(),
                file_type: "font".to_owned(),
                bytes: KICAD_STROKE_REGULAR.to_vec(),
            },
        ];
        let indexed = embedded_font_index(&files).expect("bounded embedded font index");
        assert_eq!(indexed.len(), 2);
        let (bold, selected_bold, selected_italic) =
            embedded_font_selection("renamed", true, false, &indexed).expect("bold face");
        assert_eq!(bold.file.name, "renamed-bold.ttf");
        assert!(selected_bold);
        assert!(!selected_italic);

        let regular_files = &files[1..];
        let regular_only =
            embedded_font_index(regular_files).expect("bounded regular embedded font index");
        let (fallback, selected_bold, selected_italic) =
            embedded_font_selection("renamed", true, true, &regular_only)
                .expect("regular fallback");
        assert_eq!(fallback.file.name, "renamed-regular.ttf");
        assert!(!selected_bold);
        assert!(!selected_italic);

        let faces = BTreeSet::from(["renamed".to_owned()]);
        let exact_bytes = KICAD_STROKE_REGULAR.len() * 4;
        let exact_limits = PlotterTextCacheLimits {
            max_font_bytes: exact_bytes,
            ..PlotterTextCacheLimits::default()
        };
        let styles = schematic_font_styles_with_limits(&faces, regular_files, exact_limits)
            .expect("exact reused embedded font budget");
        assert_eq!(styles.len(), 4);
        assert!(
            styles
                .iter()
                .all(|style| matches!(style.bytes, Cow::Borrowed(_)))
        );
        assert!(
            schematic_font_styles_with_limits(
                &faces,
                regular_files,
                PlotterTextCacheLimits {
                    max_font_bytes: exact_bytes - 1,
                    ..exact_limits
                },
            )
            .err()
            .expect("one-under reused embedded font budget")
            .to_string()
            .contains("aggregate limit")
        );
    }
}
