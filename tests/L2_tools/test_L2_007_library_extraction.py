"""Subtest: KiCad Library Extraction
Stratum: L2_tools
Purpose: Extract project-local KiCad library artifacts for megamaid workflows

These tests cover the non-destructive primitives used by higher-level workflow
tools to inspect and extract symbols, footprints, metadata, and embedded STEP
models from KiCad projects.
"""

from __future__ import annotations

from collections import Counter
import json
import shutil
from pathlib import Path

import pytest

from conftest import STEP_MODEL_EXTRACT_DIR
from kicad_monkey.kicad_footprint import KiCadFootprint
from kicad_monkey.kicad_library_extraction import (
    KiCadExtractionMode,
    KiCadModelReferenceKind,
    embed_external_model_payloads,
    extract_3d_models,
    extract_footprints,
    extract_symbols,
    resolve_kicad_cli,
    scan_project_assets,
    validate_pretty_library_with_kicad_cli,
    validate_symbol_library_with_kicad_cli,
    write_extraction_metadata_bundle,
    write_pretty_library,
    write_symbol_folder_library,
)
from kicad_monkey.kicad_model import Model
from kicad_monkey.kicad_symbol_lib import KiCadSymbolLib
from kicad_monkey.testing.corpus import get_kicad_corpus_case, resolve_kicad_manifest_path


def _four_ch_backplane_project() -> Path:
    case = get_kicad_corpus_case("real_world/4-ch-backplane")
    assert case is not None
    project_path = resolve_kicad_manifest_path(case, "project_file")
    assert project_path is not None
    return project_path


def test_extract_3d_models_decodes_embedded_step_payloads(tmp_path: Path) -> None:
    """Embedded KiCad model payloads must be written as STEP bytes, not base64 text."""
    pytest.importorskip("zstandard")

    project_path = tmp_path / "embedded-step.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    shutil.copyfile(
        STEP_MODEL_EXTRACT_DIR / "C0201_0.13MM_HD.kicad_mod",
        tmp_path / "C0201_0.13MM_HD.kicad_mod",
    )

    written = extract_3d_models(project_path, tmp_path / "models")

    assert len(written) == 1
    assert written[0].suffix.lower() == ".step"
    assert "ISO-10303-21" in written[0].read_text(encoding="utf-8", errors="ignore")


def test_embed_external_model_payloads_rewrites_resolvable_step_refs(tmp_path: Path) -> None:
    """Resolvable external models should be embedded into extracted footprints."""
    pytest.importorskip("zstandard")

    model_root = tmp_path / "models"
    package_dir = model_root / "Package_Custom.3dshapes"
    package_dir.mkdir(parents=True)
    step_file = package_dir / "custom.step"
    step_file.write_text("ISO-10303-21;\nEND-ISO-10303-21;\n", encoding="utf-8")

    footprint = KiCadFootprint()
    footprint.name = "ExternalModel"
    footprint.models = [Model("${KICAD10_3DMODEL_DIR}/Package_Custom.3dshapes/custom.step")]

    embedded = embed_external_model_payloads(
        footprint,
        project_root=tmp_path,
        env={"KICAD10_3DMODEL_DIR": str(model_root)},
    )

    assert embedded.models[0].path == "kicad-embed://custom.step"
    assert len(embedded.embedded_files) == 1
    assert embedded.embedded_files[0].file_type == "model"

    project_path = tmp_path / "external-model.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    embedded.save(tmp_path / "ExternalModel.kicad_mod")

    written = extract_3d_models(project_path, tmp_path / "extracted")
    assert len(written) == 1
    assert "ISO-10303-21" in written[0].read_text(encoding="utf-8", errors="ignore")


@pytest.mark.slow
def test_scan_project_assets_classifies_4ch_backplane_models() -> None:
    """The 4-ch backplane fixture exposes embedded and external KiCad model refs."""
    scan = scan_project_assets(_four_ch_backplane_project())

    assert Path(scan.project_path).name == "4-ch-backplane.kicad_pro"
    assert len(scan.pcbs) == 1
    assert len(scan.schematics) >= 18
    assert len(scan.symbol_libraries) >= 9
    assert len(scan.pretty_libraries) >= 2
    assert len(scan.footprint_files) >= 40

    kind_counts = Counter(ref.reference_kind for ref in scan.model_references)
    assert kind_counts[KiCadModelReferenceKind.EMBEDDED.value] >= 700
    assert kind_counts[KiCadModelReferenceKind.KICAD_ENV.value] >= 20
    assert any(ref.payload_scope == "board" and ref.has_embedded_payload for ref in scan.model_references)
    assert any(ref.payload_scope == "footprint" and ref.has_embedded_payload for ref in scan.model_references)
    assert scan.to_dict()["model_references"]


