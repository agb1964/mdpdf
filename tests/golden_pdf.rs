//! Golden PDF / визуальные регрессионные тесты (ТЗ §49).
//!
//! §49 прямо запрещает побайтовое сравнение PDF основным критерием ("PDF может
//! содержать меняющиеся служебные данные") и вместо этого перечисляет допустимые
//! структурные проверки: число страниц, размеры страниц, наличие ожидаемых
//! шрифтов, наличие ожидаемых изображений. Этот файл извлекает именно эти факты
//! через `lopdf` и сравнивает их с зафиксированным эталоном в формате JSON.
//!
//! `lopdf` — dev-dependency, а не runtime-зависимость: §49 отдельно оговаривает,
//! что "инструменты для визуального сравнения могут использоваться только в
//! CI/tests", а §3.1 запрещает лишние зависимости в самой утилите mdpdf.
//!
//! Что этот файл НЕ делает: perceptual comparison отрендеренной первой страницы
//! (§49 упоминает его как ещё один допустимый вариант проверки). Это потребовало
//! бы растеризатора PDF (например, pdfium или poppler через системные библиотеки)
//! и инструмента для perceptual diff (например, `image-compare` / `dssim`), что
//! в текущем окружении CI не поднято. Пункт остаётся открытым — см. отчёт задачи.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document as PdfDocument, Object};
use serde::{Deserialize, Serialize};

use mdpdf::compiler::{CompileInput, EmbeddedTypstCompiler, PdfCompiler};
use mdpdf::markdown::parser::MarkdownParser;
use mdpdf::typst_gen::generator::{RenderOptions, TypstGenerator};

const FIXTURES: &[&str] = &["basic", "nesting", "table", "cyrillic", "mermaid"];

/// Структурные факты о PDF, допустимые критерием ТЗ §49: число страниц,
/// размеры страниц, множество встроенных шрифтов, число встроенных изображений.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct PdfFacts {
    page_count: usize,
    /// Размеры страниц в пунктах (round(width), round(height)), по порядку.
    page_sizes: Vec<(i64, i64)>,
    /// Имена встроенных шрифтов (BaseFont) без префикса подмножества (ABCDEF+),
    /// отсортированные и без дублей — конкретный tag подмножества не является
    /// частью контракта и может отличаться между прогонами компилятора.
    embedded_fonts: Vec<String>,
    image_count: usize,
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/markdown")
}

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/expected_pdf")
}

