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
            .contains("design (design-review, dr)")
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
fn usage_errors_follow_the_python_cli_stream_and_exit_contract() {
    let output = binary()
        .args(["design", "--not-a-real-option"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("usage: kicad-cruncher"));
    assert!(stderr.contains("kicad-cruncher: error: unrecognized arguments: --not-a-real-option"));
}

#[test]
fn design_runtime_failure_returns_one_without_false_success() {
    let output = binary()
        .args(["design", "project.kicad_pro"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("could not resolve design input")
    );
}
