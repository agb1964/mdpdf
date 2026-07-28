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

// --- сквозные проверки флагов рендеринга (ТЗ §20, §48) -------------------------
//
// `help_lists_every_documented_option` выше проверяет только присутствие флага
// в `--help`; здесь каждый флаг прогоняется через реальный бинарь на маленьком
// документе, чтобы убедиться, что он действительно доходит до конвейера и не
// ломает компиляцию (раздел 6 code-analysis-2026-07-26.md).

/// Каталог с готовым `doc.md` внутри.
fn document(markdown: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("doc.md");
    std::fs::write(&input, markdown).expect("write fixture");
    (dir, input)
}

/// Файл существует и начинается с сигнатуры `%PDF-`.
fn assert_is_pdf(path: &std::path::Path) {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("{} must exist: {error}", path.display()));
    assert!(
        bytes.starts_with(b"%PDF-"),
        "{} is not a PDF",
        path.display()
    );
}

/// Документ с несколькими заголовками разного уровня — нужен для
/// `--toc` и `--heading-numbers`, где одного заголовка недостаточно, чтобы
/// увидеть эффект флага.
const MULTI_HEADING_DOCUMENT: &str = "\
# Первый заголовок

Текст первого раздела.

## Второй заголовок

Текст второго раздела.

### Третий заголовок

Текст третьего раздела.
";

#[test]
fn paper_letter_produces_a_pdf() {
    let (dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .arg("--paper")
        .arg("letter")
        .assert()
        .success();

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

#[test]
fn toc_with_several_headings_produces_a_pdf() {
    let (dir, input) = document(MULTI_HEADING_DOCUMENT);

    mdpdf().arg(&input).arg("--toc").assert().success();

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

#[test]
fn heading_numbers_with_several_headings_produces_a_pdf() {
    let (dir, input) = document(MULTI_HEADING_DOCUMENT);

    mdpdf()
        .arg(&input)
        .arg("--heading-numbers")
        .assert()
        .success();

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

#[test]
fn margin_override_produces_a_pdf() {
    let (dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .arg("--margin")
        .arg("15mm")
        .assert()
        .success();

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

#[test]
fn font_size_override_produces_a_pdf() {
    let (dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .arg("--font-size")
        .arg("13pt")
        .assert()
        .success();

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

#[test]
fn verbose_prints_pipeline_diagnostics_to_stderr() {
    let (dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .arg("--verbose")
        .assert()
        .success()
        .stderr(contains("mdpdf:"));

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

/// Символ без глифа не превращается в замещающий прямоугольник — он молча
/// исчезает из PDF. Без предупреждения пользователь узнаёт о потере, только
/// сверив результат с исходником (ТЗ §38).
#[test]
fn characters_without_glyphs_are_reported_but_do_not_stop_the_pipeline() {
    // U+E000 — область частного использования: глифа нет ни в одном
    // из встроенных шрифтов.
    let (dir, input) = document("# Заголовок\n\nтекст \u{E000} дальше\n");

    mdpdf()
        .arg(&input)
        .assert()
        .success()
        .stderr(contains("no glyph"))
        .stderr(contains("U+E000"));

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

#[test]
fn emoji_do_not_trigger_the_missing_glyph_warning() {
    // Регрессия: ради этих символов и встроен Noto Color Emoji.
    let (dir, input) = document("# Отметки\n\n🔴 срочно 🟡 позже 🟢 к запуску\n");

    let assert = mdpdf().arg(&input).assert().success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        !stderr.contains("no glyph"),
        "emoji reported as missing: {stderr}"
    );

    assert_is_pdf(&dir.path().join("doc.pdf"));
}

// --- невалидные значения длины (ТЗ §20.2, код завершения 6) --------------------
//
// Диапазоны проверяются юнит-тестами в `src/typst_gen/generator.rs`, но через
// бинарь путь «CLI → AppConfig → RenderOptions::validate» раньше не
// прогонялся (раздел 6 code-analysis-2026-07-26.md).

#[test]
fn margin_larger_than_half_the_page_exits_with_typst_generation_error() {
    let (_dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .arg("--margin")
        .arg("300mm")
        .assert()
        .code(6)
        .stderr(contains("margin"))
        .stderr(contains("exceeds half"));
}

#[test]
fn font_size_out_of_range_exits_with_typst_generation_error() {
    let (_dir, input) = document("# Заголовок\n\nТекст.\n");

    mdpdf()
        .arg(&input)
        .arg("--font-size")
        .arg("3pt")
        .assert()
        .code(6)
        .stderr(contains("font size"))
        .stderr(contains("outside"));
}
