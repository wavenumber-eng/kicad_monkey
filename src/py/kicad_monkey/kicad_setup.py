"""
KiCad setup functions - library table generation and configuration.

Handles creation and updates of KiCad symbol and footprint library tables.
For KiCad v10 and newer, symbols are exposed as a folder-based KiCad library.
"""

import logging
import re
import shutil
from pathlib import Path

from .kicad_environment import KiCadEnvironment
from .kicad_utilities import find_files, make_kicad_httplib

log = logging.getLogger(__name__)


def parse_entries_by_name(table_path: Path, table_start: str) -> dict[str, str]:
    """
    Parse a KiCad table file and return a dict mapping library name to entry line.

    Args:
        table_path: Path to KiCad table file (fp-lib-table or sym-lib-table).
        table_start: Table section marker, e.g. "(fp_lib_table".

    Returns:
        Mapping of library nickname -> full entry line.
    """
    entries = {}

    if table_path.exists():
        with open(table_path) as f:
            in_table = False

            for line in f:
                if line.strip().startswith(table_start):
                    in_table = True
                    continue

                if in_table:
                    if line.strip().startswith(")"):
                        break

                    match = re.search(r'\(lib\s*\(name\s+"([^"]+)"\)', line)
                    if match:
                        entries[match.group(1)] = line.rstrip()

    return entries


def _prepare_kicad_symbol_folder(
    *,
    kicad_symbol_folder: Path,
    legacy_symbol_root: Path,
) -> tuple[Path, int]:
    """
    Ensure KiCad symbol files live in symbols/kicad for v10 folder libraries.

    Legacy layout had .kicad_sym files mixed with .SchLib files in SYMBOL_LOC root.
    We migrate only top-level legacy .kicad_sym files into symbols/kicad.

    Returns:
        Tuple of (kicad_symbol_folder, symbol_file_count).
    """
    kicad_symbol_folder.mkdir(parents=True, exist_ok=True)

    # One-time migration for legacy V9 layout: symbols/*.kicad_sym -> symbols/kicad/*.kicad_sym
    legacy_files = sorted(legacy_symbol_root.glob("*.kicad_sym"))
    moved_count = 0
    skipped_count = 0

    for src in legacy_files:
        dst = kicad_symbol_folder / src.name

        if dst.exists():
            skipped_count += 1
            continue

        try:
            shutil.move(str(src), str(dst))
            moved_count += 1
        except Exception as e:
            log.warning(f"Failed moving {src.name} to symbols/kicad: {e}")

    if moved_count > 0:
        log.info(f"Migrated {moved_count} legacy KiCad symbol files to {kicad_symbol_folder}")

    if skipped_count > 0:
        log.warning(
            f"Skipped {skipped_count} legacy symbol files during migration because "
            "a same-name file already exists in symbols/kicad"
        )

    symbol_files = find_files(kicad_symbol_folder, [".kicad_sym"], recursive=False)
    return kicad_symbol_folder, len(symbol_files)


