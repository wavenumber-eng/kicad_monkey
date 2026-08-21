"""
Subtest: Shared Corpus Source Model Readiness
Stratum: L1_parsing
Purpose: Prove the KiCad OOP parser exposes the real board surfaces needed by downstream conversion.

This subtest uses the shared KiCad corpus under the resolved `KM_CORPUS`
instead of only the legacy parser fixtures. The goal is to validate the parser
surface we actually intend to use for Track D and future neutral-model work.
"""

import pytest

from kicad_monkey import KiCadPcb
from kicad_monkey.testing.corpus import get_kicad_common_case_dir


def _shared_case_board(case_name: str, board_name: str):
    case_root = get_kicad_common_case_dir(case_name)
    return next(case_root.glob(f"input/**/{board_name}"))


@pytest.mark.parametrize(
    (
        "case_name",
        "board_name",
        "expected_layers",
        "expected_footprints",
        "expected_nets",
        "expected_segments",
        "expected_vias",
        "expected_zones",
        "expected_edge_cuts_lines",
        "expect_stackup",
    ),
    [
        pytest.param(
            "custom_pads_test",
            "custom_pads_test.kicad_pcb",
            20,
            5,
            4,
            19,
            0,
            7,
            4,
            True,
            id="custom_pads_test",
        ),
        pytest.param(
            "microwave",
            "microwave.kicad_pcb",
            20,
            4,
            1,
            0,
            0,
            0,
            4,
            False,
            id="microwave",
        ),
        pytest.param(
            "multichannel",
            "multichannel_mixer.kicad_pcb",
            29,
            114,
            81,
            576,
            29,
            6,
            4,
            False,
            id="multichannel_mixer",
        ),
        pytest.param(
            "speedy_reva",
            "11-10084__speedy_processing_module__A.kicad_pcb",
            42,
            533,
            0,
            7109,
            1106,
            52,
            0,
            True,
            id="speedy_reva",
        ),
    ],
)
def test_shared_corpus_boards_expose_expected_primary_surfaces(
    case_name: str,
    board_name: str,
    expected_layers: int,
    expected_footprints: int,
    expected_nets: int,
    expected_segments: int,
    expected_vias: int,
    expected_zones: int,
    expected_edge_cuts_lines: int,
    expect_stackup: bool,
) -> None:
    pcb = KiCadPcb.from_file(_shared_case_board(case_name, board_name))

    edge_cuts_lines = [
        item for item in (getattr(pcb, "gr_lines", []) or []) if getattr(item, "layer", "") == "Edge.Cuts"
    ]

    assert len(getattr(pcb, "layers", []) or []) == expected_layers
    assert len(getattr(pcb, "footprints", []) or []) == expected_footprints
    assert len(getattr(pcb, "nets", []) or []) == expected_nets
    assert len(getattr(pcb, "segments", []) or []) == expected_segments
    assert len(getattr(pcb, "vias", []) or []) == expected_vias
    assert len(getattr(pcb, "zones", []) or []) == expected_zones
    assert len(edge_cuts_lines) == expected_edge_cuts_lines
    assert (getattr(pcb, "stackup", None) is not None) is expect_stackup


@pytest.mark.parametrize(
    ("case_name", "board_name"),
    [
        pytest.param("custom_pads_test", "custom_pads_test.kicad_pcb", id="custom_pads_test"),
        pytest.param("multichannel", "multichannel_mixer.kicad_pcb", id="multichannel_mixer"),
    ],
)
def test_shared_corpus_boards_expose_footprint_level_payloads(
    case_name: str,
    board_name: str,
) -> None:
    pcb = KiCadPcb.from_file(_shared_case_board(case_name, board_name))

    assert pcb.footprints, "Expected at least one parsed footprint"
    footprint = pcb.footprints[0]
    assert str(getattr(footprint, "library_link", "") or "").strip()
    assert isinstance(getattr(footprint, "properties", []), list)
    assert isinstance(getattr(footprint, "pads", []), list)
    assert len(footprint.pads) > 0

    edge_cuts_lines = [
        item for item in (getattr(pcb, "gr_lines", []) or []) if getattr(item, "layer", "") == "Edge.Cuts"
    ]
    assert edge_cuts_lines, "Expected Edge.Cuts carriers on real board graphics"


