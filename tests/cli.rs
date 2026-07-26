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
fn ast_validation_exits_with_its_own_code_and_position() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "# Заголовок\n\n![сеть](https://example.com/a.png)\n")
        .expect("write fixture");

    // Ошибка валидации AST имеет собственный код 5, отличный от кода Markdown 4
    // (ТЗ §43), и несёт позицию в исходнике (ТЗ §16).
    mdpdf()
        .arg(&path)
        .arg("--check")
        .assert()
        .code(5)
        .stderr(contains("doc.md:3:1:"))
        .stderr(contains("network image source is not allowed"));
}

#[test]
fn markdown_and_validation_errors_use_different_codes() {
    let dir = tempfile::tempdir().expect("temp dir");

    let unsupported = dir.path().join("html.md");
    std::fs::write(&unsupported, "текст <b>жирный</b>\n").expect("write fixture");
    mdpdf().arg(&unsupported).arg("--check").assert().code(4);

    let invalid = dir.path().join("image.md");
    std::fs::write(&invalid, "![сеть](http://example.com/a.png)\n").expect("write fixture");
    mdpdf().arg(&invalid).arg("--check").assert().code(5);
}

#[test]
fn nul_byte_in_input_exits_with_input_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("bad.md");
    std::fs::write(&path, b"text\0more").expect("write fixture");

    mdpdf().arg(&path).assert().code(3);
}
