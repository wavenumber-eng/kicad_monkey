"""
KiCad Footprint Extractor

Utility to extract footprints from KiCad PCB files (.kicad_pcb) and save them
as individual .kicad_mod files. Useful for extracting custom footprints embedded
in PCB files into a library structure.

Usage:
    # Extract from a single PCB file
    extract_footprints_from_pcb('path/to/file.kicad_pcb', 'output/dir')

    # Extract from all PCBs in a project
    extract_footprints_from_project('path/to/project.kicad_pro', 'output/dir')
"""

import logging
from pathlib import Path

from .kicad_footprint import KiCadFootprint
from .kicad_pcb_footprint import Footprint
from .kicad_sexpr import build_sexp, format_sexp
from .kicad_targeted_reader import iter_kicad_objects_from_text

log = logging.getLogger(__name__)


def sanitize_filename(name: str) -> str:
    """
    Sanitize a footprint name to be a valid filename.

    Replaces invalid filename characters with underscores.

    Args:
        name: Footprint name (may include library prefix like "speedy:QFN-28")

    Returns:
        Valid filename string
    """
    # Remove library prefix if present (e.g., "speedy:QFN-28" -> "QFN-28")
    if ':' in name:
        name = name.split(':', 1)[1]

    # Replace invalid filename characters with underscore
    invalid_chars = r'<>:"/\|?*'
    for char in invalid_chars:
        name = name.replace(char, '_')

    # Also replace spaces and other problematic characters
    name = name.replace(' ', '_')
    name = name.replace('\t', '_')
    name = name.replace('\n', '_')
    name = name.replace('\r', '_')

    return name


def _standalone_footprint_from_board_footprint(footprint: Footprint) -> KiCadFootprint:
    """Convert a PCB-placed footprint into a standalone library footprint."""
    sexp = footprint.to_sexp()
    footprint_name = str(footprint.library_link)
    if ":" in footprint_name:
        footprint_name = footprint_name.split(":", 1)[1]
    sexp[1] = footprint_name

    instance_fields = {"at", "uuid", "path", "sheetname", "sheetfile", "tstamp"}
    sexp[:] = [
        child
        for child in sexp
        if not (isinstance(child, list) and child and child[0] in instance_fields)
    ]

    return KiCadFootprint.from_sexp(sexp)


def extract_footprints_from_text(pcb_text: str) -> list[KiCadFootprint]:
    """
    Extract standalone footprint objects from PCB file text.

    Args:
        pcb_text: Contents of a .kicad_pcb file

    Returns:
        List of KiCadFootprint objects ready to write as .kicad_mod files
    """
    try:
        return [
            _standalone_footprint_from_board_footprint(footprint)
            for footprint in iter_kicad_objects_from_text(pcb_text, Footprint)
        ]
    except Exception as e:
        log.error(f"Failed to parse PCB file: {e}")
        return []


def extract_footprints_from_pcb(
    pcb_path: Path,
    output_dir: Path,
    overwrite: bool = False,
    unique_only: bool = True
) -> int:
    """
    Extract all footprints from a KiCad PCB file and save them as individual .kicad_mod files.

    Args:
        pcb_path: Path to .kicad_pcb file
        output_dir: Directory to save extracted footprint files
        overwrite: If True, overwrite existing files. If False, skip existing files.
        unique_only: If True, only extract one instance of each footprint name.
                     If False, extract all instances (may create duplicates).

    Returns:
        Number of footprints extracted
    """
    pcb_path = Path(pcb_path)
    output_dir = Path(output_dir)

    if not pcb_path.exists():
        log.error(f"PCB file not found: {pcb_path}")
        return 0

    log.info(f"Extracting footprints from: {pcb_path.name}")

    # Read PCB file
    try:
        with open(pcb_path, encoding='utf-8') as f:
            pcb_text = f.read()
    except Exception as e:
        log.error(f"Failed to read PCB file: {e}")
        return 0

    # Extract footprints
    footprints = extract_footprints_from_text(pcb_text)

    if not footprints:
        log.info(f"  No footprints found in {pcb_path.name}")
        return 0

    log.info(f"  Found {len(footprints)} footprint instance(s)")

    # If unique_only is True, keep only the first instance of each footprint name
    if unique_only:
        seen_names = set()
        unique_footprints = []
        for footprint in footprints:
            if footprint.name not in seen_names:
                seen_names.add(footprint.name)
                unique_footprints.append(footprint)
        footprints = unique_footprints
        log.info(f"  Unique footprints: {len(footprints)}")

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    # Save each footprint to its own file
    extracted_count = 0
    for footprint in footprints:
        # Sanitize the filename
        safe_name = sanitize_filename(footprint.name)
        output_file = output_dir / f"{safe_name}.kicad_mod"

        # Check if file exists
        if output_file.exists() and not overwrite:
            log.info(f"  Skipping '{footprint.name}' (file exists): {output_file.name}")
            continue

        # The footprint S-expression is a complete .kicad_mod file
        # Format it nicely before writing
        try:
            formatted_sexp = format_sexp(
                build_sexp(footprint.to_sexp()),
                indentation_size=2,
                max_nesting=2,
            )

            with open(output_file, 'w', encoding='utf-8') as f:
                f.write(formatted_sexp)
                # Ensure file ends with newline
                if not formatted_sexp.endswith('\n'):
                    f.write('\n')
            log.info(f"  Extracted '{footprint.name}' -> {output_file.name}")
            extracted_count += 1
        except Exception as e:
            log.error(f"  Failed to write footprint '{footprint.name}': {e}")

    return extracted_count