def test_speedy_reva_preserves_named_net_and_footprint_outline_surfaces() -> None:
    pcb = KiCadPcb.from_file(
        _shared_case_board("speedy_reva", "11-10084__speedy_processing_module__A.kicad_pcb")
    )

    pads_with_named_only_net = 0

    for footprint in pcb.footprints:
        for pad in getattr(footprint, "pads", []) or []:
            if (
                getattr(pad, "net", None)
                and getattr(pad.net, "ordinal", None) is None
                and str(getattr(pad.net, "name", "") or "").strip()
            ):
                pads_with_named_only_net += 1

    outline_carriers = pcb.board_outline_carriers()
    footprint_outline_carriers = [carrier for carrier in outline_carriers if carrier.owner_kind == "footprint"]
    footprint_outline_refs = {carrier.owner_ref for carrier in footprint_outline_carriers}

    assert len(pcb.nets) == 0
    assert pads_with_named_only_net == 2167
    assert sum(1 for segment in pcb.segments if str(getattr(segment.net, "name", "") or "").strip()) == 7109
    assert sum(1 for via in pcb.vias if str(getattr(via.net, "name", "") or "").strip()) == 1106
    assert sum(1 for arc in pcb.arcs if str(getattr(arc.net, "name", "") or "").strip()) == 1120
    assert sum(1 for zone in pcb.zones if str(getattr(zone.net, "name", "") or "").strip()) == 51
    assert len(footprint_outline_refs) == 1
    assert len(footprint_outline_carriers) == 48


def test_shared_corpus_exposes_footprint_net_tie_groups() -> None:
    pcb = KiCadPcb.from_file(_shared_case_board("tiny_tapeout", "tinytapeout-demo.kicad_pcb"))

    net_tie_groups = [
        group
        for footprint in pcb.footprints
        for group in getattr(footprint, "net_tie_pad_groups", []) or []
    ]

    assert net_tie_groups, "Expected parsed net-tie pad groups from shared KiCad corpus"
    assert any(tuple(group.pad_names) == ("1", "2") for group in net_tie_groups)


def test_shared_corpus_exposes_board_and_footprint_groups() -> None:
    pcb = KiCadPcb.from_file(_shared_case_board("tiny_tapeout", "tinytapeout-demo.kicad_pcb"))

    footprint_groups = [
        group
        for footprint in pcb.footprints
        for group in getattr(footprint, "groups", []) or []
    ]

    assert pcb.groups, "Expected parsed board-level groups from shared KiCad corpus"
    assert footprint_groups, "Expected parsed footprint-local groups from shared KiCad corpus"


def test_shared_corpus_exposes_zone_layer_connection_overrides() -> None:
    pcb = KiCadPcb.from_file(_shared_case_board("vme-wren", "vme-wren.kicad_pcb"))

    explicit_zone_layer_connections = [
        via.zone_layer_connections
        for via in pcb.vias
        if getattr(via, "zone_layer_connections", None) is not None
    ]

    assert explicit_zone_layer_connections, "Expected explicit zone-layer connection overrides"
    assert any(
        "In5.Cu" in zone_layer_connections.forced_layers
        for zone_layer_connections in explicit_zone_layer_connections
    )


def test_shared_corpus_exposes_footprint_local_dimensions() -> None:
    pcb = KiCadPcb.from_file(
        _shared_case_board(
            "royalblue54L_feather",
            "RoyalBlue54L-NFC-Antenna.kicad_pcb",
        )
    )

    footprint_dimensions = [
        dimension
        for footprint in pcb.footprints
        for dimension in getattr(footprint, "dimensions", []) or []
    ]

    assert footprint_dimensions, "Expected parsed footprint-local dimensions from shared KiCad corpus"
    assert any(dimension.dimension_type == "leader" for dimension in footprint_dimensions)
    assert any(
        getattr(dimension.format, "override_value", None) == "0.3mm Thickness"
        for dimension in footprint_dimensions
    )
    assert any(getattr(dimension.style, "text_frame", None) == 0 for dimension in footprint_dimensions)


