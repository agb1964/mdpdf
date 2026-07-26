//! Тесты этапа 3 (ТЗ §41).

use std::path::{Path, PathBuf};

use mdpdf::compiler::error::CompileError;
use mdpdf::compiler::{CompileInput, EmbeddedTypstCompiler, PdfCompiler, fonts, limits, pdf};
use mdpdf::markdown::parser::MarkdownParser;
use mdpdf::typst_gen::generator::{GeneratedTypst, RenderOptions, TypstGenerator};

fn generate(markdown: &str) -> GeneratedTypst {
    let document = MarkdownParser::default()
        .parse(markdown)
        .expect("markdown parses");
    TypstGenerator::new(RenderOptions::default())
        .generate(&document)
        .expect("typst generates")
}

fn compile_in(markdown: &str, base_dir: &Path) -> Result<Vec<u8>, CompileError> {
    let generated = generate(markdown);
    EmbeddedTypstCompiler::new().compile(CompileInput {
        typst_source: &generated.source,
        source_name: "doc.md",
        base_dir,
        resources: &generated.resources,
    })
}

fn compile(markdown: &str) -> Vec<u8> {
    let dir = tempfile::tempdir().expect("temp dir");
    compile_in(markdown, dir.path()).expect("document compiles")
}

/// Готовый PNG-фикстур: писать кодировщик PNG в тестах незачем.
fn png() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/images/schema.png");
    std::fs::read(path).expect("image fixture is readable")
}

// --- шрифты (ТЗ §34) ----------------------------------------------------------

#[test]
fn all_five_fonts_are_embedded() {
    assert_eq!(fonts::EMBEDDED_FONTS.len(), 5);
    for font in fonts::EMBEDDED_FONTS {
        assert!(font.len() > 1024, "embedded font looks truncated");
    }
}

#[test]
fn font_registration_order_is_deterministic() {
    assert_eq!(fonts::EMBEDDED_FONTS[0], fonts::NOTO_SANS_REGULAR);
    assert_eq!(fonts::EMBEDDED_FONTS[4], fonts::NOTO_SANS_MONO_REGULAR);
}

// --- компиляция (ТЗ §35, §39, §41) --------------------------------------------

#[test]
fn a_pdf_is_produced_without_typst_installed() {
    let bytes = compile("# Заголовок\n\nТекст.\n");
    assert!(pdf::looks_like_pdf(&bytes));
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn an_empty_document_still_produces_a_pdf() {
    assert!(pdf::looks_like_pdf(&compile("")));
}

#[test]
fn every_supported_construct_compiles() {
    let markdown = "\
# Заголовок

Абзац с **жирным**, *курсивом*, ~~зачёркнутым~~ и `кодом`.

- пункт
- вложенный:
  1. один
  2. два
- [x] сделано
- [ ] не сделано

> цитата
>
> > вложенная

| Лево | Центр | Право |
|:-----|:-----:|------:|
| a    | b     | c     |

```rust
fn main() {}
```

[ссылка](https://example.com)

---
";
    assert!(pdf::looks_like_pdf(&compile(markdown)));
}

#[test]
fn cyrillic_reaches_the_pdf() {
    let bytes = compile("# Ёжик и «кавычки»\n\nСъешь ещё этих мягких французских булок.\n");
    assert!(pdf::looks_like_pdf(&bytes));
    // Кириллица кодируется в потоках PDF, поэтому проверяется факт наличия
    // встроенного шрифта Noto, а не байты текста.
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("NotoSans"),
        "embedded font is missing from the PDF"
    );
}

#[test]
fn golden_fixtures_compile() {
    for name in ["basic", "nesting", "table", "cyrillic"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/markdown")
            .join(format!("{name}.md"));
        let markdown = std::fs::read_to_string(&path).expect("fixture is readable");
        // Пути изображений разрешаются относительно каталога документа (ТЗ §6.3),
        // поэтому базовым каталогом служит сам каталог фикстур.
        let base_dir = path.parent().expect("fixture has a parent directory");
        let bytes = compile_in(&markdown, base_dir)
            .unwrap_or_else(|error| panic!("{name}.md must compile: {error}"));
        assert!(pdf::looks_like_pdf(&bytes), "{name}.md produced no PDF");
    }
}

#[test]
fn injection_payloads_never_execute_during_compilation() {
    for payload in [
        "#panic(\"injected\")",
        "#import \"@preview/package\"",
        "#include \"secret\"",
        "#let evil = 1",
        "$ x + y $",
    ] {
        let markdown = format!("# {payload}\n\n{payload}\n");
        let bytes = compile(&markdown);
        assert!(
            pdf::looks_like_pdf(&bytes),
            "payload broke compilation: {payload}"
        );
    }
}

// --- изображения и политика доступа (ТЗ §33) ----------------------------------

#[test]
fn a_local_image_is_embedded() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("images")).expect("create dir");
    std::fs::write(dir.path().join("images/dot.png"), png()).expect("write image");

    let bytes =
        compile_in("![точка](images/dot.png)\n", dir.path()).expect("document with image compiles");
    assert!(pdf::looks_like_pdf(&bytes));
}

