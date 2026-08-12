"""Typed, transactional Altium-to-KiCad asset conversion."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from collections.abc import Callable
from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path

from kicad_monkey import (
    KiCadEnvironment,
    KiCadFilterPipeline,
    KiCadNameIndex,
    find_all_elements,
    format_kicad_sexp,
    normalize_unsafe_footprint_pad_sizes,
    parse_sexp,
)


class AltiumAssetKind(StrEnum):
    """Altium library asset kind accepted by the conversion executor."""

    SYMBOL = "symbol"
    FOOTPRINT = "footprint"


class ExistingDestinationPolicy(StrEnum):
    """Caller-owned decision for an already-existing output file."""

    REUSE = "reuse"
    REPLACE = "replace"
    ERROR = "error"


class ConversionStage(StrEnum):
    """Reviewable stages in one conversion attempt."""

    REQUEST = "request"
    VERSION = "version"
    CONVERT = "convert"
    SELECT = "select"
    NORMALIZE = "normalize"
    FILTER = "filter"
    VALIDATE = "validate"
    PUBLISH = "publish"
    REUSE = "reuse"


class StageStatus(StrEnum):
    """Outcome of one conversion stage."""

    OK = "ok"
    SKIPPED = "skipped"
    ERROR = "error"


@dataclass(frozen=True, slots=True)
class ConversionStageRecord:
    """One structured stage outcome suitable for application diagnostics."""

    stage: ConversionStage
    status: StageStatus
    message: str
    returncode: int | None = None
    stdout: str = ""
    stderr: str = ""


@dataclass(frozen=True, slots=True)
class AltiumAssetConversionRequest:
    """Explicit, model-free request for one Altium library asset."""

    source_library: Path
    source_key: str
    destination: Path
    kind: AltiumAssetKind
    existing_policy: ExistingDestinationPolicy = ExistingDestinationPolicy.ERROR
    run_cleanup_filters: bool = True


@dataclass(frozen=True, slots=True)
class AltiumAssetConversionResult:
    """Typed result for one conversion attempt."""

    request: AltiumAssetConversionRequest
    success: bool
    destination: Path
    emitted_key: str | None
    kicad_version: str
    normalization_count: int
    reused: bool
    stages: tuple[ConversionStageRecord, ...]

    @property
    def errors(self) -> tuple[ConversionStageRecord, ...]:
        """Return only failed stage records."""

        return tuple(stage for stage in self.stages if stage.status is StageStatus.ERROR)


Runner = Callable[..., subprocess.CompletedProcess[str]]


class AltiumAssetConversionExecutor:
    """Convert one exact Altium asset without exposing application models."""

    _STAGING_PREFIX = ".kicad-convert-"

    def __init__(
        self,
        *,
        kicad_cli: str | Path | None = None,
        runner: Runner = subprocess.run,
        filter_pipeline: KiCadFilterPipeline | None = None,
    ) -> None:
        if kicad_cli is None:
            installation = KiCadEnvironment().highest_installation(ignore_beta=True)
            if installation is None:
                raise RuntimeError("Could not find a KiCad installation")
            resolved_cli: str | Path = installation.kicad_cli
        else:
            resolved_cli = kicad_cli

        self.kicad_cli = Path(resolved_cli).resolve()
        self._runner = runner
        self._filter_pipeline = filter_pipeline or KiCadFilterPipeline()
        self._name_index = KiCadNameIndex()

    def convert(self, request: AltiumAssetConversionRequest) -> AltiumAssetConversionResult:
        """Execute conversion, validation, and publication for one request."""

        normalized_request = AltiumAssetConversionRequest(
            source_library=Path(request.source_library).resolve(),
            source_key=request.source_key.strip(),
            destination=Path(request.destination).resolve(),
            kind=AltiumAssetKind(request.kind),
            existing_policy=ExistingDestinationPolicy(request.existing_policy),
            run_cleanup_filters=bool(request.run_cleanup_filters),
        )
        stages: list[ConversionStageRecord] = []
        kicad_version = ""

        request_error = self._validate_request(normalized_request)
        if request_error:
            stages.append(self._error(ConversionStage.REQUEST, request_error))
            return self._result(normalized_request, stages=stages)
        stages.append(self._ok(ConversionStage.REQUEST, "request is valid"))

        version_process = self._run_cli("--version")
        if version_process.returncode != 0:
            stages.append(self._process_error(ConversionStage.VERSION, version_process))
            return self._result(normalized_request, stages=stages)
        kicad_version = (version_process.stdout or version_process.stderr).strip()
        stages.append(self._ok(ConversionStage.VERSION, kicad_version or "version resolved"))

        destination = normalized_request.destination
        if destination.exists():
            if normalized_request.existing_policy is ExistingDestinationPolicy.ERROR:
                stages.append(
                    self._error(ConversionStage.REUSE, f"destination already exists: {destination}")
                )
                return self._result(
                    normalized_request,
                    stages=stages,
                    kicad_version=kicad_version,
                )
            if normalized_request.existing_policy is ExistingDestinationPolicy.REUSE:
                return self._reuse_existing(
                    normalized_request,
                    stages=stages,
                    kicad_version=kicad_version,
                )

        return self._convert_staged(
            normalized_request,
            stages=stages,
            kicad_version=kicad_version,
        )

    def _convert_staged(
        self,
        request: AltiumAssetConversionRequest,
        *,
        stages: list[ConversionStageRecord],
        kicad_version: str,
    ) -> AltiumAssetConversionResult:
        destination = request.destination
        destination.parent.mkdir(parents=True, exist_ok=True)
        normalization_count = 0
        try:
            with tempfile.TemporaryDirectory(
                prefix=self._STAGING_PREFIX,
                dir=destination.parent,
            ) as temporary:
                staging = Path(temporary).resolve()
                prepared = self._convert_and_select(request, staging, stages)
                if prepared is None:
                    return self._result(request, stages=stages, kicad_version=kicad_version)

                normalization_count, filters_succeeded = self._normalize_and_filter(
                    request, prepared, stages
                )
                if not filters_succeeded:
                    return self._result(
                        request,
                        stages=stages,
                        kicad_version=kicad_version,
                        normalization_count=normalization_count,
                    )

                emitted_key = self._exact_emitted_key(
                    request.kind,
                    prepared,
                    request.source_key,
                )
                if emitted_key is None:
                    stages.append(
                        self._error(
                            ConversionStage.VALIDATE,
                            "prepared output no longer contains the requested exact native key",
                        )
                    )
                    return self._result(
                        request,
                        stages=stages,
                        kicad_version=kicad_version,
                        normalization_count=normalization_count,
                    )

                return self._validate_and_publish(
                    request,
                    prepared=prepared,
                    staging=staging,
                    emitted_key=emitted_key,
                    stages=stages,
                    kicad_version=kicad_version,
                    normalization_count=normalization_count,
                )
        except Exception as exc:
            stages.append(self._error(ConversionStage.PUBLISH, f"staging failed: {exc}"))
            return self._result(
                request,
                stages=stages,
                kicad_version=kicad_version,
                normalization_count=normalization_count,
            )

    def _normalize_and_filter(
        self,
        request: AltiumAssetConversionRequest,
        prepared: Path,
        stages: list[ConversionStageRecord],
    ) -> tuple[int, bool]:
        normalization_count = 0
        if request.kind is AltiumAssetKind.FOOTPRINT:
            count = self._normalize_footprint(prepared)
            normalization_count += count
            stages.append(
                self._ok(
                    ConversionStage.NORMALIZE,
                    f"normalized {count} unsafe direct pad size(s)",
                )
            )
        else:
            stages.append(
                self._skipped(
                    ConversionStage.NORMALIZE,
                    "pad normalization does not apply to symbols",
                )
            )

        if request.run_cleanup_filters:
            try:
                self._apply_cleanup_filters(request.kind, prepared)
            except Exception as exc:
                stages.append(self._error(ConversionStage.FILTER, str(exc)))
                return normalization_count, False
            stages.append(self._ok(ConversionStage.FILTER, "cleanup filters completed"))
        else:
            stages.append(
                self._skipped(ConversionStage.FILTER, "cleanup filters disabled by caller")
            )

        if request.kind is AltiumAssetKind.FOOTPRINT:
            post_filter_count = self._normalize_footprint(prepared)
            normalization_count += post_filter_count
            if post_filter_count:
                stages.append(
                    self._ok(
                        ConversionStage.NORMALIZE,
                        f"normalized {post_filter_count} post-filter pad size(s)",
                    )
                )
        return normalization_count, True

    def _validate_and_publish(
        self,
        request: AltiumAssetConversionRequest,
        *,
        prepared: Path,
        staging: Path,
        emitted_key: str,
        stages: list[ConversionStageRecord],
        kicad_version: str,
        normalization_count: int,
    ) -> AltiumAssetConversionResult:
        validation = self._validate_with_cli(request.kind, prepared, staging)
        if validation.returncode != 0:
            stages.append(self._process_error(ConversionStage.VALIDATE, validation))
            return self._result(
                request,
                stages=stages,
                kicad_version=kicad_version,
                normalization_count=normalization_count,
            )
        stages.append(self._ok(ConversionStage.VALIDATE, "KiCad CLI validation passed"))

        try:
            os.replace(prepared, request.destination)
        except OSError as exc:
            stages.append(self._error(ConversionStage.PUBLISH, str(exc)))
            return self._result(
                request,
                stages=stages,
                kicad_version=kicad_version,
                normalization_count=normalization_count,
            )

        stages.append(self._ok(ConversionStage.PUBLISH, "published atomically"))
        return self._result(
            request,
            success=True,
            emitted_key=emitted_key,
            stages=stages,
            kicad_version=kicad_version,
            normalization_count=normalization_count,
        )

    def _reuse_existing(
        self,
        request: AltiumAssetConversionRequest,
        *,
        stages: list[ConversionStageRecord],
        kicad_version: str,
    ) -> AltiumAssetConversionResult:
        destination = request.destination
        destination.parent.mkdir(parents=True, exist_ok=True)

        try:
            with tempfile.TemporaryDirectory(
                prefix=self._STAGING_PREFIX,
                dir=destination.parent,
            ) as temporary:
                staging = Path(temporary).resolve()
                copy = staging / destination.name
                shutil.copy2(destination, copy)

                if request.kind is AltiumAssetKind.FOOTPRINT:
                    normalization_count = self._normalize_footprint(copy)
                    if normalization_count:
                        stages.append(
                            self._error(
                                ConversionStage.REUSE,
                                "existing destination requires mandatory normalization; "
                                "replace it explicitly",
                            )
                        )
                        return self._result(
                            request,
                            stages=stages,
                            kicad_version=kicad_version,
                        )

                emitted_key = self._exact_emitted_key(request.kind, copy, request.source_key)
                if emitted_key is None:
                    stages.append(
                        self._error(
                            ConversionStage.REUSE,
                            "existing destination does not contain the requested exact native key",
                        )
                    )
                    return self._result(
                        request,
                        stages=stages,
                        kicad_version=kicad_version,
                    )

                validation = self._validate_with_cli(request.kind, copy, staging)
                if validation.returncode != 0:
                    stages.append(self._process_error(ConversionStage.VALIDATE, validation))
                    return self._result(
                        request,
                        stages=stages,
                        kicad_version=kicad_version,
                    )

                stages.append(self._ok(ConversionStage.VALIDATE, "existing output is valid"))
                stages.append(self._ok(ConversionStage.REUSE, "reused existing destination"))
                return self._result(
                    request,
                    success=True,
                    emitted_key=emitted_key,
                    stages=stages,
                    kicad_version=kicad_version,
                    reused=True,
                )
        except Exception as exc:
            stages.append(self._error(ConversionStage.REUSE, str(exc)))
            return self._result(request, stages=stages, kicad_version=kicad_version)

    def _convert_and_select(
        self,
        request: AltiumAssetConversionRequest,
        staging: Path,
        stages: list[ConversionStageRecord],
    ) -> Path | None:
        if request.kind is AltiumAssetKind.SYMBOL:
            converted = staging / "converted.kicad_sym"
            process = self._run_cli(
                "sym",
                "upgrade",
                "--output",
                str(converted),
                str(request.source_library),
            )
            if process.returncode != 0:
                stages.append(self._process_error(ConversionStage.CONVERT, process))
                return None
            if not converted.is_file():
                stages.append(
                    self._error(ConversionStage.CONVERT, "no symbol library was generated")
                )
                return None
            stages.append(self._ok(ConversionStage.CONVERT, "symbol library generated"))

            if self._exact_emitted_key(request.kind, converted, request.source_key) is None:
                stages.append(
                    self._error(
                        ConversionStage.SELECT,
                        "generated symbol library does not contain the requested exact native key",
                    )
                )
                return None
            stages.append(self._ok(ConversionStage.SELECT, "selected exact symbol key"))
            return converted

        converted_library = staging / "converted.pretty"
        process = self._run_cli(
            "fp",
            "upgrade",
            "--output",
            str(converted_library),
            str(request.source_library),
        )
        if process.returncode != 0:
            stages.append(self._process_error(ConversionStage.CONVERT, process))
            return None
        stages.append(self._ok(ConversionStage.CONVERT, "footprint library generated"))

        matches: list[Path] = []
        available: list[str] = []
        for candidate in sorted(converted_library.rglob("*.kicad_mod")):
            names = self._name_index.footprint_names(candidate)
            available.extend(names)
            if request.source_key in names:
                matches.append(candidate)

        if len(matches) != 1:
            detail = ", ".join(sorted(set(available))) or "none"
            stages.append(
                self._error(
                    ConversionStage.SELECT,
                    f"expected one exact footprint key {request.source_key!r}; "
                    f"found {len(matches)} (available: {detail})",
                )
            )
            return None

        prepared = staging / "prepared.kicad_mod"
        shutil.copy2(matches[0], prepared)
        stages.append(self._ok(ConversionStage.SELECT, "selected exact footprint key"))
        return prepared

    def _normalize_footprint(self, path: Path) -> int:
        expression = parse_sexp(path.read_text(encoding="utf-8"), source_path=path)
        result = normalize_unsafe_footprint_pad_sizes(expression)
        if result.count:
            text = format_kicad_sexp(result.expression)
            path.write_text(text.rstrip("\n") + "\n", encoding="utf-8")
        return result.count

    def _apply_cleanup_filters(self, kind: AltiumAssetKind, path: Path) -> None:
        if kind is AltiumAssetKind.FOOTPRINT:
            self._filter_pipeline.filter_footprint_import(path, path)
        else:
            self._filter_pipeline.filter_symbol(path, path)

    def _exact_emitted_key(
        self,
        kind: AltiumAssetKind,
        path: Path,
        requested_key: str,
    ) -> str | None:
        if kind is AltiumAssetKind.FOOTPRINT:
            names = self._name_index.footprint_names(path)
        else:
            expression = parse_sexp(path.read_text(encoding="utf-8"), source_path=path)
            names = [
                str(symbol[1])
                for symbol in find_all_elements(expression, "symbol")
                if len(symbol) > 1
            ]
        return requested_key if names.count(requested_key) == 1 else None

    def _validate_with_cli(
        self,
        kind: AltiumAssetKind,
        prepared: Path,
        staging: Path,
    ) -> subprocess.CompletedProcess[str]:
        if kind is AltiumAssetKind.SYMBOL:
            output = staging / "validated.kicad_sym"
            return self._run_cli(
                "sym",
                "upgrade",
                "--force",
                "--output",
                str(output),
                str(prepared),
            )

        validation_input = staging / "validation-input.pretty"
        validation_input.mkdir()
        shutil.copy2(prepared, validation_input / "asset.kicad_mod")
        validation_output = staging / "validation-output.pretty"
        return self._run_cli(
            "fp",
            "upgrade",
            "--force",
            "--output",
            str(validation_output),
            str(validation_input),
        )

    def _validate_request(self, request: AltiumAssetConversionRequest) -> str:
        if not request.source_key:
            return "source_key is required"
        if not request.source_library.is_file():
            return f"source library does not exist: {request.source_library}"
        if request.destination.exists() and not request.destination.is_file():
            return f"destination is not a file: {request.destination}"

        expected_source = ".schlib" if request.kind is AltiumAssetKind.SYMBOL else ".pcblib"
        expected_destination = (
            ".kicad_sym" if request.kind is AltiumAssetKind.SYMBOL else ".kicad_mod"
        )
        if request.source_library.suffix.lower() != expected_source:
            return f"{request.kind.value} source must use {expected_source}"
        if request.destination.suffix.lower() != expected_destination:
            return f"{request.kind.value} destination must use {expected_destination}"
        return ""

    def _run_cli(self, *args: str) -> subprocess.CompletedProcess[str]:
        return self._runner(
            [str(self.kicad_cli), *args],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )

    @staticmethod
    def _trim(value: str | None) -> str:
        return (value or "")[-4000:]

    @classmethod
    def _process_error(
        cls,
        stage: ConversionStage,
        process: subprocess.CompletedProcess[str],
    ) -> ConversionStageRecord:
        return ConversionStageRecord(
            stage=stage,
            status=StageStatus.ERROR,
            message=f"KiCad CLI exited with {process.returncode}",
            returncode=process.returncode,
            stdout=cls._trim(process.stdout),
            stderr=cls._trim(process.stderr),
        )

    @staticmethod
    def _ok(stage: ConversionStage, message: str) -> ConversionStageRecord:
        return ConversionStageRecord(stage, StageStatus.OK, message)

    @staticmethod
    def _skipped(stage: ConversionStage, message: str) -> ConversionStageRecord:
        return ConversionStageRecord(stage, StageStatus.SKIPPED, message)

    @staticmethod
    def _error(stage: ConversionStage, message: str) -> ConversionStageRecord:
        return ConversionStageRecord(stage, StageStatus.ERROR, message)

    @staticmethod
    def _result(
        request: AltiumAssetConversionRequest,
        *,
        stages: list[ConversionStageRecord],
        success: bool = False,
        emitted_key: str | None = None,
        kicad_version: str = "",
        normalization_count: int = 0,
        reused: bool = False,
    ) -> AltiumAssetConversionResult:
        return AltiumAssetConversionResult(
            request=request,
            success=success,
            destination=request.destination,
            emitted_key=emitted_key,
            kicad_version=kicad_version,
            normalization_count=normalization_count,
            reused=reused,
            stages=tuple(stages),
        )


__all__ = [
    "AltiumAssetConversionExecutor",
    "AltiumAssetConversionRequest",
    "AltiumAssetConversionResult",
    "AltiumAssetKind",
    "ConversionStage",
    "ConversionStageRecord",
    "ExistingDestinationPolicy",
    "StageStatus",
]
