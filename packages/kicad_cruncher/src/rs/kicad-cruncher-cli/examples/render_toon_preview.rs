use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use kicad_cruncher_cli::pcb_svg::geometer::NativeGeometer;
use kicad_cruncher_cli::pcb_svg::preview::render_top_toon_preview;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(
        args.next()
            .ok_or("usage: render_toon_preview <board.kicad_pcb> <output.svg>")?,
    );
    let output = PathBuf::from(
        args.next()
            .ok_or("usage: render_toon_preview <board.kicad_pcb> <output.svg>")?,
    );
    if args.next().is_some() {
        return Err("usage: render_toon_preview <board.kicad_pcb> <output.svg>".into());
    }

    let source = fs::read_to_string(&input)?;
    let geometer = NativeGeometer::connect_discovered()?;
    let preview = render_top_toon_preview(&source, input.display().to_string(), &geometer)?;
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, preview.svg)?;
    geometer.close()?;

    println!(
        "Rendered {} embedded model instance(s) with {} Geometer request(s) and {} cache hit(s) to {} ({} warning(s))",
        preview.rendered_model_count,
        preview.geometer_request_count,
        preview.model_cache_hit_count,
        output.display(),
        preview.warnings.len()
    );
    for warning in preview.warnings {
        eprintln!("warning: {warning}");
    }
    Ok(())
}
