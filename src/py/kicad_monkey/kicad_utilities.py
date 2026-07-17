"""Utilities for working with KiCad files, libraries, and preferences."""

import datetime
import importlib
import json
import logging
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from colorama import Fore

from ._files import find_files
from .kicad_environment import KiCadEnvironment

log = logging.getLogger(__name__)


__all__ = [
    # Project parsing (commonly used)
    'read_kicad_pro_parameters',

    # Symbol management
    'make_empty_kicad_symbol',

    # Library generation
    'make_kicad_httplib',
    'make_kicad_dblib',
    'convert_altium_libraries_to_kicad',

    # KiCad preference management
    'setup_kicad_preferences',
    'backup_kicad_preferences',
    'should_update_kicad_prefs',
    'deep_merge_json',

    # PCB file parsing helpers (used by kicad_pcb_parser.py)
    'get_first_or_value',

    # Utility functions
    'delete_folders',
    'is_fuzzy_match',
    'find_notepad_plus_plus',
]


def delete_folders(root_directory: str, name: str) -> None:
    """
    Delete all folders named 'History' under the given root directory.

    Args:
        root_directory (str): The root directory to start searching from
    """
    root_path = Path(root_directory)
    history_folders = [p for p in root_path.rglob("*") if p.is_dir() and p.name == name]

    for folder in history_folders:
        try:
            shutil.rmtree(folder)
            log.info(f"Deleted: {folder}")
        except Exception as e:
            log.error(f"Error deleting {folder}: {e}")

# Deprecated at the workflow level; retained for explicit DB library generation.
def make_kicad_dblib(
    name: str,
    tables: list[str],
    *,
    output_dir: Path,
    primary_key: str,
    dsn: str | None = None,
) -> Path:
    dblib = {}
    dblib["meta"] = {}
    dblib["meta"]["version"] = 0
    dblib["name"] = name
    dblib["decription"] = name

    source = {}
    source["type"] = "odbc"
    source["dsn"] = ""
    source["username"] = ""
    source["password"] = ""
    source["timeout_seconds"] = 10
    source["connection_string"] = (
        f"DSN={dsn or name};"
        "Extended Properties='text;HDR=Yes;FMT=Delimited;CharacterSet=65001'"
    )

    dblib["source"] = source

    dblib["libraries"] = []

    for table in tables:
        library = {}
        library["name"] = table
        library["table"] = table + ".csv"
        library["key"] = primary_key
        library["symbols"] = "kicad-symbol"
        library["footprints"] = "kicad-footprint"
        library["fields"] = []

        library["fields"].append(
            {"column": "Manufacturer",
             "name": "Manufacturer",
             "visible_on_add": False,
             "visible_in_chooser": True,
             "show_name": False}
        )

        library["fields"].append(
            {"column": "Manufacturer Part Number",
             "name": "Manufacturer Part Number",
             "visible_on_add": False,
             "visible_in_chooser": True,
             "show_name": False}
        )

        library["fields"].append(
            {"column": "Value",
             "name": "Value",
             "visible_on_add": True,
             "visible_in_chooser": True,
             "show_name": False}
        )

        library["fields"].append(
            {"column": "Description",
             "name": "Description",
             "visible_on_add": False,
             "visible_in_chooser": True,
             "show_name": False}
        )

        library["fields"].append(
            {"column": "flags",
             "name": "flags",
             "visible_on_add": False,
             "visible_in_chooser": True,
             "show_name": False}
        )

        dblib["libraries"].append(library)

    output_dir.mkdir(parents=True, exist_ok=True)
    dblib_path = output_dir / Path(name + ".kicad_dbl")

    log.info(f"Writing {dblib_path}")

    with open(dblib_path, 'w', encoding='utf-8') as f:
        json.dump(dblib, f, indent=2, ensure_ascii=False)
    return dblib_path

