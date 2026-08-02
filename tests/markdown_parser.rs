//! Тесты этапа 1 (ТЗ §17, §46).

use mdpdf::ast::block::{Alignment, Block, HeadingLevel, ListKind};
use mdpdf::ast::document::Document;
use mdpdf::ast::inline::Inline;
use mdpdf::markdown::error::MarkdownError;
use mdpdf::markdown::parser::MarkdownParser;
use mdpdf::source;

fn parse(source: &str) -> Document {
    MarkdownParser::default()
        .parse(source)
        .expect("document parses")
}

fn parse_err(source: &str) -> MarkdownError {
    MarkdownParser::default()
        .parse(source)
        .expect_err("document must not parse")
}

fn block(document: &Document, index: usize) -> &Block {
    &document.blocks[index].value
}

// --- нормализация входа (ТЗ §13) --------------------------------------------

#[test]
fn input_normalization_matches_specification() {
    let text = source::decode_and_normalize("\u{feff}# Заголовок\r\nтекст\rконец".as_bytes())
        .expect("valid input");
    assert_eq!(text, "# Заголовок\nтекст\nконец");
}

#[test]
fn empty_markdown_is_a_valid_document() {
    assert_eq!(source::decode_and_normalize(b"").expect("valid input"), "");
    assert!(parse("").is_empty());
}

// --- узлы (ТЗ §10) -----------------------------------------------------------

#[test]
fn every_heading_level_is_recognised() {
    let document = parse("# a\n\n## b\n\n### c\n\n#### d\n\n##### e\n\n###### f\n");
    let levels: Vec<HeadingLevel> = document
        .blocks
        .iter()
        .filter_map(|block| match &block.value {
            Block::Heading(heading) => Some(heading.level),
            _ => None,
        })
        .collect();
    assert_eq!(
        levels,
        vec![
            HeadingLevel::H1,
            HeadingLevel::H2,
            HeadingLevel::H3,
            HeadingLevel::H4,
            HeadingLevel::H5,
            HeadingLevel::H6,
        ]
    );
}

#[test]
fn heading_attributes_produce_an_id() {
    let document = parse("# Заголовок {#custom-id}\n");
    match block(&document, 0) {
        Block::Heading(heading) => assert_eq!(heading.id.as_deref(), Some("custom-id")),
        other => panic!("expected heading, got {other:?}"),
    }
}

#[test]
fn inline_formatting_is_preserved() {
    let document = parse("текст **жирный** *курсив* ~~зачёркнутый~~ `код`\n");
    let Block::Paragraph(paragraph) = block(&document, 0) else {
        panic!("expected paragraph");
    };
    let kinds: Vec<&str> = paragraph
        .content
        .iter()
        .map(|inline| match inline {
            Inline::Text(_) => "text",
            Inline::Strong(_) => "strong",
            Inline::Emphasis(_) => "emphasis",
            Inline::Strikethrough(_) => "strike",
            Inline::Code(_) => "code",
            Inline::SoftBreak => "soft",
            Inline::HardBreak => "hard",
            Inline::Link(_) => "link",
            Inline::Image(_) => "image",
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "text", "strong", "text", "emphasis", "text", "strike", "text", "code"
        ]
    );
}

#[test]
fn soft_and_hard_breaks_are_distinguished() {
    let document = parse("первая\nвторая  \nтретья\n");
    let Block::Paragraph(paragraph) = block(&document, 0) else {
        panic!("expected paragraph");
    };
    assert!(paragraph.content.contains(&Inline::SoftBreak));
    assert!(paragraph.content.contains(&Inline::HardBreak));
}

#[test]
fn code_block_keeps_content_and_trims_one_trailing_newline() {
    let document = parse("```rust extra params\nfn main() {\n    // отступ\n}\n```\n");
    match block(&document, 0) {
        Block::CodeBlock(code) => {
            assert_eq!(code.language.as_deref(), Some("rust"));
            assert_eq!(code.code, "fn main() {\n    // отступ\n}");
        }
        other => panic!("expected code block, got {other:?}"),
    }
}

