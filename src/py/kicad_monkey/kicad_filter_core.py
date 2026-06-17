"""
KiCad filters - Main entry point and utilities.

This module provides:
- KiCadFilterPipeline for filtering KiCad files
- S-expression formatting utilities
- CLI for testing filters

Individual filter implementations are in separate modules:
- footprint.py  - Footprint filters (fp_filter__*)
- symbol.py     - Symbol filters (sym_filter__*)
- schematic.py  - Schematic filters (sch_filter__*)
- pcb.py        - PCB filters (pcb_filter__*)
"""
import argparse
import logging
import sys
import time
from pathlib import Path
from typing import Any

from ._files import compose, find_files

from .kicad_filter_footprint import (
    fp_filter__clean_fab,
    fp_filter__fix_fp_text_font_to_arial,
    fp_filter__fix_zero_sized_pads,
    fp_filter__normalized_embedded_model_naming,
    fp_filter__orthographic_projection_outline,
)
from .kicad_filter_footprint_metadata import fp_filter__remove_schematic_inherited_metadata
from .kicad_filter_pcb import (
    pcb_filter__process_embedded_footprints,
    pcb_filter__reset_layer_user_names,
)
from .kicad_filter_schematic import (
    sch_filter__remove_altium_value_property,
)
from .kicad_filter_symbol import (
    sym_filter__clear_property_values,
    sym_filter__remove_nonstandard_properties,
    sym_filter__standardize_reference_value_fonts,
)
from .kicad_sexpr import build_sexp, format_sexp, parse_sexp

log = logging.getLogger(__name__)

__all__ = [
    # Main filter entry point
    'KiCadFilterPipeline',

    # Formatting utilities
    'format_kicad_sexp',

    # Individual footprint filters
    'fp_filter__clean_fab',
    'fp_filter__fix_fp_text_font_to_arial',
    'fp_filter__fix_zero_sized_pads',
    'fp_filter__normalized_embedded_model_naming',
    'fp_filter__orthographic_projection_outline',
    'fp_filter__remove_schematic_inherited_metadata',

    # Individual PCB filters
    'pcb_filter__process_embedded_footprints',
    'pcb_filter__reset_layer_user_names',

    # Individual schematic filters
    'sch_filter__remove_altium_value_property',

    # Individual symbol filters
    'sym_filter__clear_property_values',
    'sym_filter__remove_nonstandard_properties',
    'sym_filter__standardize_reference_value_fonts',
]


def format_kicad_sexp(sexp: Any) -> str:
    """
    Format a parsed s-expression for KiCad with proper indentation using the standard
    format_sexp() function but with appropriate parameters for KiCad files.

    KiCad uses tabs for indentation, so we convert spaces to tabs after formatting.
    """
    # Use format_sexp with high max_nesting to format at all levels
    # indentation_size=1 because we'll convert to tabs (1 space -> 1 tab)
    formatted = format_sexp(build_sexp(sexp), indentation_size=1, max_nesting=100)

    # Convert leading spaces to tabs (KiCad standard)
    lines = formatted.split('\n')
    result_lines = []
    for line in lines:
        # Count leading spaces
        leading_spaces = len(line) - len(line.lstrip(' '))
        # Replace leading spaces with tabs
        if leading_spaces > 0:
            tabs = '\t' * leading_spaces
            rest = line[leading_spaces:]
            result_lines.append(tabs + rest)
        else:
            result_lines.append(line)

    return '\n'.join(result_lines)


def _emit(sexp: Any) -> str:
    """Format a parsed s-expression for KiCad and ensure a trailing newline."""
    out = format_kicad_sexp(sexp)
    if not out.endswith('\n'):
        out += '\n'
    return out


