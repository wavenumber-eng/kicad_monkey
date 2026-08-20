use crate::{
    CompiledSchematicGraphLimits, KiCadNetlist, KiCadNetlistJsonMetadata, KiCadNetlistLimits,
    PcbView, ProjectDocument, ProjectLimits, ProjectView, SchematicBundleIndex, SourceBundle,
    SourceBundleError, SourceBundleErrorKind, build_compiled_schematic_graph, build_kicad_netlist,
    build_kicad_netlist_json,
};
use kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};

mod components;
mod hierarchy;
mod indexes;
mod pnp;
mod resource;
mod schematic_instances;
mod variants;

pub use schematic_instances::KiCadSchematicInstance;

pub const KICAD_DESIGN_JSON_SCHEMA: &str = "kicad_monkey.design.a0";
pub const KICAD_DESIGN_JSON_GENERATOR: &str = "kicad_monkey";

#[derive(Debug)]
pub struct KiCadDesignFacts<'index> {
    index: &'index SchematicBundleIndex,
    project: Option<ProjectDocument>,
    graph: CompiledSchematicGraphA0,
    netlist: KiCadNetlist,
}

impl KiCadDesignFacts<'_> {
    pub fn project(&self) -> Option<ProjectView<'_>> {
        self.project.as_ref().map(ProjectDocument::view)
    }

    pub fn graph(&self) -> &CompiledSchematicGraphA0 {
        &self.graph
    }

    pub fn netlist(&self) -> &KiCadNetlist {
        &self.netlist
    }

    pub fn into_parts(self) -> (CompiledSchematicGraphA0, KiCadNetlist) {
        (self.graph, self.netlist)
    }

    pub fn schematic_instances(&self) -> Result<Vec<KiCadSchematicInstance>, KiCadDesignJsonError> {
        schematic_instances::schematic_instances(self)
    }
}