#[test]
fn indented_code_block_has_no_language() {
    let document = parse("    plain code\n");
    match block(&document, 0) {
        Block::CodeBlock(code) => assert_eq!(code.language, None),
        other => panic!("expected code block, got {other:?}"),
    }
}

#[test]
fn ordered_list_keeps_its_start_number() {
    let document = parse("5. пять\n6. шесть\n");
    match block(&document, 0) {
        Block::List(list) => {
            assert_eq!(list.kind, ListKind::Ordered { start: 5 });
            assert_eq!(list.items.len(), 2);
        }
        other => panic!("expected list, got {other:?}"),
    }
}

#[test]
fn list_marker_style_is_not_stored() {
    let dash = parse("- a\n");
    let star = parse("* a\n");
    let plus = parse("+ a\n");
    assert_eq!(block(&dash, 0), block(&star, 0));
    assert_eq!(block(&star, 0), block(&plus, 0));
}

#[test]
fn task_list_items_carry_their_state() {
    let document = parse("- [x] сделано\n- [ ] нет\n- обычный\n");
    match block(&document, 0) {
        Block::List(list) => {
            let checked: Vec<Option<bool>> = list.items.iter().map(|item| item.checked).collect();
            assert_eq!(checked, vec![Some(true), Some(false), None]);
        }
        other => panic!("expected list, got {other:?}"),
    }
}

#[test]
fn nested_lists_are_built() {
    let document = parse("- внешний\n  - вложенный\n    - глубже\n");
    let Block::List(outer) = block(&document, 0) else {
        panic!("expected list");
    };
    let Block::List(inner) = &outer.items[0].blocks[1].value else {
        panic!("expected nested list");
    };
    assert!(matches!(inner.items[0].blocks[1].value, Block::List(_)));
}

#[test]
fn nested_quotes_are_built() {
    let document = parse("> внешняя\n>\n> > вложенная\n");
    let Block::Quote(outer) = block(&document, 0) else {
        panic!("expected quote");
    };
    assert!(matches!(outer.blocks[1].value, Block::Quote(_)));
}

#[test]
fn table_alignments_and_shape_are_parsed() {
    let document = parse("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n");
    match block(&document, 0) {
        Block::Table(table) => {
            assert_eq!(
                table.alignments,
                vec![Alignment::Left, Alignment::Center, Alignment::Right]
            );
            assert_eq!(table.header.cells.len(), 3);
            assert_eq!(table.rows.len(), 2);
        }
        other => panic!("expected table, got {other:?}"),
    }
}

#[test]
fn short_table_rows_are_padded_to_the_column_count() {
    let document = parse("| a | b | c |\n|---|---|---|\n| 1 |\n");
    match block(&document, 0) {
        Block::Table(table) => {
            assert_eq!(table.rows[0].cells.len(), 3);
            assert!(table.rows[0].cells[2].content.is_empty());
        }
        other => panic!("expected table, got {other:?}"),
    }
}

#[test]
fn links_and_images_are_kept_unresolved() {
    let document = parse("[текст](target.md \"подсказка\") ![альт](images/a.png)\n");
    let Block::Paragraph(paragraph) = block(&document, 0) else {
        panic!("expected paragraph");
    };
    let Some(Inline::Link(link)) = paragraph.content.first() else {
        panic!("expected link");
    };
    assert_eq!(link.value.destination, "target.md");
    assert_eq!(link.value.title.as_deref(), Some("подсказка"));

    let Some(Inline::Image(image)) = paragraph.content.last() else {
        panic!("expected image");
    };
    assert_eq!(image.value.source, "images/a.png");
    assert_eq!(image.value.alt, vec![Inline::Text("альт".to_owned())]);
}

#[test]
fn empty_link_destination_is_allowed() {
    let document = parse("[текст]()\n");
    let Block::Paragraph(paragraph) = block(&document, 0) else {
        panic!("expected paragraph");
    };
    let Some(Inline::Link(link)) = paragraph.content.first() else {
        panic!("expected link");
    };
    assert!(link.value.destination.is_empty());
}

#[test]
fn thematic_break_is_a_block() {
    let document = parse("текст\n\n---\n");
    assert!(matches!(block(&document, 1), Block::ThematicBreak));
}