def make_kicad_httplib(
    *,
    output_dir: Path,
    library_nickname: str,
    root_url: str = "http://127.0.0.1:8761/",
    name: str | None = None,
    description: str | None = None,
    cleanup_existing: bool = True,
) -> Path:
    root_url = root_url.strip() or "http://127.0.0.1:8761/"
    if not root_url.endswith("/"):
        root_url = f"{root_url}/"

    output_dir.mkdir(parents=True, exist_ok=True)
    existing_files = find_files(output_dir, [".kicad_httplib"], False) if cleanup_existing else []

    for httplib_file in existing_files:
        try:
            os.remove(httplib_file)
        except Exception as e:
            log.warning(f"Could not remove {httplib_file}: {e}")

    # KiCad's HTTP plugin derives the loaded library nickname from the .kicad_httplib
    # filename rather than the sym-lib-table nickname. Keep the filename aligned with the
    # actual HTTP library nickname so chooser preview reloads target the HTTP library.
    kicad_http_lib_path = output_dir / Path(f"{library_nickname}.kicad_httplib")
    display_name = name or f"{library_nickname} HTTP Library"
    httplib_payload = {
        "meta": {"version": 1.0},
        "name": display_name,
        "description": description or display_name,
        "source": {
            "type": "REST_API",
            "api_version": "v1",
            "root_url": root_url,
            "token": "",
            "timeout_parts_seconds": 60,
            "timeout_categories_seconds": 600,
        },
    }

    try:
        with open(kicad_http_lib_path, "w", encoding="utf-8", newline="\n") as f:
            json.dump(httplib_payload, f, indent=4, ensure_ascii=False)
            f.write("\n")
    except Exception as e:
        log.warning(f"Could not create {kicad_http_lib_path}: {e}")

    return kicad_http_lib_path

def make_empty_kicad_symbol(path: Path) -> None:
    with open(path, 'w') as f:  # Windows line endings
        f.write('(kicad_symbol_lib')
        f.write('    (version 20241209)')
        f.write('    (generator "kicad_symbol_editor")')
        f.write('    (generator_version "9.0")')
        f.write(')')

def convert_altium_libraries_to_kicad(
    clean_symbols: bool = False,
    clean_footprints: bool = False,
    *,
    symbol_root: Path,
    footprint_root: Path,
    kicad_symbol_root: Path | None = None,
) -> None:
    log.info("migrating kicad symbols and footprints")

    # for now,  lets not use the beta version of the cli
    installation = KiCadEnvironment().highest_installation(ignore_beta=True)

    log.info(f"using KiCad installation: {installation.root if installation else None}")
    if installation is None:
        raise RuntimeError("Could not locate a KiCad installation for library conversion")
    kicad_cli_exe = installation.kicad_cli

    symbol_root = Path(symbol_root)
    footprint_root = Path(footprint_root)
    kicad_symbol_output = Path(kicad_symbol_root or symbol_root / "kicad")
    kicad_symbol_output.mkdir(parents=True, exist_ok=True)
    altium_symbols = find_files(symbol_root, [".SchLib"], True)

    total_symbols = len(altium_symbols)
    processed_symbols = 0

    if clean_symbols:
        for existing_sym in find_files(kicad_symbol_output, [".kicad_sym"], False):
            log.warning("Cleaning symbol : " + str(existing_sym))
            existing_sym.unlink(missing_ok=True)

        # Clean legacy top-level symbol files from pre-v10 layout.
        for legacy_sym in symbol_root.glob("*.kicad_sym"):
            log.warning("Cleaning legacy symbol : " + str(legacy_sym))
            legacy_sym.unlink(missing_ok=True)

    for sym in altium_symbols:

        kicad_sym = kicad_symbol_output / Path(sym.stem + ".kicad_sym")
        legacy_kicad_sym = symbol_root / Path(sym.stem + ".kicad_sym")

        log.info(kicad_sym)

        if not kicad_sym.exists() and legacy_kicad_sym.exists():
            shutil.move(str(legacy_kicad_sym), str(kicad_sym))
            log.info(f"Migrated legacy symbol file to {kicad_sym}")
            continue

        if not kicad_sym.exists():
            log.info(f"Migrating {sym.name} to {kicad_sym.name}")

            log.info(f"{str(kicad_sym)}")
            args = ["kicad-cli", "sym", "upgrade", "--output", str(kicad_sym), str(sym)]
            try:
                result = subprocess.run([kicad_cli_exe] + args[1:],
                                        capture_output=True,
                                        text=True,
                                        check=True)
                log.info("Output:" + str(result.stdout))
                processed_symbols = processed_symbols + 1
            except subprocess.CalledProcessError as e:
                log.error(f"Error code: {e.returncode}")
                log.error(f"Command: {e.cmd}")
                log.error(f"stdout: {e.stdout}")
                log.error(f"stderr: {e.stderr}")
                exit(-1)

    log.info(f"\nProcessed: {processed_symbols} of {total_symbols} symbols\n")

    altium_footprints = find_files(footprint_root, [".PcbLib"], True)

    total_footprints = len(altium_footprints)
    processed_footprints = 0

    log.info("Total footprints: " + str(total_footprints))

    for fp in altium_footprints:
        pretty = fp.parent / (fp.stem + ".pretty")
        kicad_mod = fp.parent / f"{fp.stem}.kicad_mod"

        if clean_footprints:

            if pretty.exists():
                log.warning(f"Cleaning {pretty}")
                shutil.rmtree(pretty)

            if kicad_mod.exists():
                log.warning(f"Cleaning {kicad_mod}")
                kicad_mod.unlink(missing_ok=True)
        if kicad_mod.exists() and not pretty.exists():
            log.info(f"Skipping: {Fore.GREEN}{fp.name}{Fore.WHITE} — already converted to "
                    f"{Fore.YELLOW}{kicad_mod}")
            continue

        log.info(f"Migrating {fp.name} to {fp.parent}")
        processed_footprints += 1

        args = ["fp", "upgrade", "--output", str(pretty), str(fp)]
        try:
            # To make sure this subprocess has no issues.
            if pretty.exists():
                shutil.rmtree(pretty)
            result = subprocess.run(
                [str(kicad_cli_exe)] + args,
                capture_output=True,
                text=True,
                encoding="utf-8",
                check=True
            )
            log.info("Output: " + result.stdout)
        except subprocess.CalledProcessError as e:
            log.error(f"Error code: {e.returncode}")
            log.error(f"Command: {e.cmd}")
            log.error(f"stdout: {e.stdout}")
            log.error(f"stderr: {e.stderr}")
            continue

        if pretty.exists():
            for file_path in pretty.iterdir():
                if file_path.is_file() and file_path.suffix == ".kicad_mod":
                    destination = fp.parent / f"{fp.stem}.kicad_mod"
                    shutil.copy2(file_path, destination)
                    log.info(f"Copied {file_path} to {destination}")
            shutil.rmtree(pretty)

    log.info(f"\nProcessed: {processed_footprints} of {total_footprints} footprints\n")

