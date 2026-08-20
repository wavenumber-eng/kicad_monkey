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
    KiCadNetlist, KiCadNetlistLimits, ProjectDocument, ProjectLimits, SchematicBundleIndex,
    SchematicBundleLimits, SchematicDocument, SchematicDocumentLimits, SourceBundle,
    SourceBundleLimits, build_compiled_schematic_graph, build_kicad_netlist, emit_kicad_netlist,
    validate_compiled_schematic_graph,
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
}

#[derive(Debug)]
pub struct StructuredDesignFacts {
    pub compiled_schematic_graph: CompiledSchematicGraphA0,
    pub netlist: KiCadNetlist,
    pub kicad_netlist: String,
}

struct DesignPaths {
    input: PathBuf,
    root: PathBuf,
    project: Option<PathBuf>,
    root_schematic: PathBuf,
}

fn resolve_design_paths(input: &Path) -> Result<DesignPaths, DesignError> {
    let input_path = input
        .canonicalize()
        .map_err(|error| DesignError::context("could not resolve design input", error))?;
    if !input_path.is_file() {
        return Err(DesignError::new("design input is not a regular file"));
    }

    let suffix = input_path.extension().and_then(|value| value.to_str());
    let (project_path, root_schematic_path) = match suffix {
        Some("kicad_pro") => {
            let schematic = input_path.with_extension("kicad_sch");
            if !schematic.is_file() {
                return Err(DesignError::new(format!(
                    "project root schematic was not found: {}",
                    schematic.display()
                )));
            }
            (Some(input_path.clone()), schematic)
        }
        Some("kicad_sch") => (None, input_path.clone()),
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
    })
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
    Ok(LoadedDesignSources {
        input_path: paths.input,
        bundle_root: paths.root,
        bundle,
    })
}

pub fn build_structured_design_facts(
    loaded: &LoadedDesignSources,
) -> Result<StructuredDesignFacts, DesignError> {
    let index = SchematicBundleIndex::build(&loaded.bundle, SchematicBundleLimits::default())
        .map_err(|error| DesignError::context("could not index schematic hierarchy", error))?;
    let project = loaded
        .bundle
        .project()
        .map(|source| ProjectDocument::from_reader(source.bytes(), ProjectLimits::default()))
        .transpose()
        .map_err(|error| DesignError::context("could not parse project", error))?;
    let compiled_schematic_graph = build_compiled_schematic_graph(&index, Default::default())
        .map_err(|error| DesignError::context("could not compile schematic graph", error))?;
    validate_compiled_schematic_graph(&compiled_schematic_graph)
        .map_err(|error| DesignError::context("compiled schematic graph is invalid", error))?;
    let netlist = build_kicad_netlist(
        &index,
        project.as_ref().map(ProjectDocument::view),
        KiCadNetlistLimits::default(),
    )
    .map_err(|error| DesignError::context("could not build KiCad netlist", error))?;
    let kicad_netlist = emit_kicad_netlist(
        &netlist,
        loaded.bundle.root_schematic_path(),
        "",
        GRAPH_TOOL,
        KiCadNetlistLimits::default().max_output_bytes,
    )
    .map_err(|error| DesignError::context("could not emit KiCad netlist", error))?;
    Ok(StructuredDesignFacts {
        compiled_schematic_graph,
        netlist,
        kicad_netlist,
    })
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
