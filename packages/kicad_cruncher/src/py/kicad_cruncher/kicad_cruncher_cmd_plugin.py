"""plugin command for KiCad Cruncher."""

from __future__ import annotations

import argparse
from pathlib import Path

from kicad_cruncher.kicad_cruncher_plugin_installer import (
    DEFAULT_PLUGIN_NAME,
    api_report_lines,
    available_plugin_names,
    configure_api,
    discover_plugin_targets,
    inspect_api_config,
    install_plugin,
    result_lines,
    uninstall_plugin,
)


def cmd_plugin(args: argparse.Namespace) -> int:
    """Run one plugin management subcommand."""
    action = str(getattr(args, "plugin_action", ""))
    if action == "list-targets":
        return _cmd_list_targets(args)
    if action == "status":
        return _cmd_status(args)
    if action == "install":
        return _cmd_install(args)
    if action == "uninstall":
        return _cmd_uninstall(args)
    print("plugin subcommand required")
    return 2


def _cmd_list_targets(args: argparse.Namespace) -> int:
    targets = discover_plugin_targets(
        plugins_dir=getattr(args, "plugins_dir", None),
        kicad_version=getattr(args, "kicad_version", None),
        create_default=bool(getattr(args, "create_default", False)),
    )
    if not targets:
        print("No KiCad plugin targets discovered.")
        return 0
    for target in targets:
        version = target.version or "custom"
        print(f"{version}: {target.plugins_dir}")
    return 0


def _cmd_status(args: argparse.Namespace) -> int:
    plugin_name = str(getattr(args, "plugin_name", DEFAULT_PLUGIN_NAME))
    targets = discover_plugin_targets(
        plugins_dir=getattr(args, "plugins_dir", None),
        kicad_version=getattr(args, "kicad_version", None),
    )
    if not targets:
        print("No KiCad plugin targets discovered.")
        return 0
    marker = plugin_name.replace("-", "_")
    for target in targets:
        version = target.version or "custom"
        installed = any(target.plugins_dir.glob(f"*{marker}*"))
        print(f"{version}: {target.plugins_dir} installed={str(installed).lower()}")
        if target.version:
            for line in api_report_lines(inspect_api_config(target.version)):
                print(line)
    return 0


def _cmd_install(args: argparse.Namespace) -> int:
    plugin_name = str(getattr(args, "plugin_name", DEFAULT_PLUGIN_NAME))
    try:
        results = install_plugin(
            plugin_name,
            plugins_dir=getattr(args, "plugins_dir", None),
            kicad_version=getattr(args, "kicad_version", None),
            dry_run=bool(getattr(args, "dry_run", False)),
            create_default=bool(getattr(args, "create_default", False)),
        )
    except FileNotFoundError as exc:
        if bool(getattr(args, "best_effort", False)):
            print(f"Skipping KiCad plugin install: {exc}")
            return 0
        print(f"error: {exc}")
        return 1
    verb = "Would install" if bool(getattr(args, "dry_run", False)) else "Installed"
    for line in result_lines(results, verb=verb):
        print(line)
    _configure_api_for_results(args, [result.version for result in results])
    return 0


def _cmd_uninstall(args: argparse.Namespace) -> int:
    plugin_name = str(getattr(args, "plugin_name", DEFAULT_PLUGIN_NAME))
    results = uninstall_plugin(
        plugin_name,
        plugins_dir=getattr(args, "plugins_dir", None),
        kicad_version=getattr(args, "kicad_version", None),
        dry_run=bool(getattr(args, "dry_run", False)),
    )
    verb = "Would uninstall" if bool(getattr(args, "dry_run", False)) else "Uninstalled"
    for line in result_lines(results, verb=verb):
        print(line)
    return 0


def _configure_api_for_results(args: argparse.Namespace, versions: list[str | None]) -> None:
    setup_api = bool(getattr(args, "enable_api", False)) and not bool(
        getattr(args, "skip_api_setup", False)
    )
    python_interpreter = getattr(args, "python_interpreter", None)
    if not setup_api and python_interpreter is None:
        return
    for version in _unique_versions(versions):
        changes = configure_api(
            version,
            enable_api=setup_api,
            python_interpreter=python_interpreter,
            dry_run=bool(getattr(args, "dry_run", False)),
        )
        for change in changes:
            action = "Would update" if bool(getattr(args, "dry_run", False)) else "Updated"
            print(f"{action} KiCad {version} config: {change}")
        for line in api_report_lines(inspect_api_config(version)):
            print(line)


def _unique_versions(versions: list[str | None]) -> list[str]:
    unique: list[str] = []
    seen: set[str] = set()
    for version in versions:
        if version is None or version in seen:
            continue
        seen.add(version)
        unique.append(version)
    return unique


def _path_arg(value: str) -> Path:
    return Path(value).expanduser()


def register_parser(
    subparsers: argparse._SubParsersAction[argparse.ArgumentParser],
) -> argparse.ArgumentParser:
    """Register the plugin command parser."""
    parser = subparsers.add_parser("plugin", help="Install and inspect KiCad IPC plugins")
    plugin_subparsers = parser.add_subparsers(dest="plugin_action", metavar="<plugin-action>")

    list_parser = plugin_subparsers.add_parser("list-targets", help="List KiCad plugin targets")
    _add_target_arguments(list_parser)
    list_parser.add_argument("--create-default", action="store_true")
    list_parser.set_defaults(handler=cmd_plugin)

    status_parser = plugin_subparsers.add_parser("status", help="Report plugin install status")
    _add_plugin_name_argument(status_parser)
    _add_target_arguments(status_parser)
    status_parser.set_defaults(handler=cmd_plugin)

    install_parser = plugin_subparsers.add_parser("install", help="Install a KiCad IPC plugin")
    _add_plugin_name_argument(install_parser)
    _add_target_arguments(install_parser)
    install_parser.add_argument("--dry-run", action="store_true")
    install_parser.add_argument("--create-default", action="store_true")
    install_parser.add_argument("--best-effort", action="store_true")
    install_parser.add_argument("--enable-api", action="store_true")
    install_parser.add_argument("--skip-api-setup", action="store_true")
    install_parser.add_argument("--python-interpreter", type=_path_arg)
    install_parser.set_defaults(handler=cmd_plugin)

    uninstall_parser = plugin_subparsers.add_parser(
        "uninstall", help="Remove a KiCad IPC plugin"
    )
    _add_plugin_name_argument(uninstall_parser)
    _add_target_arguments(uninstall_parser)
    uninstall_parser.add_argument("--dry-run", action="store_true")
    uninstall_parser.set_defaults(handler=cmd_plugin)

    parser.set_defaults(handler=cmd_plugin)
    return parser


def _add_plugin_name_argument(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "plugin_name",
        nargs="?",
        default=DEFAULT_PLUGIN_NAME,
        choices=available_plugin_names(),
        help="Built-in plugin package name",
    )


def _add_target_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--plugins-dir", type=_path_arg)
    parser.add_argument("--kicad-version")
