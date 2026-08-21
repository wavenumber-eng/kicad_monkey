"""
Generate KiCad CLI reference SVGs for board SVG testing.

This script uses kicad-cli to generate black-and-white SVG files
for each board, one file per layer. These serve as reference
outputs for testing our Python to_svg() implementation.

Usage:
    uv run python tools/kicad/tests/generate_board_svg_references.py
"""

import argparse
import logging
import shutil
import subprocess
from pathlib import Path

from kicad_cli_resolver import resolve_kicad_cli

log = logging.getLogger(__name__)

# Standard layers to export for boards
# Include Edge.Cuts as common layer on all exports for reference
BOARD_LAYERS = [
    "F.Cu",
    "B.Cu",
    "In1.Cu",
    "In2.Cu",
    "F.SilkS",
    "B.SilkS",
    "F.Fab",
    "B.Fab",
    "F.Mask",
    "B.Mask",
    "F.Paste",
    "B.Paste",
    "F.CrtYd",
    "B.CrtYd",
    "Edge.Cuts",
    "User.Drawings",
    "User.Comments",
]

# All visible layers combined (for "All Layers" view)
ALL_LAYERS_COMBINED = "F.Cu,B.Cu,F.SilkS,B.SilkS,F.Fab,B.Fab,F.Mask,B.Mask,F.CrtYd,B.CrtYd,Edge.Cuts,User.Drawings,User.Comments"

# KiCad CLI fallback paths (used only if no staged cache build is found).
# The shared ``resolve_kicad_cli`` resolver always prefers the manifest-listed
# cache build (with IR/recorder emitter patches) over installed system KiCads,
# so references stay consistent with the IR oracle.
KICAD_CLI_PATHS = [
    Path("C:/Program Files/KiCad/10.0/bin/kicad-cli.exe"),
    Path("C:/Program Files/KiCad/9.0/bin/kicad-cli.exe"),
    Path("C:/Program Files/KiCad/8.0/bin/kicad-cli.exe"),
    Path("C:/Program Files/KiCad/7.0/bin/kicad-cli.exe"),
]


def layer_filename_token(layer: str) -> str:
    """Normalize layer names to the on-disk SVG filename token."""
    return layer.replace(".", "_").replace(" ", "_")


def find_kicad_cli() -> Path | None:
    """Find kicad-cli executable, preferring the manifest-listed cache build.

    Resolution order (delegated to ``resolve_kicad_cli``):
    1. ``$KICAD_CLI`` env var override;
    2. manifest-listed staged builds under ``$KICAD_CLI_CACHE_ROOT``
       (this is the canonical IR-oracle build with the recorder patches);
    3. any other staged cache build;
    4. ``PATH``;
    5. installed KiCad 10/9.

    The legacy hardcoded ``KICAD_CLI_PATHS`` list is kept as a final fallback
    only so this script remains usable on machines without a staged cache.
    """
    resolved = resolve_kicad_cli(required_capability="pcb_svg")
    if resolved is not None:
        return resolved

    cli = shutil.which("kicad-cli")
    if cli:
        return Path(cli)

    for path in KICAD_CLI_PATHS:
        if path.exists():
            return path

    return None


