from __future__ import annotations

import json
from pathlib import Path

from kicad_monkey import KiCadEnvironment, KiCadFilterPipeline


def _fake_install(root: Path, version: str, *, with_models: bool = False) -> Path:
    install = root / "Programs" / "KiCad" / version
    bin_dir = install / "bin"
    bin_dir.mkdir(parents=True)
    (bin_dir / "kicad-cli.exe").write_text("", encoding="utf-8")
    if with_models:
        (install / "share" / "kicad" / "3dmodels").mkdir(parents=True)
    return install


def _write_kicad_common(app_data: Path, version: str, payload: dict) -> Path:
    config_dir = app_data / "kicad" / version
    config_dir.mkdir(parents=True)
    common = config_dir / "kicad_common.json"
    common.write_text(json.dumps(payload), encoding="utf-8")
    return common


def test_kicad_environment_finds_highest_non_beta_installation(tmp_path) -> None:
    local_app_data = tmp_path / "local-app-data"
    install_90 = _fake_install(local_app_data, "9.0")
    install_110 = _fake_install(local_app_data, "11.0")
    install_beta = _fake_install(local_app_data, "11.99")

    environment = KiCadEnvironment(
        env={"LOCALAPPDATA": str(local_app_data)},
        platform="win32",
    )

    installations = environment.find_installations()
    highest_stable = environment.highest_installation(installations, ignore_beta=True)
    highest_any = environment.highest_installation(installations, ignore_beta=False)

    assert {
        install_90,
        install_110,
        install_beta,
    } <= {installation.root for installation in installations}
    assert highest_stable is not None
    assert highest_stable.root == install_110
    assert highest_stable.kicad_cli == install_110 / "bin" / "kicad-cli.exe"
    assert highest_any is not None
    assert highest_any.root == install_beta


def test_kicad_environment_finds_versioned_config_paths(tmp_path) -> None:
    app_data = tmp_path / "roaming"
    config_root = app_data / "kicad"
    for name in ("8.0", "10.0", "11.0", "not-version"):
        (config_root / name).mkdir(parents=True)

    environment = KiCadEnvironment(
        env={"APPDATA": str(app_data)},
        platform="win32",
    )

    assert [path.name for path in environment.find_config_paths(min_major=10)] == [
        "10.0",
        "11.0",
    ]


def test_kicad_environment_maps_model_directory_variables(tmp_path) -> None:
    local_app_data = tmp_path / "local-app-data"
    install_90 = _fake_install(local_app_data, "9.0", with_models=True)
    install_100 = _fake_install(local_app_data, "10.0", with_models=True)
    _fake_install(local_app_data, "11.0")  # no 3dmodels dir -> excluded

    # platform="linux" scans only the LOCALAPPDATA root, isolating the test from
    # any real KiCad install under the hardcoded Windows Program Files roots.
    environment = KiCadEnvironment(
        env={"LOCALAPPDATA": str(local_app_data)},
        platform="linux",
    )

    variables = environment.model_directory_variables()

    assert variables == {
        "KICAD9_3DMODEL_DIR": str(install_90 / "share" / "kicad" / "3dmodels"),
        "KICAD10_3DMODEL_DIR": str(install_100 / "share" / "kicad" / "3dmodels"),
    }


def test_kicad_environment_reads_config_path_variables(tmp_path) -> None:
    app_data = tmp_path / "roaming"
    _write_kicad_common(
        app_data,
        "9.0",
        {"environment": {"vars": {"ANT3DMDL": "C:/old"}}},
    )
    _write_kicad_common(
        app_data,
        "10.0",
        {
            "environment": {"vars": {"ANT3DMDL": "C:/models/ant", "EMPTY": None}},
            "system": {"extra_3d_search_dirs": ["C:/extra/a", "C:/extra/b"]},
        },
    )

    environment = KiCadEnvironment(env={"APPDATA": str(app_data)}, platform="win32")

    # The highest non-beta config (10.0) wins over 9.0.
    assert environment.config_path_variables() == {"ANT3DMDL": "C:/models/ant"}
    assert environment.extra_3d_model_search_dirs() == (Path("C:/extra/a"), Path("C:/extra/b"))


def test_kicad_environment_handles_null_environment_vars(tmp_path) -> None:
    app_data = tmp_path / "roaming"
    _write_kicad_common(app_data, "10.0", {"environment": {"vars": None}})

    environment = KiCadEnvironment(env={"APPDATA": str(app_data)}, platform="win32")

    assert environment.config_path_variables() == {}
    assert environment.extra_3d_model_search_dirs() == ()


def test_kicad_environment_path_variable_map_overlays_config(tmp_path) -> None:
    local_app_data = tmp_path / "local-app-data"
    install_100 = _fake_install(local_app_data, "10.0", with_models=True)
    home = tmp_path / "home"
    _write_kicad_common(
        home / ".config",
        "10.0",
        {"environment": {"vars": {"KICAD10_3DMODEL_DIR": "C:/custom/models", "ANT3DMDL": "C:/ant"}}},
    )

    # Use linux config discovery to isolate this overlay test from real Windows
    # KiCad installs under the hardcoded Program Files roots.
    environment = KiCadEnvironment(
        env={"LOCALAPPDATA": str(local_app_data), "HOME": str(home)},
        platform="linux",
    )

    install_dir = str(install_100 / "share" / "kicad" / "3dmodels")
    assert environment.model_directory_variables()["KICAD10_3DMODEL_DIR"] == install_dir
    # User override wins over the install default; custom vars are merged in.
    variable_map = environment.path_variable_map()
    assert variable_map["KICAD10_3DMODEL_DIR"] == "C:/custom/models"
    assert variable_map["ANT3DMDL"] == "C:/ant"


def test_filter_pipeline_exposes_file_level_operations() -> None:
    pipeline = KiCadFilterPipeline()

    assert callable(pipeline.filter_footprint)
    assert callable(pipeline.filter_symbol)
    assert callable(pipeline.filter_schematic)
    assert callable(pipeline.filter_pcb)