pub fn build_kicad_design_facts<'index>(
    index: &'index SchematicBundleIndex,
    bundle: &SourceBundle,
    project_limits: ProjectLimits,
    graph_limits: CompiledSchematicGraphLimits,
    netlist_limits: KiCadNetlistLimits,
) -> Result<KiCadDesignFacts<'index>, SourceBundleError> {
    if !index.project_belongs_to(bundle) {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Contract,
            None,
            "project source does not belong to the schematic index",
        ));
    }
    let project = bundle
        .project()
        .map(|source| ProjectDocument::from_reader(source.bytes(), project_limits))
        .transpose()
        .map_err(|error| {
            SourceBundleError::new(
                SourceBundleErrorKind::Project,
                bundle.project_path(),
                format!("project document is invalid: {error}"),
            )
        })?;
    let project_view = project.as_ref().map(ProjectDocument::view);
    let graph = build_compiled_schematic_graph(index, graph_limits)?;
    let netlist = build_kicad_netlist(index, project_view, netlist_limits)?;
    Ok(KiCadDesignFacts {
        index,
        project,
        graph,
        netlist,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KiCadDesignJsonLimits {
    pub max_derived_items: usize,
    pub max_materialized_bytes: usize,
    pub max_output_bytes: usize,
}

impl Default for KiCadDesignJsonLimits {
    fn default() -> Self {
        Self {
            max_derived_items: 256_000_000,
            max_materialized_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
            max_output_bytes: 2_usize.saturating_mul(1024 * 1024 * 1024),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiCadDesignSourcePath {
    pub filename: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KiCadDesignJsonPaths {
    pub project_name: Option<String>,
    pub project_filename: Option<String>,
    pub project_path: Option<String>,
    pub schematic_paths: BTreeMap<String, KiCadDesignSourcePath>,
}

#[derive(Clone, Copy, Debug)]
pub struct KiCadDesignPcb<'a> {
    pub source_filename: &'a str,
    pub view: &'a PcbView<'a>,
}

#[derive(Debug)]
pub struct KiCadDesignJsonError {
    message: String,
}

impl KiCadDesignJsonError {
    fn context(context: &str, error: impl fmt::Display) -> Self {
        Self {
            message: format!("{context}: {error}"),
        }
    }
}

impl fmt::Display for KiCadDesignJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for KiCadDesignJsonError {}

impl From<SourceBundleError> for KiCadDesignJsonError {
    fn from(error: SourceBundleError) -> Self {
        Self::context("could not resolve design variant", error)
    }
}

pub fn build_kicad_design_json(
    index: &SchematicBundleIndex,
    facts: &KiCadDesignFacts<'_>,
    paths: &KiCadDesignJsonPaths,
    pcb: Option<KiCadDesignPcb<'_>>,
    include_indexes: bool,
) -> Result<Value, KiCadDesignJsonError> {
    build_kicad_design_json_with_limits(
        index,
        facts,
        paths,
        pcb,
        include_indexes,
        KiCadDesignJsonLimits::default(),
    )
}

pub fn build_kicad_design_json_with_limits(
    index: &SchematicBundleIndex,
    facts: &KiCadDesignFacts<'_>,
    paths: &KiCadDesignJsonPaths,
    pcb: Option<KiCadDesignPcb<'_>>,
    include_indexes: bool,
    limits: KiCadDesignJsonLimits,
) -> Result<Value, KiCadDesignJsonError> {
    validate_facts_binding(index, facts)?;
    let project = facts.project();
    let graph = facts.graph();
    let netlist = facts.netlist();
    resource::preflight_design_json(
        index,
        project,
        netlist,
        graph,
        paths,
        pcb,
        include_indexes,
        limits,
    )?;
    let netlist_json = build_kicad_netlist_json(netlist, KiCadNetlistJsonMetadata::default());
    let mut result = serde_json::Map::new();
    result.insert("schema".to_owned(), json!(KICAD_DESIGN_JSON_SCHEMA));
    result.insert("generator".to_owned(), json!(KICAD_DESIGN_JSON_GENERATOR));
    result.insert("project".to_owned(), project_json(project, paths)?);
    result.insert(
        "variants".to_owned(),
        variants::variants_json(index, project)?,
    );
    result.insert("options".to_owned(), options_json(project));
    result.insert(
        "sheets".to_owned(),
        hierarchy::sheets_json(index, netlist, paths),
    );
    result.insert(
        "components".to_owned(),
        components::components_json(netlist, &netlist_json),
    );
    result.insert(
        "schematic_hierarchy".to_owned(),
        hierarchy::schematic_hierarchy_json(index, paths),
    );
    result.insert("nets".to_owned(), netlist_json["nets"].clone());
    result.insert(
        "compiled_schematic_graph".to_owned(),
        serde_json::to_value(graph)
            .map_err(|error| KiCadDesignJsonError::context("could not encode graph", error))?,
    );
    if let Some(pcb) = pcb
        && let Some(payload) = pnp::pnp_json(pcb, netlist, &netlist_json)?
    {
        result.insert("pnp".to_owned(), payload);
    }
    if !netlist.net_classes.is_empty() {
        result.insert(
            "net_classes".to_owned(),
            netlist_json["net_classes"].clone(),
        );
        result.insert(
            "net_name_to_classes".to_owned(),
            indexes::net_name_to_classes(netlist),
        );
    }
    if include_indexes {
        result.insert("indexes".to_owned(), indexes::indexes_json(netlist));
    }
    let result = Value::Object(result);
    enforce_output_limit(&result, limits.max_output_bytes)?;
    Ok(result)
}

fn validate_facts_binding(
    index: &SchematicBundleIndex,
    bound: &KiCadDesignFacts<'_>,
) -> Result<(), KiCadDesignJsonError> {
    if !std::ptr::eq(index, bound.index) {
        return Err(KiCadDesignJsonError::context(
            "compiled schematic graph does not belong to the supplied bundle",
            "graph was built from a different schematic index",
        ));
    }
    Ok(())
}

fn enforce_output_limit(
    value: &Value,
    max_output_bytes: usize,
) -> Result<(), KiCadDesignJsonError> {
    let mut writer = LimitedWriter::new(max_output_bytes);
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        KiCadDesignJsonError::context("KiCad design JSON output limit exceeded", error)
    })
}

struct LimitedWriter {
    written: usize,
    limit: usize,
}

impl LimitedWriter {
    const fn new(limit: usize) -> Self {
        Self { written: 0, limit }
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(buffer.len())
            .filter(|next| *next <= self.limit)
            .ok_or_else(|| io::Error::other(format!("output exceeds {} bytes", self.limit)))?;
        self.written = next;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn project_json(
    project: Option<ProjectView<'_>>,
    paths: &KiCadDesignJsonPaths,
) -> Result<Value, KiCadDesignJsonError> {
    let variables = project
        .map(|view| view.text_variables())
        .transpose()
        .map_err(|error| KiCadDesignJsonError::context("could not read text variables", error))?
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    Ok(json!({
        "name": paths.project_name,
        "filename": paths.project_filename,
        "path": paths.project_path,
        "text_variables": variables,
    }))
}

fn options_json(project: Option<ProjectView<'_>>) -> Value {
    let first_id = project.and_then(|view| view.get_path("schematic.subpart_first_id"));
    let separator = project.and_then(|view| view.get_path("schematic.subpart_id_separator"));
    json!({
        "net_identifier_scope": "KICAD_PROJECT",
        "allow_ports_to_name_nets": true,
        "allow_sheet_entries_to_name_nets": true,
        "allow_single_pin_nets": true,
        "append_sheet_numbers_to_local_nets": false,
        "power_port_names_take_priority": true,
        "higher_level_names_take_priority": true,
        "auto_sheet_numbering": true,
        "kicad_schematic_format": "sexpr",
        "kicad_supported_oracle_versions": ["9", "10"],
        "kicad_subpart_first_id": first_id,
        "kicad_subpart_id_separator": separator,
    })
}

#[cfg(test)]
mod tests {
    use super::{LimitedWriter, enforce_output_limit};
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn output_limit_accepts_exact_and_rejects_one_under() {
        let value = json!({"items": [1, 2, 3]});
        let encoded = serde_json::to_vec(&value).unwrap();
        enforce_output_limit(&value, encoded.len()).expect("exact output limit");
        let error = enforce_output_limit(&value, encoded.len() - 1).expect_err("one byte over");
        assert!(error.to_string().contains("output exceeds"));
    }

    #[test]
    fn limited_writer_rejects_counter_overflow() {
        let mut writer = LimitedWriter {
            written: usize::MAX,
            limit: usize::MAX,
        };
        assert!(writer.write(&[0]).is_err());
    }
}