def generate_board_reference_svgs(
    input_dir: Path,
    output_dir: Path,
    kicad_cli: Path,
    layers: list[str] | None = None,
    include_edge_cuts: bool = True,
) -> dict[str, list[Path]]:
    """
    Generate reference SVGs for all boards in input_dir.

    Args:
        input_dir: Directory containing board subfolders with .kicad_pcb files
                   Structure: input_dir/board_name/board_name.kicad_pcb
        output_dir: Directory to write reference SVGs (creates matching subfolders)
        kicad_cli: Path to kicad-cli executable
        layers: Layers to export (default: BOARD_LAYERS)
        include_edge_cuts: Include Edge.Cuts as common layer on all exports

    Returns:
        Dict mapping board name to list of generated SVG paths
    """
    if layers is None:
        layers = BOARD_LAYERS

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    # Find all board files in subfolders (new structure: input/board_name/board_name.kicad_pcb)
    pcb_files = sorted(input_dir.glob("*/*.kicad_pcb"))
    log.info(f"Found {len(pcb_files)} board files in {input_dir}")

    results = {}

    for pcb_path in pcb_files:
        pcb_name = pcb_path.stem
        # Get the board folder name (parent directory of .kicad_pcb)
        board_folder = pcb_path.parent.name
        log.info(f"Processing: {board_folder}/{pcb_name}")

        # Create matching subfolder in output directory
        board_output_dir = output_dir / board_folder
        board_output_dir.mkdir(parents=True, exist_ok=True)

        generated = []

        for layer in layers:
            # Output file: {board_folder}/{board_name}__{layer}.svg
            # Replace dots in layer name with underscore for filename
            layer_safe = layer_filename_token(layer)
            svg_path = board_output_dir / f"{pcb_name}__{layer_safe}.svg"

            # Build layer list - always include Edge.Cuts if enabled
            layer_list = layer
            if include_edge_cuts and layer != "Edge.Cuts":
                layer_list = f"{layer},Edge.Cuts"

            try:
                # Run kicad-cli to export SVG
                # Use --mode-single to get a single file output
                # Use --page-size-mode 2 for board area only (no title block)
                # Use --exclude-drawing-sheet to remove drawing sheet
                # Use --drill-shape-opt 2 for actual drill shapes
                result = subprocess.run(
                    [
                        str(kicad_cli),
                        "pcb", "export", "svg",
                        "--black-and-white",
                        "--layers", layer_list,
                        "--mode-single",
                        "--page-size-mode", "2",
                        "--exclude-drawing-sheet",
                        "--drill-shape-opt", "2",
                        "--output", str(svg_path),
                        str(pcb_path),
                    ],
                    capture_output=True,
                    text=True,
                    timeout=60,
                )

                if result.returncode != 0:
                    log.warning(f"  {layer}: kicad-cli error: {result.stderr.strip()}")
                    continue

                if svg_path.exists():
                    # Check if file has content (not empty SVG)
                    content = svg_path.read_text()
                    if "<path" in content or "<polygon" in content or "<circle" in content or "<rect" in content or "<line" in content:
                        generated.append(svg_path)
                        log.debug(f"  {layer}: generated {svg_path.name}")
                    else:
                        # Remove empty SVG
                        svg_path.unlink()
                        log.debug(f"  {layer}: empty (no graphics on this layer)")
                else:
                    log.debug(f"  {layer}: no output file generated")

            except subprocess.TimeoutExpired:
                log.error(f"  {layer}: timeout")
            except Exception as e:
                log.error(f"  {layer}: error: {e}")

        # Also generate "All Layers" combined view
        all_svg_path = board_output_dir / f"{pcb_name}__All_Layers.svg"
        try:
            result = subprocess.run(
                [
                    str(kicad_cli),
                    "pcb", "export", "svg",
                    "--black-and-white",
                    "--layers", ALL_LAYERS_COMBINED,
                    "--mode-single",
                    "--page-size-mode", "2",
                    "--exclude-drawing-sheet",
                    "--drill-shape-opt", "2",
                    "--output", str(all_svg_path),
                    str(pcb_path),
                ],
                capture_output=True,
                text=True,
                timeout=60,
            )
            if result.returncode == 0 and all_svg_path.exists():
                generated.append(all_svg_path)
                log.debug(f"  All Layers: generated {all_svg_path.name}")
        except Exception as e:
            log.warning(f"  All Layers: error: {e}")

        if generated:
            results[board_folder] = generated
            log.info(f"  Generated {len(generated)} SVG files")
        else:
            log.info("  No layers with graphics")

    return results