// --- метаданные и front matter ----------------------------------------------

#[test]
fn yaml_front_matter_is_dropped() {
    let document = parse("---\ntitle: не показывать\n---\n\n# Заголовок\n");
    assert_eq!(document.blocks.len(), 1);
    assert!(matches!(block(&document, 0), Block::Heading(_)));
}

// --- Unicode и кириллица ------------------------------------------------------

#[test]
fn cyrillic_text_survives_unchanged() {
    let document = parse("# Ёжик, ёлка и «кавычки» — тире\n");
    let Block::Heading(heading) = block(&document, 0) else {
        panic!("expected heading");
    };
    assert_eq!(
        heading.content,
        vec![Inline::Text("Ёжик, ёлка и «кавычки» — тире".to_owned())]
    );
}

#[test]
fn smart_punctuation_does_not_rewrite_the_source() {
    let document = parse("\"кавычки\" и ...\n");
    let Block::Paragraph(paragraph) = block(&document, 0) else {
        panic!("expected paragraph");
    };
    assert_eq!(
        paragraph.content,
        vec![Inline::Text("\"кавычки\" и ...".to_owned())]
    );
}

#[test]
fn spans_are_byte_ranges_into_the_source() {
    let source = "абв\n\n# Заголовок\n";
    let document = MarkdownParser::default().parse(source).expect("parses");
    for block in &document.blocks {
        assert!(block.span.start <= block.span.end);
        assert!(block.span.end <= source.len());
        assert!(source.get(block.span.start..block.span.end).is_some());
    }
    let heading = &document.blocks[1];
    assert_eq!(
        &source[heading.span.start..heading.span.end],
        "# Заголовок\n"
    );
}

// --- неподдерживаемые конструкции (ТЗ §16) ------------------------------------

#[test]
fn inline_html_is_rejected() {
    let err = parse_err("текст <b>жирный</b>\n");
    assert!(matches!(
        err,
        MarkdownError::UnsupportedConstruct { construct, .. } if construct == "inline HTML"
    ));
    assert!(err.span().is_some());
}

#[test]
fn html_block_is_rejected() {
    let err = parse_err("<div>\nблок\n</div>\n");
    assert!(matches!(
        err,
        MarkdownError::UnsupportedConstruct { construct, .. } if construct == "HTML block"
    ));
}

#[test]
fn footnotes_stay_plain_text_since_the_extension_is_off() {
    let document = parse("текст[^1]\n\n[^1]: сноска\n");
    assert!(
        document
            .blocks
            .iter()
            .all(|block| matches!(block.value, Block::Paragraph(_) | Block::List(_)))
    );
}

// --- лимиты структуры (ТЗ §40) ------------------------------------------------

mod limits {
    use mdpdf::ast::limits::MAX_NESTING_DEPTH;
    use mdpdf::markdown::error::MarkdownError;
    use mdpdf::markdown::parser::MarkdownParser;

    fn parse(source: &str) -> Result<mdpdf::ast::document::Document, MarkdownError> {
        MarkdownParser::default().parse(source)
    }

    /// Без лимита такой документ роняет процесс переполнением стека при обходе
    /// готового AST — то есть аварийным завершением, а не ошибкой.
    #[test]
    fn deeply_nested_quotes_produce_an_error_instead_of_a_crash() {
        let source = format!("{} глубоко\n", ">".repeat(5_000));
        let err = parse(&source).expect_err("depth must be rejected");
        assert!(matches!(err, MarkdownError::LimitExceeded { .. }));
    }

    #[test]
    fn deeply_nested_emphasis_produces_an_error() {
        let source = format!("{0}текст{0}\n", "*".repeat(2_000));
        let err = parse(&source).expect_err("depth must be rejected");
        assert!(matches!(err, MarkdownError::LimitExceeded { .. }));
    }

    #[test]
    fn deeply_nested_lists_produce_an_error() {
        let source: String = (0..500)
            .map(|level| format!("{}- пункт\n", "  ".repeat(level)))
            .collect();
        let err = parse(&source).expect_err("depth must be rejected");
        assert!(matches!(err, MarkdownError::LimitExceeded { .. }));
    }

