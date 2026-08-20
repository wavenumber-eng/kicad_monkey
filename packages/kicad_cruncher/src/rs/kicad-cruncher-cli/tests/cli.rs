use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kicad-cruncher"))
}

#[test]
fn executable_reports_help_and_version_without_python() {
    let help = binary().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("design, design-review, dr")
    );

    let version = binary().arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with("kicad-cruncher ")
    );
}

#[test]
fn unpromoted_design_command_cannot_report_false_success() {
    let output = binary()
        .args(["design", "project.kicad_pro"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("still under migration")
    );
}
