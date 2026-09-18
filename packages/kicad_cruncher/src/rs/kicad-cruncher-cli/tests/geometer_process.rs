use std::fs;

use geometer_client::contracts::{
    MeshIllustrationView, ModelAttachmentIllustrationSourceA0, ModelIllustrationGeometryRequestA0,
    ModelIllustrationSourceA0,
};
use kicad_cruncher_cli::pcb_svg::geometer::{
    GEOMETER_C_ABI_GENERATION, GEOMETER_RELEASE, NativeGeometer,
};
use kicad_cruncher_cli::pcb_svg::models::read_embedded_models;
use kicad_monkey_core::{PcbLimits, PcbView};

#[test]
fn released_process_runs_one_pass_embedded_step_illustration() {
    let required = std::env::var_os("KCR_REQUIRE_GEOMETER_TEST").is_some();
    let Some(executable) = std::env::var_os("GEOMETER_EXECUTABLE") else {
        assert!(
            !required,
            "GEOMETER_EXECUTABLE is required for this test lane"
        );
        return;
    };
    let service = NativeGeometer::connect(executable).expect("connect released Geometer");
    assert_eq!(service.client().welcome().release_version, GEOMETER_RELEASE);
    assert_eq!(
        service.client().welcome().c_abi_generation,
        GEOMETER_C_ABI_GENERATION
    );

    let board = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/corpus/kicad/projects/hlr_test/hlr_test.kicad_pcb"
    ))
    .expect("read embedded-model fixture");
    let view = PcbView::parse(&board, PcbLimits::default()).expect("parse fixture");
    let models = read_embedded_models(&view).expect("read embedded model");
    let model = models
        .instances
        .first()
        .expect("fixture has an embedded STEP");
    let result = service
        .illustrate_model(
            ModelIllustrationGeometryRequestA0 {
                schema: "geometry.model_illustration_geometry.request.a0".to_owned(),
                source: ModelIllustrationSourceA0::ModelSource(
                    ModelAttachmentIllustrationSourceA0 {
                        kind: "model".to_owned(),
                        attachment: "model".to_owned(),
                        transform: None,
                        material_override: None,
                        tessellation: None,
                    },
                ),
                view: MeshIllustrationView {
                    direction: [0.0, 0.0, 1.0],
                    up: [0.0, 1.0, 0.0],
                    mirror_x: None,
                },
                prepare: None,
                linework: None,
                style: None,
                work_limits: None,
            },
            model.step.to_vec(),
        )
        .expect("one-pass model illustration");
    assert_eq!(result.geometry.length_unit, "millimeter");
    assert!(!result.geometry.surfaces.is_empty());
    service.close().expect("clean Geometer shutdown");
}
