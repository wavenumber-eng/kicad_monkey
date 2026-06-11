"""Config contract signoff tests for public CLI commands."""

from __future__ import annotations

import json
from html.parser import HTMLParser
from pathlib import Path

from kicad_cruncher.bom_pnp_model import BOM_PNP_CONFIG_SCHEMA, default_bom_pnp_config_text
from kicad_cruncher.config_json import load_json_config
from kicad_cruncher.kicad_cruncher_pcb_clean import (
    PCB_CLEAN_CONFIG_SCHEMA,
    _default_pcb_clean_config_text,
)
from kicad_cruncher.kicad_cruncher_pcb_layer_step import PcbLayerStepConfig
from kicad_cruncher.kicad_cruncher_pcb_layer_step_config import (
    PCB_LAYER_STEP_CONFIG_SCHEMA_V2,
    pcb_layer_step_default_config_text,
)
from kicad_cruncher.kicad_cruncher_pcb_svg_config import (
    PCB_SVG_CONFIG_SCHEMA,
    pcb_svg_default_config_text,
)


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


def test_pcb_layer_step_default_jsonc_documents_enum_options(tmp_path: Path) -> None:
    """The generated pcb-layer-step config must be documented JSONC, not a bare JSON dump."""
    text = pcb_layer_step_default_config_text()
    required_comments = [
        "Component pad mode. Options: none, all, matching_designators.",
        "Global drill mode. Options: auto, cut, overlay, none.",
        "Drill mode for pads selected by component_pads. Options: inherit, cut, overlay, none.",
        "Overlay shape. Options: solid, ring.",
        "Plated drill ring policy. Options: annulus, pad.",
        "Stable Geometer STEP body id/name.",
        "Symmetric Z bias that prevents overlapping colored bodies from z-fighting.",
    ]
    missing = [comment for comment in required_comments if comment not in text]
    assert missing == []

    config_path = tmp_path / "pcb-layer-step.jsonc"
    config_path.write_text(text, encoding="utf-8")
    payload = load_json_config(config_path)
    assert payload["schema"] == PCB_LAYER_STEP_CONFIG_SCHEMA_V2
    config = PcbLayerStepConfig.from_dict(payload)
    assert config.outputs
    assert config.outputs[0].include_designators == ("TP*", "M*")
    assert config.outputs[0].pad_color_rules[0].step_body_name == "test_points"


def test_pcb_layer_step_v2_contract_removed_old_color_body_fields() -> None:
    """The v2 contract must not advertise removed colors/body config fields."""
    schema_text = (CONTRACTS_ROOT / "pcb_layer_step_config.v2.schema.json").read_text(
        encoding="utf-8"
    )
    assert '"colors"' not in schema_text
    assert '"body"' not in schema_text
    assert '"step_body_name"' in schema_text


def test_all_default_configs_are_generated_documented_jsonc(tmp_path: Path) -> None:
    """Every generated command config must parse and carry structured field comments."""
    configs = {
        "bom.config": (
            default_bom_pnp_config_text(),
            BOM_PNP_CONFIG_SCHEMA,
            [
                "Variant mode Options: base, all, named.",
                "BOM artifact kinds to emit Options:",
                "PnP coordinate units; JLC CPL requires mm Options: mm, mils.",
            ],
        ),
        "pcb.svg.config": (
            pcb_svg_default_config_text(),
            PCB_SVG_CONFIG_SCHEMA,
            [
                "Canvas bounds mode Options: board_outline, all_geometry.",
                "Default component projection mode Options: detail, outline, bounding_box",
                "View projection mode for ASSEMBLY_HLR_TOP/BOTTOM tokens",
            ],
        ),
        "pcb.clean.config": (
            _default_pcb_clean_config_text(),
            PCB_CLEAN_CONFIG_SCHEMA,
            [
                "Cleanup target groups. Set a group false to keep that class untouched.",
                "Require explicit --apply for file mutation",
                "Layer selection globs used by cleanup targets.",
            ],
        ),
        "pcb-layer-step.jsonc": (
            pcb_layer_step_default_config_text(),
            PCB_LAYER_STEP_CONFIG_SCHEMA_V2,
            [
                "Component pad mode. Options: none, all, matching_designators.",
                "Global drill mode. Options: auto, cut, overlay, none.",
                "Stable Geometer STEP body id/name.",
            ],
        ),
    }

    for file_name, (text, schema, required_comments) in configs.items():
        config_path = tmp_path / file_name
        config_path.write_text(text, encoding="utf-8")
        payload = load_json_config(config_path)
        assert payload["schema"] == schema
        missing = [comment for comment in required_comments if comment not in text]
        assert missing == [], f"{file_name} missing comments: {missing}"