    #[test]
    fn nesting_just_below_the_limit_is_accepted() {
        // Граница должна пропускать документы, которые в неё укладываются:
        // каждый уровень цитаты — один кадр, плюс абзац внутри.
        let source = format!("{} текст\n", ">".repeat(MAX_NESTING_DEPTH - 2));
        assert!(
            parse(&source).is_ok(),
            "document within the limit must parse"
        );
    }

    #[test]
    fn the_error_carries_a_position() {
        let source = format!("{} глубоко\n", ">".repeat(5_000));
        let err = parse(&source).expect_err("depth must be rejected");
        assert!(err.span().is_some(), "limit errors must be locatable");
    }
}

// --- golden-тесты AST (ТЗ §46) ------------------------------------------------

mod golden {
    use std::path::{Path, PathBuf};

    use mdpdf::markdown::parser::MarkdownParser;

    /// Каждому `tests/fixtures/markdown/<name>.md` соответствует
    /// `tests/fixtures/expected_ast/<name>.json`.
    const FIXTURES: [&str; 5] = ["basic", "nesting", "table", "cyrillic", "mermaid"];

    /// Обновление golden-файлов выполняется явной командой, а не автоматически
    /// при обычном прогоне тестов (ТЗ §46): `make golden-update`.
    fn update_requested() -> bool {
        std::env::var_os("MDPDF_UPDATE_GOLDEN").is_some()
    }

    fn fixture_dir(kind: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(kind)
    }

    #[test]
    fn ast_matches_golden_files() {
        for name in FIXTURES {
            let markdown_path = fixture_dir("markdown").join(format!("{name}.md"));
            let golden_path = fixture_dir("expected_ast").join(format!("{name}.json"));

            let source = std::fs::read_to_string(&markdown_path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", markdown_path.display()));
            let document = MarkdownParser::default()
                .parse(&source)
                .unwrap_or_else(|error| panic!("{name}.md must parse: {error}"));
            let actual = serde_json::to_string_pretty(&document).expect("AST serialises") + "\n";

            if update_requested() {
                std::fs::write(&golden_path, &actual).expect("golden file is writable");
                continue;
            }

            let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|error| {
                panic!(
                    "cannot read {}: {error}\nrun `make golden-update` to create it",
                    golden_path.display()
                )
            });
            assert_eq!(
                actual, expected,
                "AST for {name}.md differs from the golden file"
            );
        }
    }

    #[test]
    fn parsing_is_deterministic() {
        for name in FIXTURES {
            let path = fixture_dir("markdown").join(format!("{name}.md"));
            let source = std::fs::read_to_string(&path).expect("fixture is readable");
            let first = MarkdownParser::default().parse(&source).expect("parses");
            let second = MarkdownParser::default().parse(&source).expect("parses");
            assert_eq!(first, second, "{name}.md parsed differently twice");
        }
    }
}

// --- устойчивость парсера (ТЗ §17: fuzz-тест не вызывает panic) ---------------

mod robustness {
    use mdpdf::markdown::parser::MarkdownParser;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        /// Любой корректный UTF-8 без нулевых байтов либо разбирается,
        /// либо возвращает Err — но никогда не паникует.
        #[test]
        fn arbitrary_text_never_panics(source in ".{0,400}") {
            let _ = MarkdownParser::default().parse(&source);
        }

        /// То же для строк, собранных из «опасных» кусочков Markdown.
        #[test]
        fn markdown_soup_never_panics(
            pieces in proptest::collection::vec(
                prop::sample::select(vec![
                    "#", "##", "- ", "1. ", "> ", "```", "~~~", "|", ":-:", "*", "_", "~~",
                    "[", "]", "(", ")", "!", "`", "\n", "\r\n", "    ", "---", "\t", "\\",
                    "[x]", "[ ]", "текст", "<b>", "&amp;", "\u{feff}",
                ]),
                0..40,
            )
        ) {
            let source: String = pieces.concat();
            let _ = MarkdownParser::default().parse(&source);
        }
    }
}