def _run_filters(
    path_in: Path,
    out_path: Path,
    filters: list,
    *,
    log_progress: bool = False,
) -> None:
    """Read path_in, parse, apply the filter chain, emit to out_path.

    Shared dispatch for the KiCadFilterPipeline file-level methods. Filters are
    applied left-to-right; an empty list means parse-and-reformat only.
    Set log_progress=True for verbose timing/sizing logs (used by the PCB
    entry point because PCB formatting can be slow).
    """
    if log_progress:
        log.info(f"Reading input file: {path_in.name}")
    with open(path_in, encoding='utf-8') as f:
        original_text = f.read()
    if log_progress:
        log.info(f"  File size: {len(original_text)} bytes")
        log.info("Parsing s-expression...")

    sexp = parse_sexp(original_text)
    if log_progress:
        log.info(f"  Parsed {len(sexp)} top-level items")

    if filters:
        sexp = compose(*filters)(sexp)

    if log_progress:
        log.info("Formatting output (this may take a moment for large files)...")
        start_time = time.time()
    else:
        start_time = 0.0
    final_output = _emit(sexp)
    if log_progress:
        elapsed = time.time() - start_time
        log.info(f"  Formatting complete in {elapsed:.1f}s")
        log.info(f"  Output size: {len(final_output):,} bytes")
        log.info(f"Writing output file: {out_path.name}")

    with open(out_path, "w", encoding='utf-8') as f:
        f.write(final_output)
    if log_progress:
        log.info("  Write complete")
        log.info(f"PCB filter processing complete: {path_in.name} -> {out_path.name}")


class KiCadFilterPipeline:
    """File-level KiCad cleanup transforms for generated or migrated artifacts."""

    def filter_footprint(self, path_in: str | Path, out_path: str | Path) -> None:
        """Apply all footprint filters to a `.kicad_mod` file, in order."""
        _run_filters(Path(path_in), Path(out_path), [
            fp_filter__clean_fab,
            fp_filter__fix_zero_sized_pads,
            fp_filter__fix_fp_text_font_to_arial,
            fp_filter__remove_schematic_inherited_metadata,
            fp_filter__normalized_embedded_model_naming,
            fp_filter__orthographic_projection_outline,
        ])

    def filter_symbol(self, path_in: str | Path, out_path: str | Path) -> None:
        """Apply all symbol filters to a `.kicad_sym` file."""
        _run_filters(Path(path_in), Path(out_path), [
            sym_filter__standardize_reference_value_fonts,
            sym_filter__clear_property_values,
            sym_filter__remove_nonstandard_properties,
        ])

    def filter_schematic(self, path_in: str | Path, out_path: str | Path) -> None:
        """Apply all schematic filters to a `.kicad_sch` file."""
        _run_filters(Path(path_in), Path(out_path), [
            sch_filter__remove_altium_value_property,
        ])

    def filter_pcb(
        self,
        path_in: str | Path,
        out_path: str | Path,
        *,
        reset_layer_names: bool = False,
    ) -> None:
        """Apply PCB filters to a `.kicad_pcb` file."""
        filters = []
        if reset_layer_names:
            filters.append(pcb_filter__reset_layer_user_names)
        filters.append(pcb_filter__process_embedded_footprints)
        _run_filters(Path(path_in), Path(out_path), filters, log_progress=True)


