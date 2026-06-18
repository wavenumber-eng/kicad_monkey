"""
KiCad Symbol Extractor

Utility to extract symbols from KiCad schematic files (.kicad_sch) and save them
as individual .kicad_sym files. Useful for importing local project symbols into
a library structure.

Usage:
    # Extract from a single schematic
    extract_symbols_from_schematic('path/to/file.kicad_sch', 'output/dir')

    # Extract from all schematics in a project
    extract_symbols_from_project('path/to/project.kicad_pro', 'output/dir')
"""

import logging
from pathlib import Path

from .kicad_lib_symbol import LibSymbol
from .kicad_sexpr import build_sexp, format_sexp, parse_sexp
from .kicad_targeted_reader import iter_kicad_objects_from_text

log = logging.getLogger(__name__)


def sanitize_filename(name: str) -> str:
    """
    Sanitize a symbol name to be a valid filename.

    Replaces invalid filename characters with underscores.

    Args:
        name: Symbol name (may include library prefix like "speedy:HMCAD1511TR")

    Returns:
        Valid filename string
    """
    # Remove library prefix if present (e.g., "speedy:HMCAD1511TR" -> "HMCAD1511TR")
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


def _clean_library_symbol_name(symbol_name: str) -> str:
    if ":" in symbol_name:
        return symbol_name.split(":", 1)[1]
    return symbol_name


def _update_subsymbol_names(node: object, clean_symbol_name: str) -> None:
    """Recursively update sub-symbol names to use the cleaned parent name."""
    if not isinstance(node, list):
        return

    for child in node:
        if isinstance(child, list) and len(child) > 1 and child[0] == "symbol":
            child_name = str(child[1])
            if "_" in child_name:
                parts = child_name.split("_")
                if len(parts) >= 3:
                    try:
                        int(parts[-2])
                        int(parts[-1])
                    except ValueError:
                        pass
                    else:
                        suffix = "_" + "_".join(parts[-2:])
                        child[1] = clean_symbol_name + suffix
        _update_subsymbol_names(child, clean_symbol_name)


def _standalone_symbol(symbol: LibSymbol) -> LibSymbol:
    sexp = symbol.to_sexp()
    clean_symbol_name = _clean_library_symbol_name(str(symbol.name))
    sexp[1] = clean_symbol_name
    _update_subsymbol_names(sexp, clean_symbol_name)
    return LibSymbol.from_sexp(sexp)


def extract_symbols_from_text(schematic_text: str) -> list[LibSymbol]:
    """
    Extract library symbol objects from schematic file text.

    Args:
        schematic_text: Contents of a .kicad_sch file

    Returns:
        List of LibSymbol objects ready to write into .kicad_sym files
    """
    try:
        return [
            _standalone_symbol(symbol)
            for symbol in iter_kicad_objects_from_text(schematic_text, LibSymbol)
        ]
    except Exception as e:
        log.error(f"Failed to parse schematic file: {e}")
        return []


def create_symbol_file_content(
    symbol: LibSymbol | str,
    symbol_sexp: str | None = None,
) -> str:
    """
    Create a complete .kicad_sym file content from a symbol S-expression.

    Args:
        symbol: LibSymbol object, or the legacy symbol name string
        symbol_sexp: Optional legacy symbol S-expression string

    Returns:
        Complete .kicad_sym file content with header and footer
    """
    if isinstance(symbol, LibSymbol):
        symbol_parsed = symbol.to_sexp()
    else:
        if symbol_sexp is None:
            raise TypeError("symbol_sexp is required when symbol is a name string")
        try:
            symbol_parsed = parse_sexp(symbol_sexp)
        except Exception:
            content = f"""(kicad_symbol_lib
\t(version 20241209)
\t(generator "kicad_symbol_editor")
\t(generator_version "9.0")
\t{symbol_sexp}
)
"""
            return content

    # Build the library structure with proper formatting
    library_structure = [
        'kicad_symbol_lib',
        ['version', 20241209],
        ['generator', 'kicad_symbol_editor'],
        ['generator_version', '9.0'],
        symbol_parsed
    ]

    # Convert to formatted S-expression
    sexp_str = build_sexp(library_structure)
    formatted = format_sexp(sexp_str, indentation_size=2, max_nesting=2)

    return formatted


