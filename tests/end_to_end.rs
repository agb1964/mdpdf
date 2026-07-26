//! Сквозные сценарии (ТЗ §48).

use std::path::{Path, PathBuf};

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

/// Файл существует, начинается с `%PDF-` и не выглядит обрезанным (ТЗ §39).
fn assert_is_pdf(path: &Path) {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("{} must exist: {error}", path.display()));
    assert!(
        bytes.starts_with(b"%PDF-"),
        "{} is not a PDF",
        path.display()
    );
    assert!(bytes.len() > 512, "{} looks truncated", path.display());
}

/// Временные файлы атомарной записи не переживают запуск (ТЗ §6.4).
fn assert_no_leftovers(directory: &Path) {
    let leftovers: Vec<String> = std::fs::read_dir(directory)
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp") || name.starts_with('.'))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporary files remained: {leftovers:?}"
    );
}

// --- основной сценарий (ТЗ §61) -----------------------------------------------

#[test]
fn a_document_becomes_a_pdf_next_to_the_source() {
    let (dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .assert()
        .success()
        .stdout(contains("Created"));

    assert_is_pdf(&dir.path().join("doc.pdf"));
    assert_no_leftovers(dir.path());
}

#[test]
fn an_explicit_output_path_is_honoured() {
    let (dir, input) = document("# Заголовок\n");
    let output = dir.path().join("report.pdf");

    mdpdf()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .assert()
        .success();

    assert_is_pdf(&output);
    assert!(!dir.path().join("doc.pdf").exists());
}

#[test]
fn every_supported_construct_survives_the_whole_pipeline() {
    let (dir, input) = document(
        "\
# Заголовок

Абзац с **жирным**, *курсивом*, ~~зачёркнутым~~ и `кодом`.

- пункт
  1. вложенный
- [x] сделано

> цитата

| a | b |
|---|---|
| 1 | 2 |

```rust
fn main() {}
```

[ссылка](https://example.com)

---
",
    );

    mdpdf().arg(&input).assert().success();
    assert_is_pdf(&dir.path().join("doc.pdf"));
}

#[test]
fn quiet_suppresses_the_message_but_not_the_pdf() {
    let (dir, input) = document("# Заголовок\n");

    mdpdf()
        .arg(&input)
        .arg("--quiet")
        .assert()
        .success()
        .stdout("");

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

// --- перезапись (ТЗ §6.2, §48.20) ---------------------------------------------

#[test]
fn an_existing_pdf_is_never_overwritten_silently() {
    let (dir, input) = document("# Заголовок\n");
    let output = dir.path().join("doc.pdf");
    std::fs::write(&output, "старое содержимое").expect("seed output");

    mdpdf()
        .arg(&input)
        .assert()
        .code(8)
        .stderr(contains("already exists"));

    // Существующий файл не должен пострадать.
    assert_eq!(
        std::fs::read_to_string(&output).expect("read back"),
        "старое содержимое"
    );
    assert_no_leftovers(dir.path());
}

#[test]
fn overwrite_replaces_the_existing_pdf() {
    let (dir, input) = document("# Заголовок\n");
    let output = dir.path().join("doc.pdf");
    std::fs::write(&output, "старое содержимое").expect("seed output");

    mdpdf().arg(&input).arg("--overwrite").assert().success();

    assert_is_pdf(&output);
    assert_no_leftovers(dir.path());
}

// --- stdin (ТЗ §5.3) ----------------------------------------------------------

#[test]
fn stdin_produces_a_pdf_at_the_requested_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let output = dir.path().join("out.pdf");

    mdpdf()
        .arg("-")
        .arg("--output")
        .arg(&output)
        .write_stdin("# Привет\n\nТекст из потока.\n")
        .assert()
        .success();

    assert_is_pdf(&output);
    assert_no_leftovers(dir.path());
}

// --- диагностические режимы ---------------------------------------------------

#[test]
fn check_never_writes_a_pdf() {
    let (dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .arg("--check")
        .assert()
        .success()
        .stdout(contains("Checked"));

    assert!(!dir.path().join("doc.pdf").exists());
    assert_no_leftovers(dir.path());
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
    assert_eq!(
        parsed["blocks"].as_array().map(Vec::len),
        Some(2),
        "expected a heading and a paragraph"
    );

    // Диагностический режим без --output не создаёт PDF (ТЗ §5.5, §5.6).
    assert!(!dir.path().join("doc.pdf").exists());
}

#[test]
fn emit_typst_writes_the_generated_source() {
    let (dir, input) = document("# Заголовок\n");
    let typst = dir.path().join("out.typ");

    mdpdf()
        .arg(&input)
        .arg("--emit-typst")
        .arg(&typst)
        .assert()
        .success();

    let written = std::fs::read_to_string(&typst).expect("Typst file exists");
    assert!(written.contains("#show: mdpdf-document.with("));
    assert!(written.contains("heading(level: 1"));
    assert!(!dir.path().join("doc.pdf").exists());
}

#[test]
fn emit_alongside_output_still_produces_a_pdf() {
    let (dir, input) = document("# Заголовок\n");
    let typst = dir.path().join("out.typ");
    let output = dir.path().join("out.pdf");

    mdpdf()
        .arg(&input)
        .arg("--emit-typst")
        .arg(&typst)
        .arg("-o")
        .arg(&output)
        .assert()
        .success();

    assert!(typst.exists());
    assert_is_pdf(&output);
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

// --- политика доступа к ресурсам (ТЗ §33.2) -----------------------------------

#[test]
fn an_image_outside_the_document_directory_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let inner = dir.path().join("doc");
    std::fs::create_dir(&inner).expect("create dir");
    std::fs::write(dir.path().join("secret.png"), b"\x89PNG\r\n\x1a\n").expect("write outside");

    let input = inner.join("doc.md");
    std::fs::write(&input, "![наружу](../secret.png)\n").expect("write fixture");

    mdpdf()
        .arg(&input)
        .assert()
        .code(9)
        .stderr(contains("not allowed"));

    assert!(!inner.join("doc.pdf").exists());
    assert_no_leftovers(&inner);
}

#[test]
fn a_missing_image_stops_the_pipeline_before_writing() {
    let (dir, input) = document("![нет](nope.png)\n");

    mdpdf().arg(&input).assert().code(7);

    assert!(!dir.path().join("doc.pdf").exists());
    assert_no_leftovers(dir.path());
}

// --- изображения SVG (ТЗ §33) --------------------------------------------------

/// Сквозной прогон с настоящим SVG-изображением (не только PNG, как в
/// остальных сценариях выше) — раздел 6 code-analysis-2026-07-26.md.
#[test]
fn a_document_with_a_real_svg_image_compiles() {
    let dir = tempfile::tempdir().expect("temp dir");
    let images = dir.path().join("images");
    std::fs::create_dir(&images).expect("create dir");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/images/dot.svg"),
        images.join("dot.svg"),
    )
    .expect("copy fixture");

    let input = dir.path().join("doc.md");
    std::fs::write(&input, "# Заголовок\n\n![точка](images/dot.svg)\n").expect("write fixture");

    mdpdf().arg(&input).assert().success();

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

// --- предупреждения Typst (ТЗ §38) ---------------------------------------------

/// SVG с `<foreignObject>` — Typst не отказывается его встраивать, но
/// предупреждает, что элемент может отрисоваться неверно
/// (`typst-library`, `visualize/image/mod.rs`). Предупреждение обязано дойти
/// до stderr, а PDF всё равно должен быть создан (ТЗ §38); эта ветка
/// (`CompiledPdf.warnings` непустой) раньше не тестировалась
/// (раздел 6 code-analysis-2026-07-26.md).
#[test]
fn a_typst_warning_reaches_stderr_and_the_pdf_is_still_created() {
    let dir = tempfile::tempdir().expect("temp dir");
    let images = dir.path().join("images");
    std::fs::create_dir(&images).expect("create dir");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/images/foreign-object.svg"),
        images.join("foreign-object.svg"),
    )
    .expect("copy fixture");

    let input = dir.path().join("doc.md");
    std::fs::write(
        &input,
        "# Заголовок\n\n![картинка](images/foreign-object.svg)\n",
    )
    .expect("write fixture");

    mdpdf()
        .arg(&input)
        .assert()
        .success()
        .stderr(contains("warning"))
        .stderr(contains("foreign object"));

    assert_is_pdf(&dir.path().join("doc.pdf"));
}
