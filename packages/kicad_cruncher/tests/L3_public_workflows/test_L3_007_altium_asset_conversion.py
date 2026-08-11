from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest
from kicad_cruncher import (
    AltiumAssetConversionExecutor,
    AltiumAssetConversionRequest,
    AltiumAssetKind,
    ConversionStage,
    ExistingDestinationPolicy,
)
from kicad_monkey import find_element, parse_sexp


class FakeKiCadRunner:
    def __init__(self, *, fail_validation: bool = False) -> None:
        self.fail_validation = fail_validation
        self.commands: list[list[str]] = []

    def __call__(self, command: list[str], **_: object) -> subprocess.CompletedProcess[str]:
        self.commands.append(command)
        if command[1:] == ["--version"]:
            return subprocess.CompletedProcess(command, 0, "10.0.5\n", "")

        output = Path(command[command.index("--output") + 1])
        source = Path(command[-1])
        validating = "--force" in command
        if validating and self.fail_validation:
            return subprocess.CompletedProcess(command, 7, "", "validation failed")

        if command[1:3] == ["fp", "upgrade"]:
            output.mkdir(parents=True, exist_ok=True)
            if source.suffix.lower() == ".pcblib":
                (output / "Alpha.kicad_mod").write_text(
                    '(footprint "Alpha" (pad "1" smd rect (size 1 1)))\n',
                    encoding="utf-8",
                )
                (output / "Target.kicad_mod").write_text(
                    '(footprint "Target" '
                    '(pad "1" np_thru_hole oval (size 0 3.45) '
                    '(drill oval 1.6 3.45) (layers "*.Cu" "*.Mask")))\n',
                    encoding="utf-8",
                )
            else:
                for item in source.glob("*.kicad_mod"):
                    shutil.copy2(item, output / item.name)
            return subprocess.CompletedProcess(command, 0, "", "")

        if command[1:3] == ["sym", "upgrade"]:
            output.parent.mkdir(parents=True, exist_ok=True)
            if source.suffix.lower() == ".schlib":
                output.write_text(
                    '(kicad_symbol_lib (symbol "Alpha") (symbol "Target"))\n',
                    encoding="utf-8",
                )
            else:
                shutil.copy2(source, output)
            return subprocess.CompletedProcess(command, 0, "", "")

        return subprocess.CompletedProcess(command, 2, "", "unexpected command")


class FailingFilterPipeline:
    def filter_footprint(self, _source: Path, _destination: Path) -> None:
        raise RuntimeError("filter failure")

    def filter_symbol(self, _source: Path, _destination: Path) -> None:
        raise RuntimeError("filter failure")


def _source(tmp_path: Path, suffix: str) -> Path:
    source = tmp_path / f"source{suffix}"
    source.write_bytes(b"synthetic public test fixture")
    return source


def test_footprint_conversion_selects_exact_key_and_always_normalizes(tmp_path: Path) -> None:
    runner = FakeKiCadRunner()
    destination = tmp_path / "out" / "Target.kicad_mod"
    request = AltiumAssetConversionRequest(
        source_library=_source(tmp_path, ".PcbLib"),
        source_key="Target",
        destination=destination,
        kind=AltiumAssetKind.FOOTPRINT,
        existing_policy=ExistingDestinationPolicy.REPLACE,
        run_cleanup_filters=False,
    )

    result = AltiumAssetConversionExecutor(
        kicad_cli=tmp_path / "kicad-cli.exe",
        runner=runner,
    ).convert(request)

    assert result.success is True
    assert result.emitted_key == "Target"
    assert result.normalization_count == 1
    footprint = parse_sexp(destination.read_text(encoding="utf-8"))
    assert tuple(find_element(find_element(footprint, "pad"), "size")[1:3]) == (
        0.001,
        0.001,
    )
    assert any(stage.stage is ConversionStage.FILTER for stage in result.stages)
    assert not list(destination.parent.glob(".kicad-convert-*"))


def test_missing_exact_footprint_key_preserves_existing_destination(tmp_path: Path) -> None:
    destination = tmp_path / "Target.kicad_mod"
    original = b"existing-good-output\n"
    destination.write_bytes(original)
    request = AltiumAssetConversionRequest(
        source_library=_source(tmp_path, ".PcbLib"),
        source_key="Missing",
        destination=destination,
        kind=AltiumAssetKind.FOOTPRINT,
        existing_policy=ExistingDestinationPolicy.REPLACE,
        run_cleanup_filters=False,
    )

    result = AltiumAssetConversionExecutor(
        kicad_cli=tmp_path / "kicad-cli.exe",
        runner=FakeKiCadRunner(),
    ).convert(request)

    assert result.success is False
    assert destination.read_bytes() == original
    assert any(stage.stage is ConversionStage.SELECT for stage in result.errors)


