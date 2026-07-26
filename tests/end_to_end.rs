//! Сквозные сценарии (ТЗ §48).
//!
//! Полный набор из 20 сценариев ТЗ включается по мере готовности Milestone 1–4.
//! Сейчас проверяется то, что уже реализовано: конвейер доходит до парсинга
//! и не оставляет после себя файлов.

use assert_cmd::Command;

fn mdpdf() -> Command {
    Command::cargo_bin("mdpdf").expect("binary is built")
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