@pytest.mark.slow
def test_extract_4ch_backplane_embedded_step_models(tmp_path: Path) -> None:
    """The real-world fixture includes zstd frames without content-size headers."""
    pytest.importorskip("zstandard")

    written = extract_3d_models(_four_ch_backplane_project(), tmp_path / "models")

    assert len(written) >= 20
    assert len(written) == len(list((tmp_path / "models").iterdir()))
    assert all(path.suffix.lower() in {".step", ".stp"} for path in written)
    assert any(
        "ISO-10303-21" in path.read_text(encoding="utf-8", errors="ignore")
        for path in written
    )


@pytest.mark.slow
def test_extract_4ch_backplane_libraries_writes_valid_stripped_assets(tmp_path: Path) -> None:
    """Internal extraction dedupes library assets and strips instance metadata."""
    project_path = _four_ch_backplane_project()

    symbol_records = extract_symbols(project_path)
    footprint_records = extract_footprints(project_path)

    assert len(symbol_records) >= 70
    assert len(footprint_records) >= 40
    assert any(record.raw_fields for record in symbol_records)
    assert any(record.raw_fields for record in footprint_records)
    assert any(
        ref.reference_kind == KiCadModelReferenceKind.EMBEDDED.value and ref.has_embedded_payload
        for record in footprint_records
        for ref in record.model_references
    )

    for record in symbol_records:
        stripped_keys = {str(getattr(prop, "key", "")) for prop in record.symbol.properties}
        assert stripped_keys <= {"Reference", "Value"}

    for record in footprint_records:
        assert ":" not in record.footprint.name

    sample_with_pads = next(record for record in footprint_records if record.footprint.pads)
    assert all(not pad.net for pad in sample_with_pads.footprint.pads)
    assert all(pad.uuid is None for pad in sample_with_pads.footprint.pads)

    written_symbols = write_symbol_folder_library(symbol_records, tmp_path / "symbols")
    written_footprints = write_pretty_library(footprint_records, tmp_path / "footprints.pretty")

    assert len(written_symbols) == len(symbol_records)
    assert len(written_footprints) == len(footprint_records)
    assert KiCadSymbolLib.from_file(written_symbols[0]).symbols
    assert KiCadFootprint.from_file(written_footprints[0]).name

    metadata_path = write_extraction_metadata_bundle(
        project_path,
        tmp_path / "metadata" / "library_extraction.json",
        symbol_records=symbol_records,
        footprint_records=footprint_records,
        include_asset_scan=False,
    )
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    assert metadata["schema"] == "kicad_monkey.library_extraction_bundle.v1"
    assert len(metadata["symbols"]) == len(symbol_records)
    assert len(metadata["footprints"]) == len(footprint_records)


@pytest.mark.slow
def test_project_local_extraction_preserves_instance_metadata() -> None:
    """Project-local mode keeps editable part variants instead of stripping to bare assets."""
    project_path = _four_ch_backplane_project()

    internal_symbols = extract_symbols(project_path)
    project_symbols = extract_symbols(project_path, KiCadExtractionMode.PROJECT_LOCAL)
    assert len(project_symbols) >= len(internal_symbols)
    assert any(len(record.symbol.properties) > 2 for record in project_symbols)

    internal_footprints = extract_footprints(project_path)
    project_footprints = extract_footprints(project_path, KiCadExtractionMode.PROJECT_LOCAL)
    assert len(project_footprints) > len(internal_footprints)

    sample = next(record.footprint for record in project_footprints if record.footprint.pads)
    assert any(pad.net for pad in sample.pads)
    assert any(pad.uuid is not None for pad in sample.pads)


def test_kicad_cli_validates_extracted_library_smoke(tmp_path: Path) -> None:
    """Generated symbol and footprint libraries are acceptable to KiCad CLI when installed."""
    cli = resolve_kicad_cli()
    if cli is None:
        pytest.skip("kicad-cli not found")

    symbol_records = extract_symbols(_four_ch_backplane_project())
    footprint_records = extract_footprints(_four_ch_backplane_project())
    written_symbols = write_symbol_folder_library(symbol_records[:1], tmp_path / "symbols")
    written_footprints = write_pretty_library(footprint_records[:1], tmp_path / "footprints.pretty")

    symbol_result = validate_symbol_library_with_kicad_cli(written_symbols[0], kicad_cli=cli)
    footprint_result = validate_pretty_library_with_kicad_cli(written_footprints[0].parent, kicad_cli=cli)

    assert symbol_result.ok, symbol_result.stderr
    assert footprint_result.ok, footprint_result.stderr