def get_first_or_value(variable):
    """
    Get first element of a list or return value directly.

    Used by kicad_pcb_parser.py for s-expression processing.

    Args:
        variable: Either a list or a scalar value

    Returns:
        First element if list, else the value itself
    """
    if isinstance(variable, list):
        if len(variable) > 0:
            return variable[0]
        else:
            return None  # Return None for empty lists
    else:
        return variable


def is_fuzzy_match(search_term: str, target: str) -> bool:
    """
    Fuzzy match with bidirectional substring matching and RapidFuzz similarity.

    Matching logic:
    1. Exact match (case-insensitive)
    2. Bidirectional substring match (search in target OR target in search)
    3. Multi-part token matching (all search parts found in target parts)
    4. RapidFuzz similarity score (if rapidfuzz available)

    Args:
        search_term: The search query
        target: The target string to match against

    Returns:
        True if match found, False otherwise
    """
    search_lower = search_term.lower()
    target_lower = target.lower()

    # Exact match
    if search_lower == target_lower:
        return True

    # Bidirectional substring match
    # Matches: "PEC11R-4215K-S0024" finds "PEC11R" OR "PEC11" finds "PEC11R"
    if search_lower in target_lower or target_lower in search_lower:
        return True

    # Split on common separators and check if all parts are present
    search_parts = re.split(r'[-_\s]+', search_lower)
    target_parts = re.split(r'[-_\s]+', target_lower)

    # All search parts must be found in target parts (order doesn't matter)
    if all(any(search_part in target_part for target_part in target_parts)
           for search_part in search_parts):
        return True

    # RapidFuzz similarity check (if available)
    try:
        fuzz = importlib.import_module("rapidfuzz.fuzz")
        # Use token_sort_ratio for better matching with reordered/partial strings
        similarity = fuzz.token_sort_ratio(search_lower, target_lower)
        # Match if similarity > 70 (adjustable threshold)
        if similarity > 70:
            return True
    except ImportError:
        # RapidFuzz not available, skip fuzzy similarity check
        pass

    return False


# ============================================================================
# KiCad Preferences Management
# ============================================================================

# KiCad preferences template version - increment when template changes
KICAD_PREFS_VERSION = 3  # Updated to support source-driven preference files


def should_update_kicad_prefs(user_prefs_version: int, dont_ask: bool) -> bool:
    """
    Check if KiCad preferences should be updated.

    Args:
        user_prefs_version: The version of KiCad prefs the user currently has
        dont_ask: User's "don't ask again" preference

    Returns:
        True if prefs need updating and user hasn't disabled prompts
    """
    if dont_ask:
        return False

    return user_prefs_version < KICAD_PREFS_VERSION