def setup_kicad(
    *,
    dblib_loc: Path,
    symbol_root: Path,
    kicad_symbol_root: Path,
    footprint_root: Path,
    classic_symbol_root: Path,
    http_library_nickname: str,
    symbol_library_nickname: str,
    footprint_library_nickname: str,
    dblib_name: str | None = None,
    min_major: int = 10,
) -> None:
    """
    Setup KiCad library tables (footprints and symbols).

    Process:
    1. Find KiCad installation and configuration paths
    2. Generate HTTP library file
    3. Setup folder-based symbol library path for KiCad v10
    4. Write footprint and symbol table entries
    """
    log.info("Setting up Kicad")

    environment = KiCadEnvironment()
    environment.highest_installation(ignore_beta=False)

    config_paths = environment.find_config_paths(min_major=min_major)
    make_kicad_httplib(
        output_dir=dblib_loc,
        library_nickname=http_library_nickname,
    )

    # Note: source-library conversion is handled by separate manual workflows.
    if len(config_paths) == 0:
        log.error("Skipping library setup (no KiCad 10+ configuration paths found)")
        return

    # ========================== Footprint table entries ==================== #
    footprint_path = footprint_root.resolve().as_posix()
    fp_table_entries = [
        (
            f'(lib (name "{footprint_library_nickname}")'
            + ' (type "KiCad")'
            + f' (uri "{footprint_path}")'
            + ' (options "")'
            + f' (descr "{footprint_library_nickname} footprints")'
            + ")"
        )
    ]
    # ========================== Footprint table entries ==================== #

    # ========================== Symbol table entries ==================== #
    kicad_symbol_folder, symbol_count = _prepare_kicad_symbol_folder(
        kicad_symbol_folder=kicad_symbol_root,
        legacy_symbol_root=symbol_root,
    )
    log.info(f"{symbol_count} KiCad symbol files found in {kicad_symbol_folder}")

    # HTTP libraries
    kicad_httplib = dblib_loc / f"{http_library_nickname}.kicad_httplib"
    if kicad_httplib.exists():
        log.info(f"1 HTTP library found: {kicad_httplib.name}")
    else:
        log.warning(f"HTTP library not found: {kicad_httplib}")

    # Classic standalone symbols (power symbols, graphics)
    kicad_classic_symbols = find_files(classic_symbol_root, [".kicad_sym"], True)
    log.info(f"{len(kicad_classic_symbols)} stand-alone symbol libraries found")

    sym_table_entries = []

    if kicad_httplib.exists():
        try:
            path_string = kicad_httplib.resolve().as_posix()
            te = (
                f' (lib (name "{http_library_nickname}")'
                + ' (type "HTTP")'
                + f' (uri "{path_string}")'
                + ' (options "")'
                + f' (descr "{kicad_httplib.stem}")'
                + ")"
            )
            sym_table_entries.append(te)
        except Exception as e:
            log.error(f"Failed adding HTTP library {kicad_httplib}: {e}")

    for sym in kicad_classic_symbols:
        try:
            path_string = sym.resolve().as_posix()
            te = (
                f' (lib (name "{sym.stem}")'
                + ' (type "KiCad")'
                + f' (uri "{path_string}")'
                + ' (options "")'
                + f' (descr "{sym.stem}")'
                + ")"
            )
            sym_table_entries.append(te)
        except Exception as e:
            log.error(f"Failed adding stand-alone symbol library {sym}: {e}")

    # KiCad v10 folder-based symbol library entry.
    if symbol_count > 0:
        kicad_symbol_folder_path = kicad_symbol_folder.resolve().as_posix()
        te = (
            f' (lib (name "{symbol_library_nickname}")'
            + ' (type "KiCad")'
            + f' (uri "{kicad_symbol_folder_path}")'
            + ' (options "")'
            + f' (descr "{symbol_library_nickname} symbol folder ({symbol_count} files)")'
            + ' (hidden)'
            + ")"
        )
        sym_table_entries.append(te)
        log.info(f"Added folder-based KiCad symbol library: {symbol_library_nickname}")
    else:
        log.warning(
            "No .kicad_sym files found in symbols/kicad; "
            f"skipping {symbol_library_nickname} symbol table entry"
        )
    # ========================== Symbol table entries ==================== #

    for kicad_config_loc in config_paths:
        log.info("Writing library tables for " + str(kicad_config_loc))

        # -------------------------- fp-lib-table -------------------------- #
        kicad_fp_table = kicad_config_loc / Path("fp-lib-table")
        log.info("Writing global footprint table......")

        existing_fp_entries = parse_entries_by_name(kicad_fp_table, "(fp_lib_table")

        autogen_fp_entries = {}
        for te in fp_table_entries:
            match = re.search(r'\(lib\s*\(name\s+"([^"]+)"\)', te)
            if match:
                autogen_fp_entries[match.group(1)] = te

        merged_fp_entries = existing_fp_entries.copy()
        merged_fp_entries.update(autogen_fp_entries)

        with open(kicad_fp_table, "w") as f:
            f.write("(fp_lib_table\n")
            f.write(" (version 7)\n")
            for entry in merged_fp_entries.values():
                f.write("  " + entry.lstrip() + "\n")
            f.write(")\n")

        # -------------------------- sym-lib-table ------------------------- #
        kicad_symbol_table = kicad_config_loc / Path("sym-lib-table")
        log.info("Writing global symbol table......")

        existing_entries = parse_entries_by_name(kicad_symbol_table, "(sym_lib_table")

        # Remove old auto-generated entries from the V9 amalgamation system.
        # Keep folder entries (no .kicad_sym in URI), remove direct file entries.
        cleaned_entries = {}
        removed_count = 0

        for name, entry in existing_entries.items():
            entry_lower = entry.lower()
            is_old_amalgam_entry = bool(
                dblib_name and f"{dblib_name}.kicad_sym" in entry_lower
            )
            is_old_individual_entry = (
                ".kicad_sym" in entry_lower
                and ("dblib/symbols/" in entry_lower or "dblib\\symbols\\" in entry_lower)
            )

            if is_old_amalgam_entry or is_old_individual_entry:
                removed_count += 1
                continue

            cleaned_entries[name] = entry

        if removed_count > 0:
            log.info(f"Removed {removed_count} stale V9 symbol table entries")

        autogen_sym_entries = {}
        for te in sym_table_entries:
            match = re.search(r'\(lib\s*\(name\s+"([^"]+)"\)', te)
            if match:
                autogen_sym_entries[match.group(1)] = te

        merged_entries = cleaned_entries.copy()
        merged_entries.update(autogen_sym_entries)

        with open(kicad_symbol_table, "w") as f:
            f.write("(sym_lib_table\n")
            f.write(" (version 7)\n")
            for entry in merged_entries.values():
                f.write("  " + entry.lstrip() + "\n")
            f.write(")\n")
