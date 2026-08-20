//! Direct Rust assembly of the nonvisual design-review foundations.

use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
use kicad_monkey_contracts::generated::source_bundle_manifest::{
    CanonicalUint64Decimal, SourceBundleManifestA0, SourceBundleSource, SourceKind, SourceSlot,
};
use kicad_monkey_core::{
    KiCadDesignJsonPaths, KiCadDesignPcb, KiCadDesignSourcePath, KiCadNetlist,
    KiCadNetlistJsonMetadata, KiCadNetlistLimits, PcbLimits, PcbView, ProjectLimits,
    SchematicBundleIndex, SchematicBundleLimits, SchematicDocument, SchematicDocumentLimits,
    SourceBundle, SourceBundleLimits, build_kicad_design_facts, build_kicad_design_json,
    build_kicad_netlist_json, emit_kicad_netlist, validate_compiled_schematic_graph,
};

const GRAPH_TOOL: &str = "kicad_cruncher";

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
    let (compiled_schematic_graph, netlist) = design_facts.into_parts();
    Ok(StructuredDesignFacts {
        compiled_schematic_graph,
        netlist,
        netlist_json,
        design_json,
        kicad_netlist,
    })
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
