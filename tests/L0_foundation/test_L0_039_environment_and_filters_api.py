from __future__ import annotations

import json
import logging
from pathlib import Path

from kicad_monkey import KiCadEnvironment, KiCadFilterPipeline, setup_kicad_preferences
from kicad_monkey.kicad_setup import setup_kicad
from kicad_monkey.kicad_utilities import make_kicad_dblib, make_kicad_httplib


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


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload), encoding="utf-8")


def _read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


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


def test_library_generators_use_profile_neutral_defaults(tmp_path) -> None:
    httplib_path = make_kicad_httplib(
        output_dir=tmp_path,
        library_nickname="parts_api",
    )
    httplib = _read_json(httplib_path)

    assert httplib["name"] == "parts_api HTTP Library"
    assert httplib["description"] == "parts_api HTTP Library"

    dblib_path = make_kicad_dblib(
        "parts",
        ["resistors"],
        output_dir=tmp_path,
        primary_key="part_number",
    )
    dblib = _read_json(dblib_path)

    assert "DSN=parts;" in dblib["source"]["connection_string"]
    assert "wavenumber" not in json.dumps({"httplib": httplib, "dblib": dblib}).lower()
    assert "wn__" not in dblib["source"]["connection_string"]


def test_setup_kicad_has_no_default_profile_name() -> None:
    assert setup_kicad.__kwdefaults__["dblib_name"] is None


def test_setup_kicad_preferences_merges_source_preference_files(tmp_path, caplog) -> None:
    preferences_source = tmp_path / "prefs-source"
    config_dir = tmp_path / "config" / "10.0"
    existing_editor = tmp_path / "editor.exe"
    existing_editor.write_text("", encoding="utf-8")

    _write_json(preferences_source / "colors" / "alpha.json", {"name": "alpha"})
    _write_json(
        preferences_source / "kicad_common.json",
        {
            "input": {"zoom_speed": 4, "center_on_zoom": True},
            "graphics": {"antialiasing": True},
            "new_section": {"enabled": True},
        },
    )
    _write_json(
        preferences_source / "eeschema.json",
        {"appearance": {"color_theme": "alpha", "default_font": "Inter"}},
    )
    _write_json(
        preferences_source / "pcbnew.json",
        {
            "appearance": {"color_theme": "alpha"},
            "pcb_display": {
                "origin_invert_x_axis": True,
                "origin_invert_y_axis": False,
                "origin_mode": 2,
            },
        },
    )
    for filename in ("fpedit.json", "pl_editor.json", "gerbview.json"):
        _write_json(
            preferences_source / filename,
            {"appearance": {"color_theme": "alpha"}},
        )

    _write_json(
        config_dir / "kicad_common.json",
        {
            "input": {"keep_existing": True},
            "system": {"text_editor": str(existing_editor)},
        },
    )
    _write_json(
        config_dir / "eeschema.json",
        {"appearance": {"custom_grid": True}, "preserve": "root"},
    )
    _write_json(
        config_dir / "pcbnew.json",
        {"pcb_display": {"keep_origin_setting": "yes"}},
    )

    caplog.set_level(logging.INFO)

    success, backups = setup_kicad_preferences(
        with_backup=False,
        preferences_source=preferences_source,
        config_paths=[config_dir],
    )

    assert success is True
    assert backups == []
    assert _read_json(config_dir / "colors" / "alpha.json") == {"name": "alpha"}

    common = _read_json(config_dir / "kicad_common.json")
    assert common["input"]["zoom_speed"] == 4
    assert common["input"]["center_on_zoom"] is True
    assert common["input"]["keep_existing"] is True
    assert common["graphics"] == {"antialiasing": True}
    assert common["new_section"] == {"enabled": True}
    assert common["system"]["text_editor"] == str(existing_editor)

    eeschema = _read_json(config_dir / "eeschema.json")
    assert eeschema["appearance"]["color_theme"] == "alpha"
    assert eeschema["appearance"]["default_font"] == "Inter"
    assert eeschema["appearance"]["custom_grid"] is True
    assert eeschema["preserve"] == "root"

    pcbnew = _read_json(config_dir / "pcbnew.json")
    assert pcbnew["appearance"]["color_theme"] == "alpha"
    assert pcbnew["pcb_display"]["origin_invert_x_axis"] is True
    assert pcbnew["pcb_display"]["origin_invert_y_axis"] is False
    assert pcbnew["pcb_display"]["origin_mode"] == 2
    assert pcbnew["pcb_display"]["keep_origin_setting"] == "yes"

    for filename in ("fpedit.json", "pl_editor.json", "gerbview.json"):
        assert _read_json(config_dir / filename)["appearance"]["color_theme"] == "alpha"

    assert "themes: alpha" in caplog.text
    assert "color_theme=alpha" in caplog.text
    assert "origin_mode=2" in caplog.text
    assert "wavenumber" not in caplog.text.lower()


def test_setup_kicad_preferences_skips_missing_optional_preference_files(tmp_path) -> None:
    preferences_source = tmp_path / "prefs-source"
    config_dir = tmp_path / "config" / "10.0"
    _write_json(
        preferences_source / "eeschema.json",
        {"appearance": {"color_theme": "beta"}},
    )
    _write_json(
        config_dir / "pcbnew.json",
        {"pcb_display": {"origin_mode": 7}},
    )

    success, backups = setup_kicad_preferences(
        with_backup=False,
        preferences_source=preferences_source,
        config_paths=[config_dir],
    )

    assert success is True
    assert backups == []
    assert _read_json(config_dir / "eeschema.json")["appearance"]["color_theme"] == "beta"
    assert _read_json(config_dir / "pcbnew.json")["pcb_display"]["origin_mode"] == 7
    assert not (config_dir / "gerbview.json").exists()


def test_setup_kicad_preferences_preserves_command_style_editor_when_source_omits_it(tmp_path) -> None:
    preferences_source = tmp_path / "prefs-source"
    config_dir = tmp_path / "config" / "10.0"
    _write_json(
        preferences_source / "kicad_common.json",
        {"input": {"zoom_speed": 8}},
    )
    _write_json(
        config_dir / "kicad_common.json",
        {"system": {"text_editor": "code --reuse-window"}},
    )

    success, backups = setup_kicad_preferences(
        with_backup=False,
        preferences_source=preferences_source,
        config_paths=[config_dir],
    )

    assert success is True
    assert backups == []
    common = _read_json(config_dir / "kicad_common.json")
    assert common["input"]["zoom_speed"] == 8
    assert common["system"]["text_editor"] == "code --reuse-window"


def test_setup_kicad_preferences_uses_source_text_editor_when_present(tmp_path) -> None:
    preferences_source = tmp_path / "prefs-source"
    config_dir = tmp_path / "config" / "10.0"
    _write_json(
        preferences_source / "kicad_common.json",
        {"system": {"text_editor": "source-editor --wait"}},
    )
    _write_json(
        config_dir / "kicad_common.json",
        {"system": {"text_editor": "code --reuse-window"}},
    )

    success, backups = setup_kicad_preferences(
        with_backup=False,
        preferences_source=preferences_source,
        config_paths=[config_dir],
    )

    assert success is True
    assert backups == []
    assert _read_json(config_dir / "kicad_common.json")["system"]["text_editor"] == "source-editor --wait"