def find_pcbs_in_project(project_path: Path) -> list[Path]:
    """
    Find all PCB files associated with a KiCad project.

    Looks for .kicad_pcb files in the same directory as the project file.
    The main PCB typically has the same base name as the project.

    Args:
        project_path: Path to .kicad_pro file

    Returns:
        List of paths to PCB files
    """
    project_path = Path(project_path)
    project_dir = project_path.parent
    project_name = project_path.stem

    pcbs = []

    # Look for main PCB with same name as project
    main_pcb = project_dir / f"{project_name}.kicad_pcb"
    if main_pcb.exists():
        pcbs.append(main_pcb)

    # Find all other .kicad_pcb files in the project directory
    for pcb_file in project_dir.glob("*.kicad_pcb"):
        if pcb_file not in pcbs:
            pcbs.append(pcb_file)

    return pcbs


def extract_footprints_from_project(
    project_path: Path,
    output_dir: Path,
    overwrite: bool = False,
    unique_only: bool = True
) -> int:
    """
    Extract all footprints from all PCBs in a KiCad project.

    Args:
        project_path: Path to .kicad_pro file
        output_dir: Directory to save extracted footprint files
        overwrite: If True, overwrite existing files. If False, skip existing files.
        unique_only: If True, only extract one instance of each footprint name.

    Returns:
        Total number of footprints extracted
    """
    project_path = Path(project_path)

    if not project_path.exists():
        log.error(f"Project file not found: {project_path}")
        return 0

    log.info("=" * 80)
    log.info(f"Extracting footprints from project: {project_path.name}")
    log.info("=" * 80)

    # Find all PCBs
    pcbs = find_pcbs_in_project(project_path)

    if not pcbs:
        log.warning(f"No PCB files found for project: {project_path.name}")
        return 0

    log.info(f"Found {len(pcbs)} PCB file(s)")

    # Extract footprints from each PCB
    total_extracted = 0
    for pcb in pcbs:
        count = extract_footprints_from_pcb(pcb, output_dir, overwrite, unique_only)
        total_extracted += count

    log.info("=" * 80)
    log.info(f"Total footprints extracted: {total_extracted}")
    log.info("=" * 80)

    return total_extracted


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 3:
        log.info("Usage:")
        log.info("  python kicad_footprint_extractor.py <input_file> <output_dir> [options]")
        log.info("")
        log.info("  <input_file>  - Path to .kicad_pcb or .kicad_pro file")
        log.info("  <output_dir>  - Directory to save extracted .kicad_mod files")
        log.info("")
        log.info("Options:")
        log.info("  --overwrite   - Overwrite existing files")
        log.info("  --all         - Extract all footprint instances (not just unique)")
        sys.exit(1)

    input_file = Path(sys.argv[1])
    output_dir = Path(sys.argv[2])
    overwrite = '--overwrite' in sys.argv
    unique_only = '--all' not in sys.argv

    if not input_file.exists():
        log.info(f"Error: Input file not found: {input_file}")
        sys.exit(1)

    # Determine if it's a project or PCB file
    if input_file.suffix == '.kicad_pro':
        count = extract_footprints_from_project(input_file, output_dir, overwrite, unique_only)
    elif input_file.suffix == '.kicad_pcb':
        count = extract_footprints_from_pcb(input_file, output_dir, overwrite, unique_only)
    else:
        log.info("Error: Input file must be .kicad_pro or .kicad_pcb")
        sys.exit(1)

    log.info(f"\nDone! Extracted {count} footlog.info(s) to {output_dir}")
