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
from kicad_cruncher.kicad_cruncher_cmd_daemon import (
    daemon_host_allowed,
    daemon_host_is_loopback,
)
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
    """Verify design help explains the broader design review payload."""
    result = _run_cli("design", "--help")

    assert result.returncode == 0, result.stderr
    assert "design review bundle" in result.stdout
    assert "enriched black-and-white schematic SVGs" in result.stdout
    assert "enriched PCB copper-layer SVGs" in result.stdout
    assert "project metadata" in result.stdout
    assert "schematic hierarchy" in result.stdout
    assert "components" in result.stdout
    assert "nets" in result.stdout


@pytest.mark.parametrize("alias", ("design-review", "dr"))
def test_design_alias_help_starts(alias: str) -> None:
    """Verify the public design review aliases start and print help."""
    result = _run_cli(alias, "--help")

    assert result.returncode == 0, result.stderr
    assert "usage:" in result.stdout
    assert "design review bundle" in result.stdout


def test_pcb_svg_help_describes_config_and_hlr() -> None:
    """Verify pcb-svg help explains config-driven SVG/HLR behavior."""
    result = _run_cli("pcb-svg", "--help")

    assert result.returncode == 0, result.stderr
    assert "pcb.svg.config" in result.stdout
    assert "geometer-backed assembly HLR" in result.stdout
    assert ".kicad_pcb" in result.stdout


def test_pcb_layer_step_help_describes_fixture_step_config() -> None:
    """Verify pcb-layer-step help explains fixture-alignment STEP behavior."""
    result = _run_cli("pcb-layer-step", "--help")

    assert result.returncode == 0, result.stderr
    assert "fixture-alignment model" in result.stdout
    assert "pcb-layer-step.json" in result.stdout
    assert ".kicad_pcb" in result.stdout


def test_plugin_help_describes_install_flow() -> None:
    """Verify plugin help exposes the KiCad IPC install surface."""
    result = _run_cli("plugin", "install", "--help")

    assert result.returncode == 0, result.stderr
    assert "--enable-api" in result.stdout
    assert "--plugins-dir" in result.stdout
    assert "kicad-cruncher-tools" in result.stdout


def test_daemon_health_outputs_json() -> None:
    """Verify daemon health can be checked without starting a long-running server."""
    result = _run_cli("daemon", "--health")

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["schema"] == "kicad_cruncher.daemon.health.v0"
    assert payload["ok"] is True
    assert payload["service"] == "kicad-cruncher"


def test_daemon_help_describes_remote_host_opt_in() -> None:
    """Verify daemon help exposes the explicit remote-bind opt in."""
    result = _run_cli("daemon", "--help")

    assert result.returncode == 0, result.stderr
    assert "--allow-remote-host" in result.stdout


def test_daemon_host_policy_requires_remote_opt_in() -> None:
    """Verify daemon remote binding is explicit instead of accidental."""
    assert daemon_host_is_loopback("127.0.0.1") is True
    assert daemon_host_is_loopback("localhost") is True
    assert daemon_host_is_loopback("::1") is True
    assert daemon_host_is_loopback("0.0.0.0") is False
    assert daemon_host_allowed("0.0.0.0", allow_remote=False) is False
    assert daemon_host_allowed("0.0.0.0", allow_remote=True) is True


def test_pcb_clean_writes_config_and_dry_run_plan(tmp_path: Path) -> None:
    """Verify PCB clean has a CLI/config path before plugin UI work lands."""
    config_path = tmp_path / "pcb.clean.config"
    write_result = _run_cli("pcb", "clean", "--write-config", str(config_path))

    assert write_result.returncode == 0, write_result.stderr
    assert config_path.is_file()
    assert "kicad_cruncher.pcb.clean.config.v0" in config_path.read_text(encoding="utf-8")

    dry_run = _run_cli("pcb", "clean", "board.kicad_pcb", "--config", str(config_path), "--dry-run")
    assert dry_run.returncode == 0, dry_run.stderr
    payload = json.loads(dry_run.stdout)
    assert payload["schema"] == "kicad_cruncher.pcb.clean.plan.v0"
    assert payload["dry_run"] is True
    assert payload["mutation_supported"] is True
    assert payload["apply_policy"]["silkscreen"] == (
        "opt-in through layers.include; not selected by default"
    )


def test_plugin_install_dry_run_accepts_explicit_target(tmp_path: Path) -> None:
    """Verify plugin installer can dry-run against an explicit KiCad plugin folder."""
    plugins_dir = tmp_path / "KiCad" / "10.0" / "plugins"
    result = _run_cli(
        "plugin",
        "install",
        "--plugins-dir",
        str(plugins_dir),
        "--dry-run",
        "--enable-api",
    )

    assert result.returncode == 0, result.stderr
    assert "Would install kicad-cruncher-tools" in result.stdout
    assert "com.wavenumber.kicad-cruncher.tools" in result.stdout
    assert not plugins_dir.exists()


def test_plugin_management_commands_use_manifest_identifier(tmp_path: Path) -> None:
    """Verify plugin list/status/install/uninstall share the manifest identifier."""
    plugins_dir = tmp_path / "KiCad" / "10.0" / "plugins"

    list_result = _run_cli("plugin", "list-targets", "--plugins-dir", str(plugins_dir))
    before_status = _run_cli("plugin", "status", "--plugins-dir", str(plugins_dir))
    install_result = _run_cli("plugin", "install", "--plugins-dir", str(plugins_dir))
    after_status = _run_cli("plugin", "status", "--plugins-dir", str(plugins_dir))
    dry_uninstall = _run_cli("plugin", "uninstall", "--plugins-dir", str(plugins_dir), "--dry-run")
    uninstall_result = _run_cli("plugin", "uninstall", "--plugins-dir", str(plugins_dir))

    target_dir = plugins_dir / "com.wavenumber.kicad-cruncher.tools"
    assert list_result.returncode == 0, list_result.stderr
    assert f"custom: {plugins_dir}" in list_result.stdout
    assert before_status.returncode == 0, before_status.stderr
    assert "installed=false" in before_status.stdout
    assert install_result.returncode == 0, install_result.stderr
    assert "Installed kicad-cruncher-tools" in install_result.stdout
    assert after_status.returncode == 0, after_status.stderr
    assert "installed=true" in after_status.stdout
    assert dry_uninstall.returncode == 0, dry_uninstall.stderr
    assert "Would uninstall kicad-cruncher-tools" in dry_uninstall.stdout
    assert uninstall_result.returncode == 0, uninstall_result.stderr
    assert not target_dir.exists()


def test_schematic_clean_is_deferred_json() -> None:
    """Verify the schematic clean command group exists without mutation behavior."""
    result = _run_cli("schematic", "clean")

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["schema"] == "kicad_cruncher.schematic.clean.plan.a0"
    assert payload["status"] == "deferred"


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
            "    design (design-review, dr)",
            "                        generate KiCad design review artifacts",
            "    pcb-layer-step      generate a colored STEP model",
            "    pcb-svg             generate PCB SVG layer outputs",
            "    version             Print version information",
        ]
    )

    colored = _color_command_names_in_help(
        help_text,
        ("design", "pcb-layer-step", "pcb-svg", "version"),
    )
    color = f"{Style.BRIGHT}{Fore.YELLOW}"

    assert f"    {color}design{Style.RESET_ALL} (design-review, dr)" in colored
    assert f"    {color}pcb-layer-step{Style.RESET_ALL}      generate" in colored
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
