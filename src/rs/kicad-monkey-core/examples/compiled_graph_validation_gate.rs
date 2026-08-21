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
        let bytes_read = read_bounded_line(&mut reader, &mut line, MAX_GRAPH_BYTES)?;
        if bytes_read == 0 {
            break;
        }
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        if line.len() > MAX_GRAPH_BYTES {
            return Err(format!("graph {graph_index} exceeds {MAX_GRAPH_BYTES} bytes").into());
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

fn read_bounded_line(
    reader: &mut impl BufRead,
    output: &mut Vec<u8>,
    max_payload_bytes: usize,
) -> io::Result<usize> {
    let max_buffer_bytes = max_payload_bytes.saturating_add(2);
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(output.len());
        }
        let take = available
            .iter()
            .position(|value| *value == b'\n')
            .map_or(available.len(), |position| position + 1);
        if output.len().saturating_add(take) > max_buffer_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("graph line exceeds {max_payload_bytes} payload bytes"),
            ));
        }
        output.extend_from_slice(&available[..take]);
        reader.consume(take);
        if output.last() == Some(&b'\n') {
            return Ok(output.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::read_bounded_line;
    use std::io::{BufReader, Cursor};

    #[test]
    fn bounded_line_reader_refuses_growth_before_retaining_past_the_ceiling() {
        let mut reader = BufReader::with_capacity(2, Cursor::new(b"123456789\n"));
        let mut output = Vec::new();
        let error = read_bounded_line(&mut reader, &mut output, 4).expect_err("oversize line");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(output.len() <= 6);
    }

    #[test]
    fn bounded_line_reader_accepts_exact_payload_with_crlf() {
        let mut reader = BufReader::with_capacity(2, Cursor::new(b"1234\r\nnext"));
        let mut output = Vec::new();
        assert_eq!(read_bounded_line(&mut reader, &mut output, 4).unwrap(), 6);
        assert_eq!(output, b"1234\r\n");
    }
}
