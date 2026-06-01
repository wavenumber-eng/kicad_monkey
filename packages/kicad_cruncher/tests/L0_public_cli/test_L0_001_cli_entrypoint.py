"""Rack tests for the standalone public CLI package."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

import pytest
from colorama import Fore, Style
from kicad_cruncher._cli import _color_command_names_in_help, _format_parser_error_line
from kicad_cruncher._version import __version__, cli_version_text
from kicad_cruncher.kicad_cruncher_common import resolve_output_dir

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_MANIFEST_PATH = _PROJECT_ROOT / "docs" / "contracts" / "command_manifest.v0.json"


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    """Run the current checkout's CLI through the active Python environment."""
    return subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", *args],
        cwd=_PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _manifest_commands() -> list[str]:
    """Return public command names in manifest order."""
    manifest = json.loads(_MANIFEST_PATH.read_text(encoding="utf-8"))
    return [entry["name"] for entry in manifest["commands"]]


def test_cli_version_command() -> None:
    """Verify that the version subcommand reports the package version."""
    result = _run_cli("version")

    assert result.returncode == 0, result.stderr
    assert __version__ in result.stdout


def test_cli_no_args_prints_versioned_help() -> None:
    """Verify that a bare CLI invocation prints versioned command help."""
    result = _run_cli()

    assert result.returncode == 0, result.stderr
    assert cli_version_text() in result.stdout
    assert "usage: kicad-cruncher" in result.stdout
    assert "Run `kicad-cruncher <command> --help`" in result.stdout


def test_cli_help_lists_manifest_commands() -> None:
    """Verify that manifest commands are visible from root help output."""
    expected_commands = _manifest_commands()

    result = _run_cli("--help")

    assert result.returncode == 0, result.stderr
    assert cli_version_text() in result.stdout
    assert "usage: kicad-cruncher" in result.stdout
    assert "Run `kicad-cruncher <command> --help`" in result.stdout
    for command in expected_commands:
        assert command in result.stdout


def test_cli_help_lists_global_logging_controls() -> None:
    """Verify that root help documents global logging controls."""
    result = _run_cli("--help")

    assert result.returncode == 0, result.stderr
    assert "--quiet" in result.stdout
    assert "--verbose" in result.stdout
    assert "--log-level" in result.stdout


def test_design_help_describes_design_json_contents() -> None:
    """Verify design help explains the broader design JSON payload."""
    result = _run_cli("design", "--help")

    assert result.returncode == 0, result.stderr
    assert "project metadata" in result.stdout
    assert "schematic hierarchy" in result.stdout
    assert "components" in result.stdout
    assert "nets" in result.stdout


def test_pcb_svg_help_describes_config_and_hlr() -> None:
    """Verify pcb-svg help explains config-driven SVG/HLR behavior."""
    result = _run_cli("pcb-svg", "--help")

    assert result.returncode == 0, result.stderr
    assert "pcb.svg.config" in result.stdout
    assert "geometer-backed assembly HLR" in result.stdout
    assert ".kicad_pcb" in result.stdout


def test_cli_help_lists_commands_alphabetically() -> None:
    """Verify that root help presents commands in alphabetical order."""
    expected_commands = _manifest_commands()
    result = _run_cli("--help")
    command_pattern = re.compile(r"^    ([a-z0-9][a-z0-9-]*)(?:\s|$)")

    help_commands = [
        match.group(1)
        for line in result.stdout.splitlines()
        if (match := command_pattern.match(line)) is not None
        and match.group(1) in expected_commands
    ]

    assert result.returncode == 0, result.stderr
    assert help_commands == sorted(expected_commands)


def test_cli_help_colorizes_root_command_names_with_kicad_amber() -> None:
    """Verify terminal help highlights root command names in amber/yellow."""
    help_text = "\n".join(
        [
            "positional arguments:",
            "  <command>             Available commands",
            "    design              generate KiCad-native design JSON",
            "    pcb-svg             generate PCB SVG layer outputs",
            "    version             Print version information",
        ]
    )

    colored = _color_command_names_in_help(help_text, ("design", "pcb-svg", "version"))
    color = f"{Style.BRIGHT}{Fore.YELLOW}"

    assert f"    {color}design{Style.RESET_ALL}              generate" in colored
    assert f"    {color}pcb-svg{Style.RESET_ALL}             generate" in colored
    assert f"    {color}version{Style.RESET_ALL}             Print" in colored


def test_cli_parser_error_formatter_supports_red_terminal_output() -> None:
    """Verify parser errors can be highlighted in red for terminal output."""
    formatted = _format_parser_error_line(
        "kicad-cruncher",
        "invalid choice: 'netlist'",
        color=True,
    )

    assert formatted == (
        f"{Style.BRIGHT}{Fore.RED}"
        "kicad-cruncher: error: invalid choice: 'netlist'"
        f"{Style.RESET_ALL}\n"
    )


def test_cli_command_help_starts_for_manifest_commands() -> None:
    """Verify that each manifest command has command-level help."""
    expected_commands = _manifest_commands()

    for command in expected_commands:
        result = _run_cli(command, "--help")

        assert result.returncode == 0, f"{command}: {result.stderr}"
        assert cli_version_text() in result.stdout
        assert "usage:" in result.stdout
        assert command in result.stdout


def test_resolve_output_dir_defaults_to_command_subfolder(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Verify shared output policy defaults to ./output/<command>/."""
    monkeypatch.chdir(tmp_path)

    output_dir = resolve_output_dir(None, "design")
    assert output_dir == Path("output") / "design"
    assert (tmp_path / "output" / "design").is_dir()

    explicit_dir = tmp_path / "custom"
    resolved = resolve_output_dir(explicit_dir, "design")
    assert resolved == explicit_dir
    assert explicit_dir.is_dir()
