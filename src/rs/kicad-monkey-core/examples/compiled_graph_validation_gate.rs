use kicad_monkey_contracts::decode_compiled_schematic_graph_a0;
use kicad_monkey_core::validate_compiled_schematic_graph;
use std::error::Error;
use std::io::{self, BufRead};

const MAX_GRAPH_BYTES: usize = 512 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let input = io::stdin();
    let mut reader = input.lock();
    let mut line = Vec::new();
    let mut graph_index = 0_usize;
    loop {
        line.clear();
        let bytes_read = reader.read_until(b'\n', &mut line)?;
        if bytes_read == 0 {
            break;
        }
        if line.len() > MAX_GRAPH_BYTES {
            return Err(format!("graph {graph_index} exceeds {MAX_GRAPH_BYTES} bytes").into());
        }
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        if line.is_empty() {
            continue;
        }
        let graph = decode_compiled_schematic_graph_a0(&line)
            .map_err(|error| format!("graph {graph_index} transport: {error}"))?;
        validate_compiled_schematic_graph(&graph)
            .map_err(|error| format!("graph {graph_index} semantics: {error}"))?;
        println!("graph {graph_index} rows {}", row_count(&graph));
        graph_index += 1;
    }
    if graph_index == 0 {
        return Err("no compiled graphs supplied".into());
    }
    Ok(())
}

fn row_count(
    graph: &kicad_monkey_contracts::generated::compiled_schematic_graph::CompiledSchematicGraphA0,
) -> usize {
    graph.unit_definitions.len()
        + graph.page_definitions.len()
        + graph.unit_occurrences.len()
        + graph.page_occurrences.len()
        + graph.hierarchy_occurrences.len()
        + graph.component_occurrences.len()
        + graph.local_net_occurrences.len()
        + graph.terminal_occurrences.len()
        + graph.hierarchy_terminal_bindings.len()
        + graph.graphical_artifact_links.len()
}