def test_royalblue_feather_main_board_parses_kicad_teardrop_dialect() -> None:
    pcb = KiCadPcb.from_file(
        _shared_case_board("royalblue54L_feather", "RoyalBlue54L-Feather.kicad_pcb")
    )

    pads_with_teardrops = [
        pad
        for footprint in pcb.footprints
        for pad in getattr(footprint, "pads", []) or []
        if getattr(pad, "teardrops", None) is not None
    ]

    assert len(pcb.footprints) == 71
    assert len(pads_with_teardrops) >= 190
    assert any(
        pad.teardrops.curved_edges is False
        and pad.teardrops.filter_ratio == 0.9
        and pad.teardrops.enabled is True
        for pad in pads_with_teardrops
    )


def test_speedy_reva_exposes_typed_footprint_placement_metadata() -> None:
    pcb = KiCadPcb.from_file(
        _shared_case_board("speedy_reva", "11-10084__speedy_processing_module__A.kicad_pcb")
    )

    placements = [footprint.placement for footprint in pcb.footprints if getattr(footprint, "placement", None)]

    assert placements, "Expected typed footprint placement metadata on high-end shared-corpus board"
    assert any(placement.sheetname == "/TOP_LEVEL_IO/" for placement in placements)
    assert any(placement.sheetfile == "Ethernet.kicad_sch" for placement in placements)
    assert all(placement.path for placement in placements)


def test_shared_corpus_exposes_generated_tuning_patterns() -> None:
    pcb = KiCadPcb.from_file(
        _shared_case_board("speedy_reva", "11-10084__speedy_processing_module__A.kicad_pcb")
    )

    assert pcb.generated_items, "Expected parsed board-level generated items"
    assert any(item.generator_type == "tuning_pattern" for item in pcb.generated_items)
    assert any(item.members for item in pcb.generated_items)


@pytest.mark.parametrize(
    ("case_name", "board_name", "expected_min_assignments"),
    [
        pytest.param("multichannel", "multichannel_mixer.kicad_pcb", 0, id="multichannel_mixer"),
        pytest.param("speedy_reva", "11-10084__speedy_processing_module__A.kicad_pcb", 400, id="speedy_reva"),
    ],
)
def test_shared_corpus_exposes_project_net_settings_sidecar(
    case_name: str,
    board_name: str,
    expected_min_assignments: int,
) -> None:
    pcb = KiCadPcb.from_file(_shared_case_board(case_name, board_name))

    assert pcb.source_path is not None
    assert pcb.project is not None
    assert pcb.project.project_path is not None
    assert pcb.project.net_settings is not None
    assert pcb.project.net_settings.classes
    assert "Default" in {item.name for item in pcb.project.net_settings.classes}
    assert len(pcb.project.net_settings.netclass_assignments) >= expected_min_assignments
    default_class = next(item for item in pcb.project.net_settings.classes if item.name == "Default")
    assert default_class.track_width is not None
    assert default_class.clearance is not None
    assert default_class.diff_pair_gap is not None


def test_speedy_reva_exposes_typed_project_diff_pair_settings() -> None:
    pcb = KiCadPcb.from_file(
        _shared_case_board("speedy_reva", "11-10084__speedy_processing_module__A.kicad_pcb")
    )

    assert pcb.project is not None
    assert pcb.project.board_design_settings is not None
    assert pcb.project.board_design_settings.diff_pair_dimensions
    diff_pair_preset = pcb.project.board_design_settings.diff_pair_dimensions[0]
    assert diff_pair_preset.width is not None
    assert diff_pair_preset.gap is not None
    assert diff_pair_preset.via_gap is not None
    assert pcb.project.board_design_settings.tuning_pattern_settings is not None
    tuning = pcb.project.board_design_settings.tuning_pattern_settings
    assert tuning.diff_pair_defaults is not None
    assert tuning.diff_pair_defaults.spacing is not None
    assert tuning.diff_pair_defaults.corner_style is not None
