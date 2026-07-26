//! Сквозные сценарии (ТЗ §48).
//!
//! Полный набор из 20 сценариев ТЗ включается по мере готовности Milestone 1–4.
//! Сейчас проверяется то, что уже реализовано: конвейер доходит до парсинга
//! и не оставляет после себя файлов.

use assert_cmd::Command;
use predicates::str::contains;

fn mdpdf() -> Command {
    Command::cargo_bin("mdpdf").expect("binary is built")
}

/// Каталог с готовым `doc.md` внутри.
fn document(markdown: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("doc.md");
    std::fs::write(&input, markdown).expect("write fixture");
    (dir, input)
}

#[test]
fn pipeline_reaches_the_parser_and_writes_nothing_yet() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("doc.md");
    std::fs::write(&input, "# Заголовок\n\nТекст.\n").expect("write fixture");

    mdpdf().arg(&input).assert().failure();

    // PDF ещё не реализован, но и мусора после себя утилита не оставляет.
    assert!(!dir.path().join("doc.pdf").exists());
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read temp dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .filter(|name| name != "doc.md")
        .collect();
    assert!(leftovers.is_empty(), "unexpected leftovers: {leftovers:?}");
}

#[test]
fn emit_ast_writes_valid_json_and_succeeds() {
    let (dir, input) = document("# Заголовок\n\nТекст со [ссылкой](a.md).\n");
    let ast = dir.path().join("ast.json");

    mdpdf()
        .arg(&input)
        .arg("--emit-ast")
        .arg(&ast)
        .assert()
        .success()
        .stdout(contains("Created"));

    let written = std::fs::read_to_string(&ast).expect("AST file exists");
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("AST file is valid JSON");
    assert!(parsed.get("metadata").is_some(), "metadata is missing");
    assert_eq!(
        parsed["blocks"].as_array().map(Vec::len),
        Some(2),
        "expected a heading and a paragraph"
    );
    assert!(written.ends_with('\n'), "file must end with a newline");

    // Диагностический режим без --output не создаёт PDF (ТЗ §5.5, §5.6).
    assert!(!dir.path().join("doc.pdf").exists());
}

#[test]
fn quiet_suppresses_the_success_message() {
    let (dir, input) = document("# Заголовок\n");
    let ast = dir.path().join("ast.json");

    mdpdf()
        .arg(&input)
        .arg("--emit-ast")
        .arg(&ast)
        .arg("--quiet")
        .assert()
        .success()
        .stdout("");

    assert!(ast.exists(), "--quiet must not disable the actual work");
}

#[test]
fn emit_ast_to_an_unwritable_path_exits_with_output_error() {
    let (dir, input) = document("# Заголовок\n");
    let unwritable = dir.path().join("missing-subdir").join("ast.json");

    mdpdf()
        .arg(&input)
        .arg("--emit-ast")
        .arg(&unwritable)
        .assert()
        .code(8)
        .stderr(contains("cannot write"));

    assert!(!unwritable.exists());
}

#[test]
fn check_never_writes_a_pdf() {
    let (dir, input) = document("# Заголовок\n\nТекст.\n");

    // Код завершения станет нулевым, когда появится компилятор (ТЗ §5.4);
    // инвариант, который обязан держаться всегда, — PDF не создаётся.
    let _ = mdpdf().arg(&input).arg("--check").assert();

    assert!(!dir.path().join("doc.pdf").exists());
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read temp dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .filter(|name| name != "doc.md")
        .collect();
    assert!(leftovers.is_empty(), "unexpected leftovers: {leftovers:?}");
}

#[test]
fn stdin_input_is_accepted() {
    let dir = tempfile::tempdir().expect("temp dir");
    let output = dir.path().join("out.pdf");

    mdpdf()
        .arg("-")
        .arg("--output")
        .arg(&output)
        .write_stdin("# Привет\n")
        .assert()
        .failure();

    assert!(!output.exists());
}