/// Компилирует фикстуру `<name>.md` в PDF, как это делает `tests/compiler.rs`:
/// парсинг Markdown -> генерация Typst -> компиляция встроенным Typst.
/// Базовым каталогом для относительных путей к изображениям (ТЗ §6.3) служит
/// каталог самой фикстуры.
fn compile_fixture(name: &str) -> Vec<u8> {
    let markdown_path = fixture_dir().join(format!("{name}.md"));
    let markdown = std::fs::read_to_string(&markdown_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", markdown_path.display()));
    let base_dir = markdown_path
        .parent()
        .expect("fixture has a parent directory");

    let document = MarkdownParser::default()
        .parse(&markdown)
        .unwrap_or_else(|error| panic!("{name}.md must parse: {error}"));
    let generated = TypstGenerator::new(RenderOptions::default())
        .generate(&document)
        .unwrap_or_else(|error| panic!("{name}.md must generate Typst: {error}"));

    EmbeddedTypstCompiler::new()
        .compile(CompileInput {
            typst_source: &generated.source,
            source_name: "doc.md",
            base_dir,
            resources: &generated.resources,
        })
        .unwrap_or_else(|error| panic!("{name}.md must compile: {error}"))
}

/// Ищет MediaBox страницы, поднимаясь по цепочке `/Parent`, поскольку в PDF
/// он может быть унаследован от узла `/Pages` и отсутствовать в самом
/// словаре страницы.
fn resolve_media_box<'a>(doc: &'a PdfDocument, mut dict: &'a Dictionary) -> &'a Object {
    loop {
        if let Ok(media_box) = dict.get(b"MediaBox") {
            return media_box;
        }
        let parent_id = dict
            .get(b"Parent")
            .and_then(Object::as_reference)
            .expect("page without MediaBox must have a /Parent chain that provides one");
        dict = doc
            .get_dictionary(parent_id)
            .expect("parent of a page must be a readable dictionary");
    }
}

fn page_size_points(doc: &PdfDocument, dict: &Dictionary) -> (i64, i64) {
    let media_box = resolve_media_box(doc, dict)
        .as_array()
        .expect("MediaBox must be an array");
    assert_eq!(media_box.len(), 4, "MediaBox must have four numbers");
    let x0 = media_box[0].as_float().expect("MediaBox x0 is numeric");
    let y0 = media_box[1].as_float().expect("MediaBox y0 is numeric");
    let x1 = media_box[2].as_float().expect("MediaBox x1 is numeric");
    let y1 = media_box[3].as_float().expect("MediaBox y1 is numeric");
    ((x1 - x0).round() as i64, (y1 - y0).round() as i64)
}

/// Отбрасывает префикс подмножества шрифта вида `ABCDEF+` (шесть заглавных
/// латинских букв и `+`), который Typst/krilla добавляют к BaseFont встроенных
/// подмножеств. Тег подмножества не детерминирован по замыслу спецификации
/// PDF (произвольные шесть букв) и не является частью проверяемого контракта.
fn strip_subset_prefix(base_font: &str) -> String {
    let bytes = base_font.as_bytes();
    if bytes.len() > 7
        && bytes[6] == b'+'
        && bytes[..6].iter().all(|byte| byte.is_ascii_uppercase())
    {
        base_font[7..].to_string()
    } else {
        base_font.to_string()
    }
}

/// Извлекает допустимые ТЗ §49 структурные факты из скомпилированного PDF.
fn extract_facts(bytes: &[u8]) -> PdfFacts {
    let doc = PdfDocument::load_mem(bytes).expect("compiled bytes must be a valid PDF");

    let pages: BTreeMap<u32, (u32, u16)> = doc.get_pages();

    let mut page_sizes = Vec::with_capacity(pages.len());
    let mut fonts = std::collections::BTreeSet::new();
    let mut image_count = 0usize;

    for page_id in pages.values() {
        let page_dict = doc
            .get_dictionary(*page_id)
            .expect("page object must be a dictionary");
        page_sizes.push(page_size_points(&doc, page_dict));

        let page_fonts = doc
            .get_page_fonts(*page_id)
            .expect("page fonts must be readable");
        for font_dict in page_fonts.values() {
            if let Ok(base_font) = font_dict.get(b"BaseFont").and_then(Object::as_name) {
                let name = String::from_utf8_lossy(base_font).into_owned();
                fonts.insert(strip_subset_prefix(&name));
            }
        }

        let images = doc
            .get_page_images(*page_id)
            .expect("page images must be readable");
        image_count += images.len();
    }

    PdfFacts {
        page_count: pages.len(),
        page_sizes,
        embedded_fonts: fonts.into_iter().collect(),
        image_count,
    }
}

fn update_golden_requested() -> bool {
    std::env::var_os("MDPDF_UPDATE_GOLDEN").is_some()
}

#[test]
fn pdf_structure_matches_golden_facts() {
    let update = update_golden_requested();

    for name in FIXTURES {
        let bytes = compile_fixture(name);
        let actual = extract_facts(&bytes);
        let golden_path = golden_dir().join(format!("{name}.json"));

        if update {
            let json = serde_json::to_string_pretty(&actual).expect("facts serialise") + "\n";
            std::fs::write(&golden_path, json).expect("golden file is writable");
            continue;
        }

        let expected_json = std::fs::read_to_string(&golden_path).unwrap_or_else(|error| {
            panic!(
                "cannot read {}: {error}\nrun `make golden-update` to create it",
                golden_path.display()
            )
        });
        let expected: PdfFacts =
            serde_json::from_str(&expected_json).expect("golden file is valid JSON");

        assert_eq!(
            actual, expected,
            "PDF structural facts for {name}.md differ from the golden file \
             (run `make golden-update` if the change is intentional)"
        );
    }
}

/// Главный риск, который называет ТЗ §49 — то, что PDF может отличаться между
/// прогонами из-за меняющихся служебных данных. Эталонный JSON фиксирует
/// ожидаемые факты, но не проверяет сам по себе, что два прогона одной и той
/// же фикстуры дают одинаковую структуру. Это отдельная и независимая гарантия
/// от `compilation_is_deterministic` в `tests/compiler.rs`, которая сравнивает
/// байты PDF целиком: там, где байты могут различаться (см. комментарий §49 о
/// служебных данных), извлечённые структурные факты обязаны совпадать.
#[test]
fn pdf_structural_facts_are_deterministic_across_runs() {
    for name in FIXTURES {
        let first = extract_facts(&compile_fixture(name));
        let second = extract_facts(&compile_fixture(name));
        assert_eq!(
            first, second,
            "structural facts for {name}.md differ between two compilations of the same input"
        );
    }
}

/// Компилирует произвольный Markdown с заданными параметрами рендеринга.
fn compile_with(markdown: &str, options: RenderOptions) -> Vec<u8> {
    let document = MarkdownParser::default()
        .parse(markdown)
        .expect("markdown parses");
    let generated = TypstGenerator::new(options)
        .generate(&document)
        .expect("typst generates");
    let dir = tempfile::tempdir().expect("temp dir");

    EmbeddedTypstCompiler::new()
        .compile(CompileInput {
            typst_source: &generated.source,
            source_name: "doc.md",
            base_dir: dir.path(),
            resources: &generated.resources,
        })
        .expect("document compiles")
}

/// Все четыре фикстуры умещаются на одну страницу, поэтому сам по себе
/// эталонный `page_count` ничего бы не поймал. Здесь проверяется, что счётчик
/// действительно считает: длинный документ обязан дать несколько страниц (§49).
#[test]
fn page_count_grows_with_the_document() {
    let long_document: String = (1..=40)
        .map(|section| {
            format!("## Раздел {section}\n\nАбзац раздела {section}. Съешь ещё этих мягких французских булок да выпей чаю.\n\n")
        })
        .collect();

    let facts = extract_facts(&compile_with(&long_document, RenderOptions::default()));
    assert!(
        facts.page_count > 1,
        "long document produced {} page(s)",
        facts.page_count
    );
}

/// Подписи диаграммы живут внутри SVG, а рисует их Typst встроенным шрифтом
/// (ТЗ §10.5, §34). Если бы семейство не разрешилось, usvg **молча** выбросил
/// бы текстовые узлы: PDF собрался бы, диаграмма выглядела бы как набор пустых
/// рамок, и ни один структурный факт этого бы не заметил.
///
/// Документ намеренно состоит из одной диаграммы и не содержит собственного
/// текста — иначе шрифт попал бы в PDF из заголовка, а не из подписей.
#[test]
fn cyrillic_diagram_labels_reach_the_pdf() {
    let markdown = "```mermaid\ngraph TD\n    A[Начало] --> B[Конец]\n```\n";
    let facts = extract_facts(&compile_with(markdown, RenderOptions::default()));

    assert!(
        facts
            .embedded_fonts
            .iter()
            .any(|font| font.contains("NotoSans")),
        "в PDF из одной диаграммы нет встроенного шрифта — подписи потеряны: {:?}",
        facts.embedded_fonts
    );
}

/// Typst рисует SVG векторно, а не как Image XObject, поэтому диаграммы
/// не увеличивают `image_count`. Тест фиксирует это как ожидаемое свойство:
/// иначе эталон `image_count: 0` для `mermaid.md` выглядел бы ошибкой.
#[test]
fn diagrams_are_vector_content_not_image_xobjects() {
    let markdown = "```mermaid\ngraph TD\n    A --> B\n```\n";
    let facts = extract_facts(&compile_with(markdown, RenderOptions::default()));
    assert_eq!(facts.image_count, 0);
}

/// Размер страницы из `--paper` должен доходить до PDF, а не оставаться
/// параметром на бумаге. A4 — 595×842 pt, US Letter — 612×792 pt (§20.1).
#[test]
fn paper_size_reaches_the_pdf() {
    let markdown = "# Заголовок\n\nТекст.\n";

    let a4 = extract_facts(&compile_with(markdown, RenderOptions::default()));
    assert_eq!(a4.page_sizes, vec![(595, 842)]);

    let letter = extract_facts(&compile_with(
        markdown,
        RenderOptions {
            paper: mdpdf::typst_gen::generator::PaperSize::Letter,
            ..RenderOptions::default()
        },
    ));
    assert_eq!(letter.page_sizes, vec![(612, 792)]);
}

/// Моноширинный шрифт обязан попадать в PDF, когда в документе есть код.
/// Регрессия: `raw` ставит шрифт собственным show-правилом, и обёртка
/// `text(font: ..)` его не перебивала — код молча выводился основным шрифтом,
/// а `NotoSansMono-Regular` в PDF не встраивался вовсе.
#[test]
fn code_blocks_embed_the_monospace_font() {
    let facts = extract_facts(&compile_with(
        "Абзац с `inline` кодом.\n\n```rust\nfn main() {}\n```\n",
        RenderOptions::default(),
    ));
    assert!(
        facts
            .embedded_fonts
            .iter()
            .any(|font| font.contains("Mono")),
        "monospace font is missing from the PDF: {:?}",
        facts.embedded_fonts
    );
}

/// Эмодзи не покрыты ни одним начертанием Noto Sans, а системные шрифты
/// запрещены §32 — без встроенного Noto Color Emoji они молча исчезали
/// из PDF: код возврата 0, сообщение об успехе, пропавшее содержимое.
///
/// Проверка идёт по сырым байтам, а не через `PdfFacts`: цветной растровый
/// шрифт попадает в PDF как Type3, глифы которого нарисованы встроенными
/// изображениями. Ни `get_page_fonts`, ни `get_page_images` такой шрифт
/// не показывают — он не значится ни обычным `BaseFont`, ни XObject страницы.
#[test]
fn emoji_reach_the_pdf() {
    let with_emoji = compile_with(
        "# Отметки\n\n🔴 срочно 🟡 позже 🟢 к запуску\n",
        RenderOptions::default(),
    );
    let without_emoji = compile_with("# Отметки\n\nсрочно позже\n", RenderOptions::default());

    let haystack = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let with_emoji = haystack(&with_emoji);
    let without_emoji = haystack(&without_emoji);

    assert!(
        with_emoji.contains("NotoColorEmoji"),
        "emoji font did not reach the PDF"
    );
    assert!(
        with_emoji.contains("/Type3"),
        "colour emoji are expected as a Type3 font with bitmap glyphs"
    );
    // Контроль: документ без эмодзи эмодзи-шрифт не тянет, то есть проверка
    // выше действительно реагирует на содержимое, а не на факт встраивания.
    assert!(
        !without_emoji.contains("NotoColorEmoji"),
        "emoji font must not be embedded when the document has no emoji"
    );
}
