//! Тесты поверхности CLI (ТЗ §5, §43, §44).

use assert_cmd::Command;
use predicates::str::contains;

fn mdpdf() -> Command {
    Command::cargo_bin("mdpdf").expect("binary is built")
}

#[test]
fn help_lists_every_documented_option() {
    let assert = mdpdf().arg("--help").assert().success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    for flag in [
        "--output",
        "--title",
        "--author",
        "--paper",
        "--margin",
        "--font-size",
        "--toc",
        "--heading-numbers",
        "--check",
        "--emit-ast",
        "--emit-typst",
        "--overwrite",
        "--quiet",
        "--verbose",
    ] {
        assert!(stdout.contains(flag), "help is missing {flag}");
    }
}

#[test]
fn version_reports_the_package_version() {
    mdpdf()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn missing_input_exits_with_cli_error() {
    mdpdf().assert().code(2);
}

#[test]
fn unknown_flag_exits_with_cli_error() {
    mdpdf().args(["input.md", "--nope"]).assert().code(2);
}

#[test]
fn stdin_without_output_exits_with_cli_error() {
    mdpdf().arg("-").write_stdin("# test").assert().code(2);
}

#[test]
fn missing_file_exits_with_input_error() {
    mdpdf()
        .arg("definitely-missing-file.md")
        .assert()
        .code(3)
        .stderr(contains("cannot read"));
}

#[test]
fn nul_byte_in_input_exits_with_input_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("bad.md");
    std::fs::write(&path, b"text\0more").expect("write fixture");

    mdpdf().arg(&path).assert().code(3);
}
