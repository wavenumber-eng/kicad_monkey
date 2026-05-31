"""Config contract signoff tests for public CLI commands."""

from __future__ import annotations

import json
from html.parser import HTMLParser
from pathlib import Path


def _project_root() -> Path:
    """Find the repository root from this test file."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "pyproject.toml").exists():
            return parent
    raise RuntimeError("Could not locate repository root")


PACKAGE_ROOT = _project_root()
CONTRACTS_ROOT = PACKAGE_ROOT / "docs" / "contracts"
DESIGN_ROOT = PACKAGE_ROOT / "docs" / "design"
CLI_DESIGN_ROOT = DESIGN_ROOT / "cli"
COMMAND_MANIFEST = CONTRACTS_ROOT / "command_manifest.v0.json"


class _DataAttrParser(HTMLParser):
    """Collect HTML elements carrying data attributes."""

    def __init__(self) -> None:
        super().__init__()
        self.elements: list[tuple[str, dict[str, str]]] = []

    def handle_starttag(
        self,
        tag: str,
        attrs: list[tuple[str, str | None]],
    ) -> None:
        data = {key: value or "" for key, value in attrs if key.startswith("data-")}
        if data:
            self.elements.append((tag, data))


def _manifest_commands() -> list[str]:
    payload = json.loads(COMMAND_MANIFEST.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_cruncher.command_manifest.v0"
    commands = payload["commands"]
    assert isinstance(commands, list)
    return [str(command["name"]) for command in commands]


def _data_elements(path: Path) -> list[tuple[str, dict[str, str]]]:
    parser = _DataAttrParser()
    parser.feed(path.read_text(encoding="utf-8"))
    return parser.elements


def _cli_index_contracts() -> dict[str, str]:
    rows: dict[str, str] = {}
    for tag, attrs in _data_elements(CLI_DESIGN_ROOT / "index.html"):
        if tag == "tr" and "data-command" in attrs:
            rows[attrs["data-command"]] = attrs.get("data-config-contract", "")
    return rows


def _cli_doc_body_attrs(command: str) -> dict[str, str]:
    doc_path = CLI_DESIGN_ROOT / f"{command}.html"
    for tag, attrs in _data_elements(doc_path):
        if tag == "body":
            return attrs
    raise AssertionError(f"{doc_path}: missing body element with data attributes")


def test_cli_config_contract_links_are_release_ready() -> None:
    """Every public CLI design doc must resolve its declared config contract."""
    index_contracts = _cli_index_contracts()
    failures: list[str] = []

    for command in _manifest_commands():
        index_contract = index_contracts.get(command)
        body_attrs = _cli_doc_body_attrs(command)
        doc_contract = body_attrs.get("data-config-contract", "")
        if not index_contract:
            failures.append(f"{command}: missing CLI index config contract")
            continue
        if index_contract != doc_contract:
            failures.append(
                f"{command}: CLI index contract {index_contract!r} does not "
                f"match design doc contract {doc_contract!r}"
            )

        if doc_contract == "pending":
            failures.append(f"{command}: config contract is still pending")
            continue
        if doc_contract == "none":
            continue
        if not doc_contract.startswith("docs/contracts/"):
            failures.append(
                f"{command}: config contract must live under docs/contracts"
            )
            continue
        if not doc_contract.endswith(".schema.json"):
            failures.append(f"{command}: config contract is not a JSON schema")
            continue
        contract_path = PACKAGE_ROOT / doc_contract
        if not contract_path.exists():
            failures.append(f"{command}: missing config contract {doc_contract}")

    pending_docs = [
        path.relative_to(PACKAGE_ROOT).as_posix()
        for path in DESIGN_ROOT.rglob("*.html")
        if 'data-config-contract="pending"' in path.read_text(encoding="utf-8")
    ]
    failures.extend(f"{path}: contains pending config contract" for path in pending_docs)

    assert failures == [], "Config contract link signoff gaps:\n" + "\n".join(
        failures
    )

