"""High-level KiCad CLI workflows."""

from kicad_cruncher._version import Version, __version__, version
from kicad_cruncher.altium_asset_conversion import (
    AltiumAssetConversionExecutor,
    AltiumAssetConversionRequest,
    AltiumAssetConversionResult,
    AltiumAssetKind,
    ConversionStage,
    ConversionStageRecord,
    ExistingDestinationPolicy,
    StageStatus,
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
    "Version",
    "__version__",
    "version",
]