def find_notepad_plus_plus():
    """
    Check if Notepad++ exists on the user's machine.
    Returns the path if found, None otherwise.
    """
    possible_paths = [
        Path("C:/Program Files/Notepad++/notepad++.exe"),
        Path("C:/Program Files (x86)/Notepad++/notepad++.exe"),
    ]

    for path in possible_paths:
        if path.exists():
            return path.as_posix()
    return None


def deep_merge_json(target: dict, source: dict, keys_to_merge: list | None = None):
    """
    Selectively merge source dict into target dict.
    If keys_to_merge is provided, only those top-level keys will be updated.
    Nested dicts are merged recursively.

    Args:
        target: The dict to update (user's existing config)
        source: The dict with new values (template config)
        keys_to_merge: List of top-level keys to merge. If None, merges all.
    """
    keys = source.keys() if keys_to_merge is None else keys_to_merge

    for key in keys:
        if key in source:
            source_value = source[key]
            target_value = target.get(key)
            if isinstance(target_value, dict) and isinstance(source_value, dict):
                deep_merge_json(target_value, source_value)
            else:
                target[key] = source_value


def backup_kicad_preferences(
    *,
    config_paths: list[Path] | None = None,
) -> list[Path] | None:
    """
    Backup current KiCad preferences before making changes.
    Creates timestamped backup in user's temp folder.
    """
    config_paths = list(config_paths) if config_paths is not None else KiCadEnvironment().find_config_paths()

    if len(config_paths) == 0:
        log.warning("No KiCad configuration paths found to backup")
        return None

    timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
    backup_base = Path(tempfile.gettempdir()) / "kicad_prefs_backup"
    backup_base.mkdir(exist_ok=True)

    backup_paths = []

    for kicad_config_loc in config_paths:
        version = kicad_config_loc.name  # e.g., "9.0"
        backup_path = backup_base / f"kicad_{version}_{timestamp}"

        try:
            shutil.copytree(kicad_config_loc, backup_path, dirs_exist_ok=False)
            backup_paths.append(backup_path)
            log.info(f"Backed up KiCad {version} preferences to: {backup_path}")
        except Exception as e:
            log.error(f"Failed to backup {kicad_config_loc}: {e}")

    return backup_paths


def _load_json_config(path: Path) -> dict:
    if path.exists():
        with open(path) as f:
            return json.load(f)
    return {}


def _write_json_config(path: Path, config: dict) -> None:
    with open(path, 'w') as f:
        json.dump(config, f, indent=2)


def _preference_file_names() -> tuple[str, ...]:
    return (
        "kicad_common.json",
        "eeschema.json",
        "pcbnew.json",
        "fpedit.json",
        "pl_editor.json",
        "gerbview.json",
    )


def _copy_kicad_colors(preferences_source: Path, kicad_config_loc: Path) -> None:
    colors_src = preferences_source / "colors"
    colors_dst = kicad_config_loc / "colors"
    try:
        if colors_src.exists():
            theme_names = sorted(path.stem for path in colors_src.glob("*.json"))
            theme_summary = ", ".join(theme_names) if theme_names else "no JSON themes"
            log.info(f"  Copying colors folder (themes: {theme_summary})...")
            shutil.copytree(colors_src, colors_dst, dirs_exist_ok=True)
            log.info("    Colors folder copied successfully")
        else:
            log.warning(f"    Colors folder not found at {colors_src}")
    except Exception as e:
        log.error(f"    Error copying colors: {e}")


def _summarize_preference_settings(config: dict) -> str:
    details: list[str] = []
    appearance = config.get("appearance")
    if isinstance(appearance, dict):
        for key in ("color_theme", "default_font"):
            if key in appearance:
                details.append(f"{key}={appearance[key]}")
    pcb_display = config.get("pcb_display")
    if isinstance(pcb_display, dict):
        for key in (
            "origin_invert_x_axis",
            "origin_invert_y_axis",
            "origin_mode",
        ):
            if key in pcb_display:
                details.append(f"{key}={pcb_display[key]}")
    return ", ".join(details)


