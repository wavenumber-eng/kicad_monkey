"""Build an external all-family SVG consumer against one exact Git revision."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess


def _run(command: list[str], *, cwd: Path, input_bytes: bytes | None = None, timeout: int = 900) -> subprocess.CompletedProcess[bytes]:
    completed = subprocess.run(command, cwd=cwd, input=input_bytes, capture_output=True, timeout=timeout, check=False)
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed: {' '.join(command)}\nstdout:\n{completed.stdout.decode(errors='replace')}\n"
            f"stderr:\n{completed.stderr.decode(errors='replace')}"
        )
    return completed


def _candidate_repository(repository: Path, destination: Path) -> tuple[Path, str]:
    source = destination / "candidate"
    _run(
        [
            "git", "-c", "core.longpaths=true", "clone", "--quiet", "--local",
            "--no-hardlinks", str(repository), str(source),
        ],
        cwd=destination,
    )
    _run(["git", "config", "core.longpaths", "true"], cwd=source)
    patch = _run(["git", "diff", "--binary", "HEAD"], cwd=repository).stdout
    if patch:
        _run(["git", "apply", "--binary", "--whitespace=nowarn", "-"], cwd=source, input_bytes=patch)
    untracked = _run(["git", "ls-files", "--others", "--exclude-standard", "-z"], cwd=repository).stdout.split(b"\0")
    for encoded in untracked:
        if not encoded:
            continue
        relative = Path(encoded.decode("utf-8"))
        target = source / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(repository / relative, target)
    _run(["git", "config", "user.name", "KiCad Monkey Tests"], cwd=source)
    _run(["git", "config", "user.email", "tests@invalid.example"], cwd=source)
    _run(["git", "add", "--all"], cwd=source)
    _run(["git", "commit", "--quiet", "--allow-empty", "-m", "candidate snapshot"], cwd=source)
    revision = _run(["git", "rev-parse", "HEAD"], cwd=source).stdout.decode().strip()
    return source, revision


def _is_clean(repository: Path) -> bool:
    return not _run(
        ["git", "status", "--porcelain", "--untracked-files=all"], cwd=repository
    ).stdout


CONSUMER = r'''
use std::path::PathBuf;
use kicad_monkey_core::{
    BoardPlotLimits, FootprintPlotLimits, PlotDocumentMetadata, PlotDocumentProjectionLimits,
    SchematicPlotContext, SchematicPlotLimits, SymbolPlotLimits, board_plot_document,
    footprint_plot_document, project_board_plot_document_with_metadata_a0,
    project_footprint_plot_document_a0, project_schematic_plot_document_a0,
    project_symbol_plot_document_a0, schematic_plot_document, symbol_plot_document,
};
use kicad_monkey_svg::{
    SvgColor, SvgContextLimits, SvgFitOptions, SvgIdentityMode, SvgRenderContextA1,
    SvgRenderLimits, SvgSemanticRole, SvgStyleOverride, ViewportPolicy,
    render_board_document_svg, render_footprint_svg, render_schematic_svg, render_symbol_svg,
};

const FOOTPRINT: &str = r#"(footprint "External" (layer "F.Cu")
  (fp_line (start -1 0) (end 1 0) (stroke (width 0.2) (type dash)) (layer "F.SilkS"))
  (fp_text user "gyp" (at 0 1) (layer "F.SilkS")
    (effects (font (size 2 1) italic) (justify mirror right bottom))))"#;
const SYMBOL: &str = r#"(kicad_symbol_lib (version 20241209) (generator kicad_symbol_editor)
  (symbol "External" (symbol "External_0_1"
    (rectangle (start -2 2) (end 2 -2) (stroke (width 0.2) (type solid)) (fill (type none)))
    (text "SYM" (at 0 0 0) (effects (font (size 1.27 1.27)))))))"#;
const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator pcbnew)
  (general (thickness 1.6)) (paper "A4")
  (gr_line (start 0 0) (end 5 5) (stroke (width 0.25) (type solid)) (layer "Edge.Cuts") (uuid "line"))
  (gr_text "PCB" (at 2 1) (layer "F.SilkS") (effects (font (size 1 1)))))"#;
const SCHEMATIC: &str = r#"(kicad_sch (version 20240101) (generator eeschema) (uuid "sch")
  (paper "A4")
  (wire (pts (xy 1 2) (xy 8 2)) (stroke (width 0.2) (type solid)) (uuid "wire"))
  (text "SCH" (exclude_from_sim no) (at 4 3 0) (effects (font (size 1.27 1.27))) (uuid "text")))"#;

fn metadata(id: &str) -> PlotDocumentMetadata {
    PlotDocumentMetadata { document_id: id.to_owned(), source_path: None }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("missing output directory")?);
    std::fs::create_dir_all(&output)?;
    let accent = SvgColor::parse("#2468ACFF")?;
    let mut builder = SvgRenderContextA1::builder().identity_mode(SvgIdentityMode::None);
    for role in [
        SvgSemanticRole::Copper, SvgSemanticRole::Drill, SvgSemanticRole::Mask,
        SvgSemanticRole::Silkscreen, SvgSemanticRole::Fabrication, SvgSemanticRole::Courtyard,
        SvgSemanticRole::BoardEdge, SvgSemanticRole::Worksheet, SvgSemanticRole::SchematicWire,
        SvgSemanticRole::SchematicBus, SvgSemanticRole::Junction, SvgSemanticRole::Label,
        SvgSemanticRole::Pin, SvgSemanticRole::SymbolBody, SvgSemanticRole::HierarchicalSheet,
        SvgSemanticRole::Text, SvgSemanticRole::Image, SvgSemanticRole::Other,
    ] {
        builder = builder.semantic_style(role, SvgStyleOverride::new().with_stroke(accent.clone()).with_fill(accent.clone()));
    }
    let context = builder.build().validate(SvgContextLimits::default())?;
    let viewport = ViewportPolicy::Fit(SvgFitOptions { padding_nm: 0, min_extent_nm: 1, fallback: None });
    let limits = SvgRenderLimits::default();
    let footprint = project_footprint_plot_document_a0(
        footprint_plot_document(FOOTPRINT, FootprintPlotLimits::default())?, metadata("external-footprint"),
        PlotDocumentProjectionLimits::default())?;
    let symbol = project_symbol_plot_document_a0(
        symbol_plot_document(SYMBOL, "External", Some(1), 0, SymbolPlotLimits::default())?, metadata("external-symbol"),
        PlotDocumentProjectionLimits::default())?;
    let board = project_board_plot_document_with_metadata_a0(
        board_plot_document(BOARD, BoardPlotLimits::default())?, metadata("external-board"),
        PlotDocumentProjectionLimits::default())?;
    let schematic_source = schematic_plot_document(SCHEMATIC, SchematicPlotLimits::default(), &SchematicPlotContext::default())?;
    let schematic = project_schematic_plot_document_a0(&schematic_source, PlotDocumentProjectionLimits::default())?;
    let artifacts = [
        ("footprint", render_footprint_svg(&footprint, viewport, &context, limits)?),
        ("symbol", render_symbol_svg(&symbol, viewport, &context, limits)?),
        ("board", render_board_document_svg(&board, viewport, &context, limits)?),
        ("schematic", render_schematic_svg(&schematic, viewport, &context, limits)?),
    ];
    for (family, artifact) in artifacts {
        if !artifact.svg.contains("#2468AC") || artifact.svg.contains("data-ref=") {
            return Err(format!("{family} ignored the external context").into());
        }
        std::fs::write(output.join(format!("{family}.svg")), artifact.svg)?;
    }
    Ok(())
}
'''


def run(repository: Path, output: Path) -> dict[str, object]:
    output.mkdir(parents=True, exist_ok=True)
    repository = repository.resolve()
    if _is_clean(repository):
        candidate = repository
        revision = _run(["git", "rev-parse", "HEAD"], cwd=repository).stdout.decode().strip()
        synthetic = False
    else:
        candidate, revision = _candidate_repository(repository, output)
        synthetic = True
    consumer = output / "consumer"
    (consumer / "src").mkdir(parents=True)
    git_url = candidate.resolve().as_uri()
    (consumer / "Cargo.toml").write_text(
        "\n".join([
            "[package]", 'name = "kicad-monkey-svg-external-consumer"', 'version = "0.0.0"', 'edition = "2024"', "",
            "[dependencies]",
            f'kicad-monkey-core = {{ git = "{git_url}", rev = "{revision}" }}',
            f'kicad-monkey-svg = {{ git = "{git_url}", rev = "{revision}" }}', "",
        ]), encoding="utf-8")
    (consumer / "src" / "main.rs").write_text(CONSUMER, encoding="utf-8")
    _run(["cargo", "generate-lockfile"], cwd=consumer)
    tree = _run(["cargo", "tree", "--locked", "--edges", "normal"], cwd=consumer).stdout.decode()
    forbidden = ["kicad-monkey-wasm", "kicad-monkey-native", "kicad-cruncher"]
    present = [name for name in forbidden if name in tree]
    if present:
        raise RuntimeError(f"forbidden runtime dependencies: {present}\n{tree}")
    artifacts = output / "artifacts"
    _run(["cargo", "run", "--locked", "--", str(artifacts)], cwd=consumer)
    lock = (consumer / "Cargo.lock").read_text(encoding="utf-8")
    if revision not in lock:
        raise RuntimeError("Cargo.lock does not retain the exact candidate revision")
    result = {
        "revision": revision, "git_url": git_url, "synthetic": synthetic,
        "artifacts": {family: str(artifacts / f"{family}.svg") for family in ("footprint", "symbol", "board", "schematic")},
        "runtime_tree": tree,
    }
    (output / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(run(args.repository, args.output)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