def extract_symbols_from_schematic(
    schematic_path: Path,
    output_dir: Path,
    overwrite: bool = False
) -> int:
    """
    Extract all symbols from a KiCad schematic file and save them as individual .kicad_sym files.

    Args:
        schematic_path: Path to .kicad_sch file
        output_dir: Directory to save extracted symbol files
        overwrite: If True, overwrite existing files. If False, skip existing files.

    Returns:
        Number of symbols extracted
    """
    schematic_path = Path(schematic_path)
    output_dir = Path(output_dir)

    if not schematic_path.exists():
        log.error(f"Schematic file not found: {schematic_path}")
        return 0

    log.info(f"Extracting symbols from: {schematic_path.name}")

    # Read schematic file
    try:
        with open(schematic_path, encoding='utf-8') as f:
            schematic_text = f.read()
    except Exception as e:
        log.error(f"Failed to read schematic file: {e}")
        return 0

    # Extract symbols
    symbols = extract_symbols_from_text(schematic_text)

    if not symbols:
        log.info(f"  No embedded symbols found in {schematic_path.name}")
        return 0

    log.info(f"  Found {len(symbols)} symbol(s)")

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    # Save each symbol to its own file
    extracted_count = 0
    for symbol in symbols:
        # Sanitize the filename
        safe_name = sanitize_filename(symbol.name)
        output_file = output_dir / f"{safe_name}.kicad_sym"

        # Check if file exists
        if output_file.exists() and not overwrite:
            log.info(f"  Skipping '{symbol.name}' (file exists): {output_file.name}")
            continue

        # Create the complete symbol file content
        file_content = create_symbol_file_content(symbol)

        # Write to file
        try:
            with open(output_file, 'w', encoding='utf-8') as f:
                f.write(file_content)
            log.info(f"  Extracted '{symbol.name}' -> {output_file.name}")
            extracted_count += 1
        except Exception as e:
            log.error(f"  Failed to write symbol '{symbol.name}': {e}")

    return extracted_count


def find_schematics_in_project(project_path: Path) -> list[Path]:
    """
    Find all schematic files associated with a KiCad project.

    Looks for .kicad_sch files in the same directory as the project file.
    The main schematic typically has the same base name as the project.

    Args:
        project_path: Path to .kicad_pro file

    Returns:
        List of paths to schematic files
    """
    project_path = Path(project_path)
    project_dir = project_path.parent
    project_name = project_path.stem

    schematics = []

    # Look for main schematic with same name as project
    main_schematic = project_dir / f"{project_name}.kicad_sch"
    if main_schematic.exists():
        schematics.append(main_schematic)

    # Find all other .kicad_sch files in the project directory
    for schematic_file in project_dir.glob("*.kicad_sch"):
        if schematic_file not in schematics:
            schematics.append(schematic_file)

    return schematics


def extract_symbols_from_project(
    project_path: Path,
    output_dir: Path,
    overwrite: bool = False
) -> int:
    """
    Extract all symbols from all schematics in a KiCad project.

    Args:
        project_path: Path to .kicad_pro file
        output_dir: Directory to save extracted symbol files
        overwrite: If True, overwrite existing files. If False, skip existing files.

    Returns:
        Total number of symbols extracted
    """
    project_path = Path(project_path)

    if not project_path.exists():
        log.error(f"Project file not found: {project_path}")
        return 0

    log.info("=" * 80)
    log.info(f"Extracting symbols from project: {project_path.name}")
    log.info("=" * 80)

    # Find all schematics
    schematics = find_schematics_in_project(project_path)

    if not schematics:
        log.warning(f"No schematic files found for project: {project_path.name}")
        return 0

    log.info(f"Found {len(schematics)} schematic file(s)")

    # Extract symbols from each schematic
    total_extracted = 0
    for schematic in schematics:
        count = extract_symbols_from_schematic(schematic, output_dir, overwrite)
        total_extracted += count

    log.info("=" * 80)
    log.info(f"Total symbols extracted: {total_extracted}")
    log.info("=" * 80)

    return total_extracted


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 3:
        log.info("Usage:")
        log.info("  python kicad_symbol_extractor.py <input_file> <output_dir> [--overwrite]")
        log.info("")
        log.info("  <input_file>  - Path to .kicad_sch or .kicad_pro file")
        log.info("  <output_dir>  - Directory to save extracted .kicad_sym files")
        log.info("  --overwrite   - Overwrite existing files (optional)")
        sys.exit(1)

    input_file = Path(sys.argv[1])
    output_dir = Path(sys.argv[2])
    overwrite = '--overwrite' in sys.argv

    if not input_file.exists():
        log.info(f"Error: Input file not found: {input_file}")
        sys.exit(1)

    # Determine if it's a project or schematic file
    if input_file.suffix == '.kicad_pro':
        count = extract_symbols_from_project(input_file, output_dir, overwrite)
    elif input_file.suffix == '.kicad_sch':
        count = extract_symbols_from_schematic(input_file, output_dir, overwrite)
    else:
        log.info("Error: Input file must be .kicad_pro or .kicad_sch")
        sys.exit(1)

    log.info(f"\nDone! Extracted {count} symbol(s) to {output_dir}")
