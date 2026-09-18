use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use kicad_cruncher_cli::pcb_svg::geometer::NativeGeometer;
use kicad_cruncher_cli::pcb_svg::preview::{
    BoardSide, render_toon_preview, render_top_toon_preview,
};
use kicad_cruncher_cli::toon::run_toon;
use kicad_cruncher_cli::{ToonOptions, ToonSides};

const BOARD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../tests/corpus/kicad/projects/hlr_test/hlr_test.kicad_pcb"
);

#[test]
fn released_process_composes_a_native_toon_board_svg() {
    let required = std::env::var_os("KCR_REQUIRE_GEOMETER_TEST").is_some();
    let Some(executable) = std::env::var_os("GEOMETER_EXECUTABLE") else {
        assert!(
            !required,
            "GEOMETER_EXECUTABLE is required for this test lane"
        );
        return;
    };
    assert_preview_process(executable);
    assert_public_transactional_command();
}

fn assert_preview_process(executable: std::ffi::OsString) {
    let source = fs::read_to_string(BOARD).expect("read embedded-model fixture");
    let service = NativeGeometer::connect(executable).expect("connect released Geometer");
    let preview = render_top_toon_preview(&source, "hlr-test", &service)
        .expect("compose native Toon preview");
    assert_top_preview(&preview.svg, preview.rendered_model_count);
    assert_altium_toon_order(&preview.svg);

    let bottom = render_toon_preview(&source, "hlr-test-bottom", BoardSide::Bottom, &service)
        .expect("compose bottom native Toon preview");
    assert!(bottom.svg.contains("id=\"bottom-view-mirror\""));
    assert!(
        bottom
            .svg
            .contains("data-layer-token=\"SOLDERMASK_FILM_BOTTOM\"")
    );
    assert!(bottom.svg.contains("id=\"illustration-bottom\""));
    service.close().expect("clean Geometer shutdown");
}

fn assert_top_preview(svg: &str, rendered_model_count: usize) {
    assert!(rendered_model_count > 0);
    assert!(svg.contains("<svg "));
    assert!(svg.trim_end().ends_with("</svg>"));
    assert!(svg.contains("id=\"illustration-top\""));
    assert!(svg.contains("data-component=\"U1\""));
    assert!(svg.contains("data-model-sha256="));
    assert!(svg.contains("<path "));
    assert!(svg.contains("<line "));
    assert!(svg.contains("fill=\"#EEEEEE\""));
    assert!(svg.contains("data-hlr-outline-width-mm=\"0.025\""));
    assert!(svg.contains("data-hlr-detail-width-mm=\"0.01375\""));
}

fn assert_altium_toon_order(svg: &str) {
    let ordered_tokens = [
        "BOARD_SUBSTRATE",
        "TOP",
        "SOLDERMASK_FILM_TOP",
        "TOPOVERLAY",
        "BOARD_CUTOUTS",
        "DRILLS",
        "SLOTS",
        "BOARD_OUTLINE",
        "ILLUSTRATION_TOP",
    ];
    let mut previous = 0;
    for token in ordered_tokens {
        let position = svg[previous..]
            .find(&format!("data-layer-token=\"{token}\""))
            .map(|offset| previous + offset)
            .unwrap_or_else(|| panic!("missing Toon layer {token}"));
        assert!(position >= previous, "Toon layer {token} is out of order");
        previous = position;
    }
}

fn assert_public_transactional_command() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "kicad-cruncher-toon-process-{}-{nonce}",
        std::process::id()
    ));
    let output = root.join("toon");
    fs::create_dir_all(&output).expect("create previous output");
    fs::write(output.join("obsolete.txt"), "previous").expect("write previous output");
    let run = run_toon(&ToonOptions {
        input: Some(BOARD.into()),
        output: Some(output.clone()),
        pcbdoc: None,
        sides: ToonSides::Both,
        assembly: false,
        footprint: None,
        variant: None,
        all_variants: false,
        theme: None,
        soldermask_color: Some("#EEEEEE".to_owned()),
        config: Some(root.join("toon.config")),
        write_config: None,
        workers: 1,
        cache_dir: None,
        no_cache: true,
        timings: None,
    })
    .expect("run public transactional Toon command");
    assert_eq!(run.artifact_count, 2);
    assert!(!output.join("obsolete.txt").exists());
    let manifest = fs::read_to_string(output.join("manifest.json")).expect("read Toon manifest");
    assert!(manifest.contains("\"schema\": \"kicad_cruncher.toon_manifest.a0\""));
    assert!(manifest.contains("\"source\": \"hlr_test.kicad_pcb\""));
    assert!(manifest.contains("\"soldermask_color\": \"#EEEEEE\""));
    assert!(!manifest.contains(env!("CARGO_MANIFEST_DIR")));
    assert!(output.join("hlr_test__toon_top.svg").is_file());
    assert!(output.join("hlr_test__toon_bottom.svg").is_file());
    fs::remove_dir_all(root).expect("remove Toon process test output");
}