def _update_preference_json(
    *,
    filename: str,
    preferences_source: Path,
    kicad_config_loc: Path,
) -> None:
    log.info(f"  Updating {filename}...")
    user_path = kicad_config_loc / filename
    template_path = preferences_source / filename
    try:
        if not template_path.exists():
            log.info(f"    Source file not found: {template_path}")
            return
        user_config = _load_json_config(user_path)
        template_config = _load_json_config(template_path)
        deep_merge_json(user_config, template_config)
        _write_json_config(user_path, user_config)
        setting_summary = _summarize_preference_settings(template_config)
        if setting_summary:
            log.info(f"    Applied source settings ({setting_summary})")
        log.info(f"    {filename} updated successfully")
    except Exception as e:
        log.error(f"    Error updating {filename}: {e}")


def _setup_kicad_config_preferences(
    *,
    preferences_source: Path,
    kicad_config_loc: Path,
) -> None:
    log.info("Setting up KiCad configuration at " + str(kicad_config_loc))
    _copy_kicad_colors(preferences_source, kicad_config_loc)
    for filename in _preference_file_names():
        _update_preference_json(
            filename=filename,
            preferences_source=preferences_source,
            kicad_config_loc=kicad_config_loc,
        )
    log.info(f"  KiCad preferences updated at {kicad_config_loc}")


def _backup_preferences_when_requested(
    *,
    enabled: bool,
    config_paths: list[Path],
) -> list[Path]:
    if not enabled:
        return []
    log.info("="*80)
    log.info("Backing up existing KiCad preferences...")
    log.info("="*80)
    backup_paths = backup_kicad_preferences(config_paths=config_paths) or []
    if backup_paths:
        log.info(f"Created {len(backup_paths)} backup(s)")
    log.info("")
    return backup_paths


def setup_kicad_preferences(
    with_backup: bool = True,
    user_preferences: Any | None = None,
    *,
    preferences_source: Path,
    config_paths: list[Path] | None = None,
) -> tuple[bool, list[Path]]:
    """
    Selectively update KiCad preferences instead of blanket copying.
    - Backups existing preferences (if with_backup=True)
    - Copies color themes from the supplied preferences source
    - Merges supported KiCad preference JSON files from the supplied source

    Args:
        with_backup: If True, creates backup before making changes
        user_preferences: Optional UserPreferences object to update version

    Returns:
        Tuple of (success: bool, backup_paths: list)
    """
    preferences_source = Path(preferences_source)
    config_paths = list(config_paths) if config_paths is not None else KiCadEnvironment().find_config_paths()

    if len(config_paths) == 0:
        log.error("No KiCad user configuration paths found")
        log.error("Make sure to run KiCad Once before running this script.")
        return False, []

    backup_paths = _backup_preferences_when_requested(
        enabled=with_backup,
        config_paths=config_paths,
    )

    for kicad_config_loc in config_paths:
        _setup_kicad_config_preferences(
            preferences_source=preferences_source,
            kicad_config_loc=kicad_config_loc,
        )

    log.info("")
    log.info("="*80)
    log.info("KiCad preferences setup completed successfully!")
    log.info("="*80)

    # Update user preferences version if provided
    if user_preferences:
        user_preferences.set("kicad_prefs_version", KICAD_PREFS_VERSION)
        log.info(f"Updated preferences version to {KICAD_PREFS_VERSION}")

    return True, backup_paths


def read_kicad_pro_parameters(filepath: Path) -> dict[str, str]:
    """
    Extract text_variables (project parameters) from a KiCad .kicad_pro file.

    Args:
        filepath: Path to .kicad_pro file

    Returns:
        Dictionary of parameter name -> value from text_variables section.
        Returns empty dict if file doesn't exist or has no text_variables.

    Example:
        >>> params = read_kicad_pro_parameters(Path("project.kicad_pro"))
        >>> log.info(params.get("CCA_CODENAME"))
        "my-board"
    """
    filepath = Path(filepath).resolve()

    if not filepath.exists():
        log.warning(f"KiCad project file not found: {filepath}")
        return {}

    try:
        with open(filepath, encoding='utf-8') as f:
            pro_data = json.load(f)

        # Extract text_variables dict (project parameters)
        text_vars = pro_data.get('text_variables', {})

        if not text_vars:
            log.warning(f"No text_variables found in {filepath.name}")
            return {}

        log.info(f"Loaded {len(text_vars)} parameters from {filepath.name}")
        return text_vars

    except json.JSONDecodeError as e:
        log.error(f"Failed to parse KiCad project file {filepath}: {e}")
        return {}
    except Exception as e:
        log.error(f"Error reading KiCad project file {filepath}: {e}")
        return {}