#[test]
fn path_traversal_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let inner = dir.path().join("doc");
    std::fs::create_dir(&inner).expect("create dir");
    std::fs::write(dir.path().join("secret.png"), png()).expect("write outside");

    let err = compile_in("![наружу](../secret.png)\n", &inner).expect_err("traversal is refused");
    assert!(matches!(err, CompileError::ResourceAccess { .. }));
}

#[test]
fn a_missing_image_is_reported_clearly() {
    let dir = tempfile::tempdir().expect("temp dir");
    let err = compile_in("![нет](nope.png)\n", dir.path()).expect_err("missing image");
    match err {
        CompileError::Image {
            path,
            span,
            message,
        } => {
            assert_eq!(path, "nope.png");
            assert!(span.is_some(), "image errors carry the Markdown span");
            assert!(message.contains("cannot read"));
        }
        other => panic!("expected an image error, got {other:?}"),
    }
}

#[test]
fn a_file_that_is_not_an_image_is_refused_before_typst() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("fake.png"), b"not an image at all").expect("write");
    let err = compile_in("![подделка](fake.png)\n", dir.path()).expect_err("bad format");
    assert!(matches!(err, CompileError::Image { .. }));
}

#[test]
fn network_images_never_reach_the_compiler() {
    // Отсекается ещё валидацией AST (ТЗ §10.12), до этапа компиляции.
    let result = MarkdownParser::default().parse("![сеть](https://example.com/a.png)\n");
    assert!(result.is_err());
}

// --- изоляция окружения (ТЗ §32) ----------------------------------------------

#[test]
fn the_document_cannot_read_arbitrary_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    // Пользовательский Markdown не может выразить include/read, но проверяем,
    // что даже прямой Typst-вызов в тексте остаётся текстом.
    let bytes = compile_in("#read(\"/etc/passwd\")\n", dir.path()).expect("stays literal text");
    assert!(pdf::looks_like_pdf(&bytes));
}

#[test]
fn resource_limits_match_specification() {
    assert_eq!(limits::MAX_AST_NODES, 1_000_000);
    assert_eq!(limits::MAX_NESTING_DEPTH, 128);
    assert_eq!(limits::MAX_IMAGES, 1_000);
    assert_eq!(limits::MAX_IMAGE_BYTES, 64 * 1024 * 1024);
    assert_eq!(limits::MAX_TOTAL_IMAGE_BYTES, 256 * 1024 * 1024);
    assert_eq!(limits::MAX_URL_BYTES, 16 * 1024);
    assert_eq!(limits::MAX_TEXT_NODE_BYTES, 16 * 1024 * 1024);
}

#[test]
fn compilation_is_deterministic() {
    let markdown = "# Заголовок\n\nТекст со ссылкой [сюда](a.md).\n";
    let first = compile(markdown);
    let second = compile(markdown);
    assert_eq!(
        first.len(),
        second.len(),
        "PDF size differs between identical runs"
    );
}
