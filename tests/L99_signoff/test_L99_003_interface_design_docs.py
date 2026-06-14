"""Interface design documentation signoff for promoted public API classes."""

from __future__ import annotations

import inspect
import json
import re
from dataclasses import dataclass
from pathlib import Path

from kicad_monkey.kicad_api_contract import (
    PUBLIC_API_MARKER_ROOT_NAMES,
    iter_public_api_exports,
    resolve_public_api_root,
)


def _project_root() -> Path:
    """Find the repository root from this test file."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "pyproject.toml").exists():
            return parent
    raise RuntimeError("Could not locate repository root")


@dataclass(frozen=True)
class InterfaceDoc:
    """Machine-readable interface documentation metadata."""

    name: str
    doc_path: Path
    section_text: str
    rack_stratum: str
    test_file: Path
    test_target: str


PACKAGE_ROOT = _project_root()
DESIGN_ROOT = PACKAGE_ROOT / "docs" / "design"
INTERFACE_MANIFEST = (
    PACKAGE_ROOT / "docs" / "contracts" / "interface_design_manifest.v0.json"
)
DESIGN_ENTRYPOINTS = (
    DESIGN_ROOT / "README.md",
    DESIGN_ROOT / "index.html",
    DESIGN_ROOT / "styles.css",
    DESIGN_ROOT / "api" / "index.html",
    PACKAGE_ROOT / "docs" / "contracts" / "README.md",
)


def _major_interfaces() -> set[str]:
    """Return explicitly listed major interfaces that require design docs."""
    payload = json.loads(INTERFACE_MANIFEST.read_text(encoding="utf-8"))

    assert payload["schema"] == "kicad_monkey.interface_design_manifest.v0"
    entries = payload["major_interfaces"]
    assert isinstance(entries, list)

    names: set[str] = set()
    for entry in entries:
        assert isinstance(entry, dict)
        name = entry["name"]
        group = entry["group"]
        assert isinstance(name, str)
        assert isinstance(group, str)
        assert group in {"foundation", "schematic", "pcb", "project"}
        names.add(name)
    return names


def _promoted_public_classes() -> set[str]:
    """Return promoted package-root exports that resolve to classes."""
    names: set[str] = set()
    for export in iter_public_api_exports():
        obj = resolve_public_api_root(export.name)
        if inspect.isclass(obj):
            names.add(export.name)
    return names


def _required_interfaces() -> set[str]:
    """Return the full public class/interface documentation set."""
    return _promoted_public_classes() | _major_interfaces()


def _interface_docs() -> dict[str, InterfaceDoc]:
    """Collect interface design-doc sections from HTML design docs."""
    docs: dict[str, InterfaceDoc] = {}
    section_pattern = re.compile(
        r"<section\b(?P<attrs>[^>]*)data-interface=\"(?P<name>[^\"]+)\""
        r"(?P<attrs2>[^>]*)>(?P<body>.*?)</section>",
        re.DOTALL,
    )
    attr_pattern = re.compile(r"(?P<name>data-[a-z-]+)=\"(?P<value>[^\"]+)\"")

    for doc_path in DESIGN_ROOT.rglob("*.html"):
        text = doc_path.read_text(encoding="utf-8")
        for match in section_pattern.finditer(text):
            attrs = dict(attr_pattern.findall(match.group("attrs") + match.group("attrs2")))
            name = match.group("name")
            docs[name] = InterfaceDoc(
                name=name,
                doc_path=doc_path,
                section_text=match.group("body"),
                rack_stratum=attrs.get("data-rack-stratum", ""),
                test_file=PACKAGE_ROOT / attrs.get("data-test-file", ""),
                test_target=attrs.get("data-test-target", ""),
            )
    return docs


def test_design_doc_entrypoints_exist() -> None:
    """Verify design docs have stable human and machine entry points."""
    missing = [str(path.relative_to(PACKAGE_ROOT)) for path in DESIGN_ENTRYPOINTS if not path.exists()]

    assert missing == []


def test_design_html_docs_declare_status() -> None:
    """Verify HTML design docs declare a standards-compatible status."""
    status_pattern = re.compile(r"\bdata-doc-status=\"(?P<status>[^\"]+)\"")
    allowed = {"draft", "proposal", "accepted", "superseded"}
    failures: list[str] = []

    for doc_path in sorted(DESIGN_ROOT.rglob("*.html")):
        text = doc_path.read_text(encoding="utf-8")
        match = status_pattern.search(text)
        relpath = doc_path.relative_to(PACKAGE_ROOT)
        if match is None:
            failures.append(f"{relpath}: missing data-doc-status")
            continue
        status = match.group("status")
        if status not in allowed:
            failures.append(f"{relpath}: invalid data-doc-status {status!r}")

    assert failures == [], "Design doc status gaps:\n" + "\n".join(failures)


def test_manifest_covers_promoted_public_facade_roots() -> None:
    """Verify every marker-promoted facade root is documented by the manifest."""
    required = set(PUBLIC_API_MARKER_ROOT_NAMES)
    manifest_names = _major_interfaces()

    assert required <= manifest_names


def test_promoted_public_classes_and_major_interfaces_have_design_docs() -> None:
    """Verify promoted public classes and major interfaces have design docs."""
    required = _required_interfaces()
    docs = _interface_docs()

    missing = sorted(required - set(docs))
    assert missing == [], "Missing interface design docs:\n" + "\n".join(missing)


def test_interface_design_docs_define_rationale_tests_and_working_state() -> None:
    """Verify each interface doc records design and test expectations."""
    docs = _interface_docs()
    failures: list[str] = []

    for name in sorted(_required_interfaces() & set(docs)):
        doc = docs[name]
        for required_text in (
            "Rationale",
            "Purpose",
            "Test Requirements",
            "Working Definition",
        ):
            if required_text not in doc.section_text:
                failures.append(f"{name}: missing {required_text} in {doc.doc_path}")

    assert failures == [], "Interface design content gaps:\n" + "\n".join(failures)


def test_interface_design_docs_point_to_rack_exercising_tests() -> None:
    """Verify each interface doc points to an exercising Rack test target."""
    docs = _interface_docs()
    failures: list[str] = []

    for name in sorted(_required_interfaces() & set(docs)):
        doc = docs[name]
        stratum = PACKAGE_ROOT / "tests" / doc.rack_stratum / "STRATUM.toml"
        if not stratum.exists():
            failures.append(f"{name}: missing Rack stratum {doc.rack_stratum}")
        if not doc.test_file.exists():
            failures.append(f"{name}: missing test file {doc.test_file}")
            continue

        test_text = doc.test_file.read_text(encoding="utf-8")
        if doc.test_target not in test_text:
            failures.append(f"{name}: test target not found: {doc.test_target}")

    assert failures == [], "Interface test ownership gaps:\n" + "\n".join(failures)