if __name__ == "__main__":
    # Set up command line argument parsing
    parser = argparse.ArgumentParser(
        description='KiCad Filter Testing Tool - Selectively run different filter types',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog='''
Examples:
  python kicad_filters.py --all              # Run all filters
  python kicad_filters.py -f                 # Run footprint filters only
  python kicad_filters.py -p                 # Run PCB filters only
  python kicad_filters.py -s                 # Run schematic filters only
  python kicad_filters.py -y                 # Run symbol filters only
  python kicad_filters.py -f -p              # Run footprint and PCB filters
  python kicad_filters.py --pcb --schematics # Run PCB and schematic filters
        '''
    )

    parser.add_argument('-f', '--footprints', action='store_true',
                        help='Test footprint filters (.kicad_mod files)')
    parser.add_argument('-p', '--pcb', action='store_true',
                        help='Test PCB filters (.kicad_pcb files)')
    parser.add_argument('-s', '--schematics', action='store_true',
                        help='Test schematic filters (.kicad_sch files)')
    parser.add_argument('-y', '--symbols', action='store_true',
                        help='Test symbol filters (.kicad_sym files)')
    parser.add_argument('-a', '--all', action='store_true',
                        help='Test all filters (default if no options specified)')

    args = parser.parse_args()

    # If no arguments provided, show help and exit
    if not (args.footprints or args.pcb or args.schematics or args.symbols or args.all):
        parser.print_help()
        sys.exit(0)

    # If --all is specified, enable all filter types
    if args.all:
        args.footprints = True
        args.pcb = True
        args.schematics = True
        args.symbols = True

    test_dir = Path(__file__).parent / "test_cases"
    pipeline = KiCadFilterPipeline()

    # Test footprint filters
    if args.footprints:
        log.info("\n" + "="*80)
        log.info("TESTING FOOTPRINT FILTERS")
        log.info("="*80)

        fp_files = find_files(test_dir / "kicad" / "fp_filter" /"in", [".kicad_mod"], True)
        log.info(f"\nFound {len(fp_files)} footprint test files")

        filtered_dir = test_dir / "kicad" / "fp_filter" / "out"
        filtered_dir.mkdir(parents=True, exist_ok=True)

        for file_in in fp_files:
            file_out = filtered_dir / file_in.name  # Keep original filename
            log.info(f"\nProcessing: {file_in.name}")
            pipeline.filter_footprint(file_in, file_out)

    # Test symbol filters
    if args.symbols:
        log.info("\n" + "="*80)
        log.info("TESTING SYMBOL FILTERS")
        log.info("="*80)

        sym_files = find_files(test_dir / "kicad" / "symbol_filter_tests", [".kicad_sym"], True)
        log.info(f"\nFound {len(sym_files)} symbol test files")

        for file_in in sym_files:
            filtered_dir = test_dir / "kicad" / "symbol_filter_tests" / "filtered"
            filtered_dir.mkdir(parents=True, exist_ok=True)
            file_out = filtered_dir / file_in.name  # Keep original filename
            log.info(f"\nProcessing: {file_in.name}")
            pipeline.filter_symbol(file_in, file_out)

    # Test PCB filters
    if args.pcb:
        log.info("\n" + "="*80)
        log.info("TESTING PCB FILTERS")
        log.info("="*80)

        pcb_input_dir = test_dir / "kicad" / "project" / "in"
        pcb_output_dir = test_dir / "kicad" / "project" / "out"

        if pcb_input_dir.exists():
            pcb_files = find_files(pcb_input_dir, [".kicad_pcb"], True)
            log.info(f"\nFound {len(pcb_files)} PCB test files")

            pcb_output_dir.mkdir(parents=True, exist_ok=True)

            for file_in in pcb_files:
                file_out = pcb_output_dir / file_in.name  # Keep original filename
                log.info(f"\nProcessing: {file_in.name}")
                pipeline.filter_pcb(file_in, file_out)
        else:
            log.info(f"\nSkipping PCB tests - directory not found: {pcb_input_dir}")

    # Test schematic filters
    if args.schematics:
        log.info("\n" + "="*80)
        log.info("TESTING SCHEMATIC FILTERS")
        log.info("="*80)

        sch_input_dir = test_dir / "kicad" / "project" / "in"
        sch_output_dir = test_dir / "kicad" / "project" / "out"

        if sch_input_dir.exists():
            sch_files = find_files(sch_input_dir, [".kicad_sch"], True)
            log.info(f"\nFound {len(sch_files)} schematic test files")

            sch_output_dir.mkdir(parents=True, exist_ok=True)

            for file_in in sch_files:
                file_out = sch_output_dir / file_in.name  # Keep original filename
                log.info(f"\nProcessing: {file_in.name}")
                pipeline.filter_schematic(file_in, file_out)

            # Copy .kicad_pro and .kicad_pcb files (needed to open the full project)
            import shutil
            log.info("\nCopying project files (.kicad_pro and .kicad_pcb)...")
            for ext in [".kicad_pro", ".kicad_pcb"]:
                project_files = find_files(sch_input_dir, [ext], False)
                for file_in in project_files:
                    file_out = sch_output_dir / file_in.name
                    shutil.copy2(file_in, file_out)
                    log.info(f"  Copied: {file_in.name}")
        else:
            log.info(f"\nSkipping schematic tests - directory not found: {sch_input_dir}")

    log.info("\n" + "="*80)
    log.info("TESTING COMPLETE")
    log.info("="*80)