@pytest.mark.parametrize("failure", ["filter", "validation"])
def test_each_post_conversion_failure_preserves_existing_destination(
    tmp_path: Path,
    failure: str,
) -> None:
    destination = tmp_path / "Target.kicad_mod"
    original = b"existing-good-output\n"
    destination.write_bytes(original)
    runner = FakeKiCadRunner(fail_validation=failure == "validation")
    filters = FailingFilterPipeline() if failure == "filter" else None
    request = AltiumAssetConversionRequest(
        source_library=_source(tmp_path, ".PcbLib"),
        source_key="Target",
        destination=destination,
        kind=AltiumAssetKind.FOOTPRINT,
        existing_policy=ExistingDestinationPolicy.REPLACE,
        run_cleanup_filters=failure == "filter",
    )

    result = AltiumAssetConversionExecutor(
        kicad_cli=tmp_path / "kicad-cli.exe",
        runner=runner,
        filter_pipeline=filters,
    ).convert(request)

    assert result.success is False
    assert destination.read_bytes() == original


def test_reuse_rejects_invalid_existing_footprint_without_mutating_it(tmp_path: Path) -> None:
    destination = tmp_path / "Target.kicad_mod"
    original = b'(footprint "Target" (pad "1" thru_hole circle (size 0 0)))\n'
    destination.write_bytes(original)
    request = AltiumAssetConversionRequest(
        source_library=_source(tmp_path, ".PcbLib"),
        source_key="Target",
        destination=destination,
        kind=AltiumAssetKind.FOOTPRINT,
        existing_policy=ExistingDestinationPolicy.REUSE,
    )

    result = AltiumAssetConversionExecutor(
        kicad_cli=tmp_path / "kicad-cli.exe",
        runner=FakeKiCadRunner(),
    ).convert(request)

    assert result.success is False
    assert destination.read_bytes() == original
    assert any("requires mandatory normalization" in stage.message for stage in result.errors)


def test_publication_failure_preserves_existing_destination(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    destination = tmp_path / "Target.kicad_mod"
    original = b"existing-good-output\n"
    destination.write_bytes(original)

    def fail_replace(source: Path, target: Path) -> None:
        raise OSError(f"simulated publish failure for {target}")

    monkeypatch.setattr("kicad_cruncher.altium_asset_conversion.os.replace", fail_replace)
    request = AltiumAssetConversionRequest(
        source_library=_source(tmp_path, ".PcbLib"),
        source_key="Target",
        destination=destination,
        kind=AltiumAssetKind.FOOTPRINT,
        existing_policy=ExistingDestinationPolicy.REPLACE,
        run_cleanup_filters=False,
    )

    result = AltiumAssetConversionExecutor(
        kicad_cli=tmp_path / "kicad-cli.exe",
        runner=FakeKiCadRunner(),
    ).convert(request)

    assert result.success is False
    assert destination.read_bytes() == original
    assert result.errors[-1].stage is ConversionStage.PUBLISH


def test_symbol_conversion_requires_exact_key_and_reports_reusable_result(tmp_path: Path) -> None:
    destination = tmp_path / "symbols" / "source.kicad_sym"
    request = AltiumAssetConversionRequest(
        source_library=_source(tmp_path, ".SchLib"),
        source_key="Target",
        destination=destination,
        kind=AltiumAssetKind.SYMBOL,
        existing_policy=ExistingDestinationPolicy.REPLACE,
        run_cleanup_filters=False,
    )
    executor = AltiumAssetConversionExecutor(
        kicad_cli=tmp_path / "kicad-cli.exe",
        runner=FakeKiCadRunner(),
    )

    converted = executor.convert(request)
    reused = executor.convert(
        AltiumAssetConversionRequest(
            source_library=request.source_library,
            source_key=request.source_key,
            destination=request.destination,
            kind=request.kind,
            existing_policy=ExistingDestinationPolicy.REUSE,
            run_cleanup_filters=False,
        )
    )

    assert converted.success is True
    assert reused.success is True
    assert reused.reused is True
    assert reused.emitted_key == "Target"