def _export_board_layer(
    *,
    kicad_cli: Path,
    pcb_path: Path,
    svg_path: Path,
    layer_list: str,
) -> bool:
    """Invoke kicad-cli for one (board, layer-list) combo. Returns True
    when the resulting SVG has at least one stroke / fill element."""
    try:
        result = subprocess.run(
            [
                str(kicad_cli),
                "pcb", "export", "svg",
                "--black-and-white",
                "--layers", layer_list,
                "--mode-single",
                "--page-size-mode", "2",
                "--exclude-drawing-sheet",
                "--drill-shape-opt", "2",
                "--output", str(svg_path),
                str(pcb_path),
            ],
            capture_output=True,
            text=True,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        log.error(f"  {layer_list}: timeout")
        return False
    except Exception as exc:
        log.error(f"  {layer_list}: error: {exc}")
        return False
    if result.returncode != 0:
        log.warning(f"  {layer_list}: kicad-cli error: {result.stderr.strip()}")
        return False
    if not svg_path.exists():
        return False
    content = svg_path.read_text()
    if not any(tag in content for tag in ("<path", "<polygon", "<circle", "<rect", "<line")):
        svg_path.unlink()
        return False
    return True


def _regenerate_pcb_foundation_references(
    pcb_foundation_dir: Path,
    kicad_cli: Path,
) -> dict[str, list[Path]]:
    """Walk ``pcb_foundation/<case>/input/*.kicad_pcb`` and rewrite each
    case's adjacent ``reference_output/`` SVG set with one SVG per layer
    plus the combined ``All_Layers`` view.

    Per-case layout (post Phase 1 rename, 2026-05-17):

        pcb_foundation/<case>/input/<board>.kicad_pcb
        pcb_foundation/<case>/reference_output/<board>__<Layer>.svg
    """
    results: dict[str, list[Path]] = {}
    for pcb_path in sorted(pcb_foundation_dir.glob("*/input/*.kicad_pcb")):
        case_dir = pcb_path.parent.parent
        reference_dir = case_dir / "reference_output"
        reference_dir.mkdir(parents=True, exist_ok=True)
        board_name = pcb_path.stem
        log.info(f"Processing: {case_dir.name}/{board_name}")

        generated: list[Path] = []
        for layer in BOARD_LAYERS:
            layer_list = layer if layer == "Edge.Cuts" else f"{layer},Edge.Cuts"
            svg_path = reference_dir / f"{board_name}__{layer_filename_token(layer)}.svg"
            if _export_board_layer(
                kicad_cli=kicad_cli,
                pcb_path=pcb_path,
                svg_path=svg_path,
                layer_list=layer_list,
            ):
                generated.append(svg_path)

        # Combined "All Layers" view.
        all_svg_path = reference_dir / f"{board_name}__All_Layers.svg"
        if _export_board_layer(
            kicad_cli=kicad_cli,
            pcb_path=pcb_path,
            svg_path=all_svg_path,
            layer_list=ALL_LAYERS_COMBINED,
        ):
            generated.append(all_svg_path)

        results[case_dir.name] = generated
        log.info(f"  Generated {len(generated)} SVG files")
    return results


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--corpus-root",
        type=Path,
        required=True,
        help="Writable fixture-authoring root containing kicad/.",
    )
    args = parser.parse_args()
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(message)s",
    )

    # Find kicad-cli
    kicad_cli = find_kicad_cli()
    if not kicad_cli:
        log.error("kicad-cli not found. Please install KiCad.")
        return 1

    log.info(f"Using kicad-cli: {kicad_cli}")

    # Walk the synthetic PCB foundation corpus (per-case
    # input/reference_output layout, migrated 2026-05-17 from
    # the legacy board_svg/ topic dir).
    pcb_foundation_dir = args.corpus_root / "kicad" / "pcb_foundation"
    if not pcb_foundation_dir.is_dir():
        parser.error(f"PCB foundation authoring tree not found: {pcb_foundation_dir}")
    results = _regenerate_pcb_foundation_references(pcb_foundation_dir, kicad_cli)

    total_svgs = sum(len(svgs) for svgs in results.values())
    log.info(f"\nGenerated {total_svgs} reference SVGs across {len(results)} cases")
    log.info(f"Foundation root: {pcb_foundation_dir}")

    return 0


if __name__ == "__main__":
    exit(main())
