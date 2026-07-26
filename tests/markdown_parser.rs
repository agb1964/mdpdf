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
        MarkdownError::UnsupportedConstruct { ref construct, .. } if construct == "inline HTML"
    ));
    assert!(err.span().is_some());
}

#[test]
fn html_block_is_rejected() {
    let err = parse_err("<div>\nблок\n</div>\n");
    assert!(matches!(
        err,
        MarkdownError::UnsupportedConstruct { ref construct, .. } if construct == "HTML block"
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

// --- golden-тесты AST (ТЗ §46) ------------------------------------------------

mod golden {
    use std::path::{Path, PathBuf};

    use mdpdf::markdown::parser::MarkdownParser;

    /// Каждому `tests/fixtures/markdown/<name>.md` соответствует
    /// `tests/fixtures/expected_ast/<name>.json`.
    const FIXTURES: [&str; 4] = ["basic", "nesting", "table", "cyrillic"];

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

// --- инварианты builder-а (ТЗ §12.3) ------------------------------------------

mod builder_invariants {
    use mdpdf::ast::SourceSpan;
    use mdpdf::ast::metadata::DocumentMetadata;
    use mdpdf::markdown::builder::AstBuilder;
    use mdpdf::markdown::error::MarkdownError;
    use pulldown_cmark::{Event, Tag, TagEnd};

    fn span() -> SourceSpan {
        SourceSpan::new(0, 1)
    }

    #[test]
    fn closing_an_unopened_container_is_an_error() {
        let mut builder = AstBuilder::new(DocumentMetadata::default());
        let err = builder
            .handle(Event::End(TagEnd::Paragraph), span())
            .expect_err("nothing to close");
        assert!(matches!(err, MarkdownError::InvalidNesting { .. }));
    }

    #[test]
    fn mismatched_close_is_an_error() {
        let mut builder = AstBuilder::new(DocumentMetadata::default());
        builder
            .handle(Event::Start(Tag::Paragraph), span())
            .expect("paragraph opens");
        let err = builder
            .handle(Event::End(TagEnd::Table), span())
            .expect_err("wrong container");
        match err {
            MarkdownError::InvalidNesting {
                expected, actual, ..
            } => {
                assert_eq!(expected, "table");
                assert_eq!(actual, "paragraph");
            }
            other => panic!("expected InvalidNesting, got {other:?}"),
        }
    }

    #[test]
    fn unclosed_container_fails_at_finish() {
        let mut builder = AstBuilder::new(DocumentMetadata::default());
        builder
            .handle(Event::Start(Tag::Paragraph), span())
            .expect("paragraph opens");
        let err = builder.finish().expect_err("stack is not empty");
        assert!(matches!(err, MarkdownError::IncompleteDocument { .. }));
    }

    #[test]
    fn task_marker_outside_a_list_item_is_an_error() {
        let mut builder = AstBuilder::new(DocumentMetadata::default());
        let err = builder
            .handle(Event::TaskListMarker(true), span())
            .expect_err("no list item");
        assert!(matches!(err, MarkdownError::InvalidNesting { .. }));
    }

    /// Все виды кадров вместе с их именами для диагностики.
    fn every_frame() -> Vec<(Tag<'static>, &'static str)> {
        use pulldown_cmark::{CodeBlockKind, CowStr, LinkType, MetadataBlockKind};

        vec![
            (Tag::Paragraph, "paragraph"),
            (
                Tag::Heading {
                    level: pulldown_cmark::HeadingLevel::H2,
                    id: None,
                    classes: vec![],
                    attrs: vec![],
                },
                "heading",
            ),
            (Tag::BlockQuote(None), "block quote"),
            (
                Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::Borrowed(""))),
                "code block",
            ),
            (Tag::List(None), "list"),
            (Tag::Item, "list item"),
            (Tag::Table(vec![]), "table"),
            (Tag::TableHead, "table head"),
            (Tag::TableRow, "table row"),
            (Tag::TableCell, "table cell"),
            (Tag::Emphasis, "emphasis"),
            (Tag::Strong, "strong"),
            (Tag::Strikethrough, "strikethrough"),
            (
                Tag::Link {
                    link_type: LinkType::Inline,
                    dest_url: CowStr::Borrowed("a"),
                    title: CowStr::Borrowed(""),
                    id: CowStr::Borrowed(""),
                },
                "link",
            ),
            (
                Tag::Image {
                    link_type: LinkType::Inline,
                    dest_url: CowStr::Borrowed("a.png"),
                    title: CowStr::Borrowed(""),
                    id: CowStr::Borrowed(""),
                },
                "image",
            ),
            (
                Tag::MetadataBlock(MetadataBlockKind::YamlStyle),
                "metadata block",
            ),
        ]
    }

    #[test]
    fn every_frame_reports_its_own_name_and_span_when_left_open() {
        for (tag, name) in every_frame() {
            let mut builder = AstBuilder::new(DocumentMetadata::default());
            builder
                .handle(Event::Start(tag), SourceSpan::new(3, 9))
                .expect("container opens");
            match builder.finish().expect_err("stack is not empty") {
                MarkdownError::IncompleteDocument {
                    open_construct,
                    span,
                } => {
                    assert_eq!(open_construct, name);
                    assert_eq!(span, SourceSpan::new(3, 9));
                }
                other => panic!("expected IncompleteDocument for {name}, got {other:?}"),
            }
        }
    }

    /// Конструкции, выключенные в первой версии, обязаны давать понятную ошибку,
    /// а не теряться молча (ТЗ §12.3, §16).
    #[test]
    fn every_unsupported_tag_is_reported() {
        use pulldown_cmark::CowStr;

        let unsupported = vec![
            Tag::HtmlBlock,
            Tag::FootnoteDefinition(CowStr::Borrowed("1")),
            Tag::DefinitionList,
            Tag::DefinitionListTitle,
            Tag::DefinitionListDefinition,
            Tag::Superscript,
            Tag::Subscript,
        ];

        for tag in unsupported {
            let mut builder = AstBuilder::new(DocumentMetadata::default());
            let err = builder
                .handle(Event::Start(tag), span())
                .expect_err("tag must be rejected");
            assert!(
                matches!(err, MarkdownError::UnsupportedConstruct { .. }),
                "expected UnsupportedConstruct, got {err:?}"
            );
        }
    }

    #[test]
    fn every_unsupported_event_is_reported() {
        use pulldown_cmark::CowStr;

        let unsupported = vec![
            Event::Html(CowStr::Borrowed("<div>")),
            Event::InlineHtml(CowStr::Borrowed("<b>")),
            Event::InlineMath(CowStr::Borrowed("x")),
            Event::DisplayMath(CowStr::Borrowed("x")),
            Event::FootnoteReference(CowStr::Borrowed("1")),
        ];

        for event in unsupported {
            let mut builder = AstBuilder::new(DocumentMetadata::default());
            let err = builder
                .handle(event, span())
                .expect_err("event must be rejected");
            assert!(
                matches!(err, MarkdownError::UnsupportedConstruct { .. }),
                "expected UnsupportedConstruct, got {err:?}"
            );
        }
    }

    #[test]
    fn table_parts_must_close_into_a_table() {
        let cases = [
            (Tag::TableHead, TagEnd::TableHead, "table"),
            (Tag::TableRow, TagEnd::TableRow, "table"),
            (Tag::TableCell, TagEnd::TableCell, "table head or table row"),
            (Tag::Item, TagEnd::Item, "list"),
        ];

        for (open, close, expected_parent) in cases {
            let mut builder = AstBuilder::new(DocumentMetadata::default());
            builder
                .handle(Event::Start(open), span())
                .expect("container opens");
            match builder.handle(Event::End(close), span()) {
                Err(MarkdownError::InvalidNesting {
                    expected, actual, ..
                }) => {
                    assert_eq!(expected, expected_parent);
                    assert_eq!(actual, "nothing");
                }
                other => panic!("expected InvalidNesting, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_block_cannot_land_in_a_list_frame() {
        let mut builder = AstBuilder::new(DocumentMetadata::default());
        builder
            .handle(Event::Start(Tag::List(None)), span())
            .expect("list opens");
        // Горизонтальная линия — блок, а список принимает только элементы.
        match builder.handle(Event::Rule, span()) {
            Err(MarkdownError::InvalidNesting {
                expected, actual, ..
            }) => {
                assert_eq!(expected, "block container");
                assert_eq!(actual, "list");
            }
            other => panic!("expected InvalidNesting, got {other:?}"),
        }
    }

    #[test]
    fn inline_cannot_land_in_a_list_frame() {
        let mut builder = AstBuilder::new(DocumentMetadata::default());
        builder
            .handle(Event::Start(Tag::List(None)), span())
            .expect("list opens");
        match builder.handle(Event::SoftBreak, span()) {
            Err(MarkdownError::InvalidNesting {
                expected, actual, ..
            }) => {
                assert_eq!(expected, "inline container");
                assert_eq!(actual, "list");
            }
            other => panic!("expected InvalidNesting, got {other:?}"),
        }
    }

    #[test]
    fn metadata_block_swallows_its_content() {
        use pulldown_cmark::{CowStr, MetadataBlockKind};

        let mut builder = AstBuilder::new(DocumentMetadata::default());
        let kind = MetadataBlockKind::YamlStyle;
        builder
            .handle(Event::Start(Tag::MetadataBlock(kind)), span())
            .expect("metadata opens");
        builder
            .handle(Event::Text(CowStr::Borrowed("title: x")), span())
            .expect("text is swallowed");
        builder
            .handle(Event::End(TagEnd::MetadataBlock(kind)), span())
            .expect("metadata closes");
        let document = builder.finish().expect("document finishes");
        assert!(document.is_empty());
    }

    #[test]
    fn error_spans_are_reported_only_where_they_exist() {
        use mdpdf::ast::validate::AstValidationError;

        let with_span = MarkdownError::UnsupportedConstruct {
            construct: "x".to_owned(),
            span: span(),
        };
        assert_eq!(with_span.span(), Some(span()));

        let invalid_input = MarkdownError::InvalidInput {
            message: "x".to_owned(),
            span: Some(span()),
        };
        assert_eq!(invalid_input.span(), Some(span()));

        let internal = MarkdownError::InternalInvariant {
            message: "x".to_owned(),
        };
        assert_eq!(internal.span(), None);

        let validation =
            MarkdownError::AstValidation(AstValidationError::EmptyList { span: span() });
        assert_eq!(validation.span(), None);
    }
}
