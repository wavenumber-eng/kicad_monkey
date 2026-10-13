//! Write the complete materialization of a board as a pack.
//!
//! Usage: `cargo run --release -p kicad-monkey-core --example
//! materialization_pack -- BOARD.kicad_pcb OUT.kmpack [TILE_MM]`. A
//! `.kicad_pro` next to the board supplies its net classes.

use kicad_monkey_core::board_materialization::materialize_project;
use kicad_monkey_materialization_pack::encode_pack;
use serde_json::json;

fn fail(message: &str) -> ! {
    eprintln!("materialization_pack: {message}");
    std::process::exit(1);
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (Some(board_path), Some(out)) = (arguments.first(), arguments.get(1)) else {
        fail("usage: materialization_pack BOARD.kicad_pcb OUT.kmpack [TILE_MM]");
    };
    let tile_mm: f64 = arguments
        .get(2)
        .and_then(|text| text.parse().ok())
        .unwrap_or(10.0);
    let board =
        std::fs::read_to_string(board_path).unwrap_or_else(|error| fail(&error.to_string()));
    let project =
        std::fs::read_to_string(std::path::Path::new(board_path).with_extension("kicad_pro")).ok();
    let bytes = board.len() + project.as_ref().map_or(0, String::len);
    let request = serde_json::from_value(json!({
        "type": "kicad_monkey.materialization.request",
        "version": "a0",
        "products": ["components", "copper", "dielectric", "holes", "paste", "silkscreen",
                     "solder_mask"],
        "tile_size_nm": (tile_mm * 1e6).round() as i64,
        "max_source_bytes": bytes.to_string(),
        "max_operations": 50_000_000,
        "max_points": 500_000_000
    }))
    .unwrap_or_else(|error| fail(&error.to_string()));
    let document = materialize_project(&board, project.as_deref(), &request)
        .unwrap_or_else(|error| fail(&error.to_string()));
    let pack = encode_pack(&document).unwrap_or_else(|error| fail(&error.to_string()));
    std::fs::write(out, &pack).unwrap_or_else(|error| fail(&error.to_string()));
    println!(
        "{{\"operations\":{},\"chunks\":{},\"pack_bytes\":{}}}",
        document.operations.len(),
        document.chunks.len(),
        pack.len()
    );
}
