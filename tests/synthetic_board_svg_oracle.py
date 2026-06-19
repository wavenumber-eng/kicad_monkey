"""Shared synthetic-board SVG oracle helpers for KiCad CLI parity checks."""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, Tuple, cast

from kicad_monkey.testing.corpus import get_kicad_pcb_foundation_dir

from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from svg.canonical_svg import semantic_metrics as canonical_semantic_metrics


@dataclass(frozen=True)
class SyntheticOracleCase:
    """Synthetic case configuration for semantic oracle checks."""

    case_id: str
    board_relpath: str
    layers: Tuple[str, ...]
    metrics: Tuple[str, ...]
    minimums: Tuple[Tuple[str, int], ...]


SYNTHETIC_ORACLE_CASES: Tuple[SyntheticOracleCase, ...] = (
    SyntheticOracleCase(
        case_id="via_copper_drill",
        board_relpath="case019__via_basic/one_via.kicad_pcb",
        layers=("F.Cu",),
        metrics=("viewbox", "white_drill_circles", "total_circles"),
        minimums=(("white_drill_circles", 1),),
    ),
    SyntheticOracleCase(
        case_id="via_edgecuts_drill_outline",
        board_relpath="case019__via_basic/one_via.kicad_pcb",
        layers=("Edge.Cuts",),
        metrics=("viewbox",),
        minimums=(),
    ),
    SyntheticOracleCase(
        case_id="slot_copper_drill_fill",
        board_relpath="case084__pad_slot_hole/one_slot_drill.kicad_pcb",
        layers=("F.Cu",),
        metrics=("viewbox", "white_stroke_paths"),
        minimums=(("white_stroke_paths", 1),),
    ),
    SyntheticOracleCase(
        case_id="slot_edgecuts_drill_outline",
        board_relpath="case084__pad_slot_hole/one_slot_drill.kicad_pcb",
        layers=("Edge.Cuts",),
        metrics=("viewbox", "stroke_paths_1p0000"),
        minimums=(("stroke_paths_1p0000", 1),),
    ),
    SyntheticOracleCase(
        case_id="zone_fill_top_copper",
        board_relpath="case024__fill_top_zone/one_zone_filled_top.kicad_pcb",
        layers=("F.Cu",),
        metrics=("viewbox", "total_strokes", "total_circles"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="outline_only_edgecuts",
        board_relpath="case037__outline_rect/board_outline.kicad_pcb",
        layers=("Edge.Cuts",),
        metrics=("viewbox",),
        minimums=(),
    ),
    # Phase C lead-in: knockout text on silkscreen — one ``gr_text``
    # with ``knockout`` modifier on F.SilkS. Validates that the IR
    # renderer's text-knockout geometry matches kicad-cli structurally.
    SyntheticOracleCase(
        case_id="knockout_text_silk",
        board_relpath="case200__text_knockout_basic/simple_test_knockout.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox",),
        minimums=(),
    ),
    # Phase C: footprint-local ``fp_text`` knockout on silkscreen.
    # ``component_designator_top.kicad_pcb`` contains an ``fp_text user
    # "+"`` with ``knockout`` modifier on F.SilkS using an Arial Bold
    # face with a real ``render_cache`` polygon. Validates that the IR
    # renderer applies the same fill-rule-evenodd compound polygon
    # treatment to footprint-local text that it does for board-level
    # ``gr_text``.
    SyntheticOracleCase(
        case_id="knockout_fp_text_silk",
        board_relpath="case066__comp_smd_top_designator/component_designator_top.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox",),
        minimums=(),
    ),
    # Phase C dimensions: synthetic per-type dimension fixtures. These cover
    # center, aligned, orthogonal, leader, and radial dimension geometry plus
    # tessellated stroke-font value text on Cmts.User.
    SyntheticOracleCase(
        case_id="dim_center",
        board_relpath="case221__dim_center/dim_center.kicad_pcb",
        layers=("Cmts.User",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 2),),
    ),
    SyntheticOracleCase(
        case_id="dim_aligned_horizontal",
        board_relpath="case220__dim_aligned_horizontal/dim_aligned_horizontal.kicad_pcb",
        layers=("Cmts.User",),
        metrics=("viewbox",),
        minimums=(),
    ),
    SyntheticOracleCase(
        case_id="dim_orthogonal_horizontal",
        board_relpath="case224__dim_orthogonal_horizontal/dim_orthogonal_horizontal.kicad_pcb",
        layers=("Cmts.User",),
        metrics=("viewbox",),
        minimums=(),
    ),
    SyntheticOracleCase(
        case_id="dim_orthogonal_vertical",
        board_relpath="case225__dim_orthogonal_vertical/dim_orthogonal_vertical.kicad_pcb",
        layers=("Cmts.User",),
        metrics=("viewbox",),
        minimums=(),
    ),
    SyntheticOracleCase(
        case_id="dim_leader_plain",
        board_relpath="case223__dim_leader_plain/dim_leader_plain.kicad_pcb",
        layers=("Cmts.User",),
        metrics=("viewbox",),
        minimums=(),
    ),
    SyntheticOracleCase(
        case_id="dim_leader_frame_rect",
        board_relpath="case222__dim_leader_frame_rect/dim_leader_frame_rect.kicad_pcb",
        layers=("Cmts.User",),
        metrics=("viewbox",),
        minimums=(),
    ),
    SyntheticOracleCase(
        case_id="dim_radial",
        board_relpath="case226__dim_radial/dim_radial.kicad_pcb",
        layers=("Cmts.User",),
        metrics=("viewbox",),
        minimums=(),
    ),
    # Phase C stroke-style decomposition: dashed/dotted/dash-dot line and
    # arc gr_* primitives. kicad-cli decomposes these into per-dash
    # ``ThickSegment`` calls via ``STROKE_PARAMS::Stroke`` rather than
    # using CSS ``stroke-dasharray``. The IR converter now mirrors that
    # decomposition in :mod:`kicad_stroke_decompose`. Each dash becomes
    # one IR ``thick_segment`` op → one SVG element, so ``total_strokes``
    # parity verifies the algorithm.
    SyntheticOracleCase(
        case_id="line_silk_dashed",
        board_relpath="case243__line_silk_dashed/silk_line_top_dashed.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="line_silk_dotted",
        board_relpath="case244__line_silk_dotted/silk_line_top_dotted.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="line_silk_dash_dot",
        board_relpath="case245__line_silk_dash_dot/silk_line_top_dash_dot.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="line_silk_dash_dot_dot",
        board_relpath="case246__line_silk_dash_dot_dot/silk_line_top_dash_dot_dot.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="arc_silk_dash_dot",
        board_relpath="case232__arc_silk_dash_dot/silk_arc_top_dash_dot.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="arc_silk_dash_dot_dot",
        board_relpath="case233__arc_silk_dash_dot_dot/silk_arc_top_dash_dot_dot.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 1),),
    ),
    # Pad-shape parity ratchet (2026-05-18). These three cases were
    # previously suspected of IR pad-shape divergence but actually pass
    # IR-vs-CLI metric parity (verified 2026-05-18 across F.Cu). Pinning
    # them here so any regression in the pad emitters trips immediately.
    SyntheticOracleCase(
        case_id="pad_smd_oval_top_copper",
        board_relpath="case013__pad_smd_oval/case013__pad_smd_oval.kicad_pcb",
        layers=("F.Cu",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="pad_th_oval_top_copper",
        board_relpath="case018__pad_th_oval/case018__pad_th_oval.kicad_pcb",
        layers=("F.Cu",),
        metrics=("viewbox", "total_strokes", "total_circles"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="pad_chamfered_roundrect_top_copper",
        board_relpath="case083__pad_chamfered_roundrect/one_chamfer_roundrect.kicad_pcb",
        layers=("F.Cu",),
        metrics=("viewbox", "total_strokes", "filled_black_ink_area"),
        minimums=(("total_strokes", 1),),
    ),
    # Font parity ratchet (2026-06-10). Despite their directory names,
    # case026-028 carry an Arial ``(face ...)`` + embedded render_cache,
    # so they pin TTF glyph-polygon replay (filled paths + ink area).
    # case029 is the true KiCad stroke-font case: kicad-cli plots one
    # path per stroke segment, matched by the gr_text
    # ``polyline_per_segment`` granularity in the IR converter.
    # case030-032 pin a second TTF face ("Wavenumber" render_cache)
    # across silk / copper / mask layers.
    SyntheticOracleCase(
        case_id="text_ttf_arial_silk",
        board_relpath="case026__text_stroke_basic/simple_text.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes", "filled_black_ink_area"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="text_ttf_arial_bold_silk",
        board_relpath="case027__text_stroke_bold/simple_text_bold.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes", "filled_black_ink_area"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="text_ttf_arial_italic_silk",
        board_relpath="case028__text_stroke_italic/simple_test_italic.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes", "filled_black_ink_area"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="text_stroke_kicad_font_silk",
        board_relpath="case029__text_stroke_kicad_font/simple_test_kicad_font.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes"),
        minimums=(("total_strokes", 4),),
    ),
    SyntheticOracleCase(
        case_id="text_ttf_wavenumber_silk",
        board_relpath="case030__text_ttf_arial/complex_font.kicad_pcb",
        layers=("F.SilkS",),
        metrics=("viewbox", "total_strokes", "filled_black_ink_area"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="text_ttf_wavenumber_top_copper",
        board_relpath="case031__text_ttf_arial_top/complex_font_top_layer.kicad_pcb",
        layers=("F.Cu",),
        metrics=("viewbox", "total_strokes", "filled_black_ink_area"),
        minimums=(("total_strokes", 1),),
    ),
    SyntheticOracleCase(
        case_id="text_ttf_wavenumber_top_mask",
        board_relpath="case032__text_ttf_arial_top_mask/complex_font_top_layer_top_soldermask.kicad_pcb",
        layers=("F.Mask",),
        metrics=("viewbox", "total_strokes", "filled_black_ink_area"),
        minimums=(("total_strokes", 1),),
    ),
)


# IR-only oracle cases pin behaviours covered by the PCB IR renderer.
# Consumed exclusively by ``test_L3_007_pcb_ir_svg_oracle``.
IR_ONLY_ORACLE_CASES: Tuple[SyntheticOracleCase, ...] = (
    # Phase E blocker #2 (2026-05-18): case082 has 16 vias including
    # 3 untented variants (``(tenting (front no) (back no))`` etc.)
    # that produce mask openings + drill knockouts on F.Mask / B.Mask.
    # Pins the via-mask synthesis in :func:`via_to_record`
    # (``via_mask_opening`` + ``via_mask_drill`` ops) against CLI.
    SyntheticOracleCase(
        case_id="pad_per_layer_shapes_f_mask",
        board_relpath="case082__pad_per_layer_shapes/synthetic_pad_shapes.kicad_pcb",
        layers=("F.Mask",),
        metrics=("viewbox", "total_circles", "white_drill_circles"),
        minimums=(("white_drill_circles", 1),),
    ),
    SyntheticOracleCase(
        case_id="pad_per_layer_shapes_b_mask",
        board_relpath="case082__pad_per_layer_shapes/synthetic_pad_shapes.kicad_pcb",
        layers=("B.Mask",),
        metrics=("viewbox", "total_circles", "white_drill_circles"),
        minimums=(("white_drill_circles", 1),),
    ),
)


def pcb_foundation_dir() -> Path:
    """Return the synthetic PCB foundation corpus root.

    Migrated 2026-05-17 from ``<corpus>/kicad/board_svg/input/<case>/`` to
    the per-case ``<corpus>/kicad/pcb_foundation/<case>/{input,
    reference_output, output}/`` layout used by all kicad_monkey
    validation work (parsing, IR, SVG, IPC, viz, data-model).
    """
    return get_kicad_pcb_foundation_dir()


def resolve_case_board_path(case: SyntheticOracleCase) -> Path:
    """Resolve a case's board path on disk.

    ``case.board_relpath`` stays in its original ``<case>/<file>`` form;
    this helper injects the ``input/`` segment for the pcb_foundation
    per-case layout.
    """
    case_dir, _, filename = case.board_relpath.partition("/")
    return pcb_foundation_dir() / case_dir / "input" / filename


def find_kicad_cli() -> Path | None:
    """Find a KiCad 9/10 ``kicad-cli`` executable."""
    return resolve_kicad_cli(required_capability="pcb_svg")


def export_svg_with_kicad_cli(
    *,
    kicad_cli: Path,
    board_path: Path,
    layers: Iterable[str],
    output_path: Path,
    timeout_s: int = 60,
) -> None:
    """Export board SVG with kicad-cli using deterministic CLI options."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    layer_csv = ",".join(layers)

    result = subprocess.run(
        [
            str(kicad_cli),
            "pcb",
            "export",
            "svg",
            "--black-and-white",
            "--layers",
            layer_csv,
            "--mode-single",
            "--page-size-mode",
            "2",
            "--exclude-drawing-sheet",
            "--drill-shape-opt",
            "2",
            "--output",
            str(output_path),
            str(board_path),
        ],
        capture_output=True,
        text=True,
        env=kicad_cli_subprocess_env(kicad_cli),
        timeout=timeout_s,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"kicad-cli export failed ({board_path.name}, layers={layer_csv}): {result.stderr.strip()}"
        )
    if not output_path.exists():
        raise RuntimeError(
            f"kicad-cli export produced no output ({board_path.name}, layers={layer_csv})"
        )


def semantic_snapshot(svg: str) -> Dict[str, object]:
    """Extract semantic metrics used for synthetic oracle comparison.

    The canonical SVG analyzer applies inherited group styles, element style
    overrides, and transforms before producing renderer-agnostic count and area
    metrics. This wrapper keeps the historical helper name used by L3 tests and
    oracle-generation scripts.
    """
    return canonical_semantic_metrics(svg)


def compare_semantic_metrics(
    ours: Dict[str, object],
    reference: Dict[str, object],
    selected_metrics: Iterable[str],
    *,
    # Keep this semantic check focused on gross canvas drift. Canonical KiCad
    # CLI builds differ by ~0.07 mm on these tiny board outlines.
    viewbox_tol_mm: float = 0.1,
) -> list[str]:
    """Compare selected semantic metrics and return mismatch messages."""
    issues: list[str] = []
    for metric in selected_metrics:
        if metric == "viewbox":
            ours_vb = ours.get("viewbox")
            ref_vb = reference.get("viewbox")
            if not isinstance(ours_vb, tuple) or not isinstance(ref_vb, tuple):
                issues.append("viewbox metric malformed")
                continue
            ours_vb = cast(tuple[float | int | str, ...], ours_vb)
            ref_vb = cast(tuple[float | int | str, ...], ref_vb)
            deltas = [abs(float(a) - float(b)) for a, b in zip(ours_vb, ref_vb)]
            if any(delta > viewbox_tol_mm for delta in deltas):
                issues.append(
                    f"viewBox mismatch ours={ours_vb} ref={ref_vb} deltas={tuple(round(d, 4) for d in deltas)}"
                )
            continue

        if metric == "filled_black_ink_area":
            ours_area = float(cast(float | int | str, ours.get(metric, 0.0)))
            ref_area = float(cast(float | int | str, reference.get(metric, 0.0)))
            tolerance = max(0.005, abs(ref_area) * 0.02)
            delta = abs(ours_area - ref_area)
            if delta > tolerance:
                issues.append(
                    f"{metric} mismatch ours={ours_area} ref={ref_area} "
                    f"delta={round(delta, 4)} tolerance={round(tolerance, 4)}"
                )
            continue

        ours_value = ours.get(metric)
        ref_value = reference.get(metric)
        if ours_value != ref_value:
            issues.append(f"{metric} mismatch ours={ours_value} ref={ref_value}")
    return issues
