//! Парсер Markdown и единая точка настройки расширений (ТЗ §9, §15).
//!
//! Типы `pulldown-cmark` наружу не экспортируются: набор расширений задаётся
//! через собственный [`MarkdownOptions`].

use pulldown_cmark::{Options, Parser};

use crate::ast::block::Block;
use crate::ast::document::Document;
use crate::ast::inline::Inline;
use crate::ast::metadata::DocumentMetadata;
use crate::ast::validate::validate_document;
use crate::ast::{SourceSpan, Spanned};
use crate::markdown::builder::AstBuilder;
use crate::markdown::error::MarkdownError;

/// Набор расширений Markdown, поддерживаемых первой версией (ТЗ §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownOptions {
    /// Таблицы GFM.
    pub tables: bool,
    /// Списки задач.
    pub task_lists: bool,
    /// Зачёркнутый текст.
    pub strikethrough: bool,
    /// Идентификаторы и атрибуты заголовков.
    pub heading_attributes: bool,
    /// Распознавать YAML front matter, чтобы отбросить его вместе с содержимым.
    pub skip_metadata_blocks: bool,
}

impl Default for MarkdownOptions {
    /// Набор из ТЗ §9. Сноски, raw HTML и smart punctuation выключены.
    fn default() -> Self {
        Self {
            tables: true,
            task_lists: true,
            strikethrough: true,
            heading_attributes: true,
            skip_metadata_blocks: true,
        }
    }
}

/// Единственное место, где набор расширений превращается в опции парсера (ТЗ §9).
fn markdown_options(options: MarkdownOptions) -> Options {
    let mut result = Options::empty();
    result.set(Options::ENABLE_TABLES, options.tables);
    result.set(Options::ENABLE_TASKLISTS, options.task_lists);
    result.set(Options::ENABLE_STRIKETHROUGH, options.strikethrough);
    result.set(
        Options::ENABLE_HEADING_ATTRIBUTES,
        options.heading_attributes,
    );
    result.set(
        Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
        options.skip_metadata_blocks,
    );
    result
}

/// Парсер Markdown (ТЗ §15).
#[derive(Debug, Clone)]
pub struct MarkdownParser {
    options: MarkdownOptions,
    metadata: DocumentMetadata,
}

impl MarkdownParser {
    /// Создаёт парсер с заданным набором расширений.
    #[must_use]
    pub const fn new(options: MarkdownOptions) -> Self {
        Self {
            options,
            metadata: DocumentMetadata {
                title: None,
                author: None,
                language: None,
            },
        }
    }

    /// Задаёт метаданные документа. В первой версии они приходят из CLI (ТЗ §10.2).
    #[must_use]
    pub fn with_metadata(mut self, metadata: DocumentMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Преобразует Markdown в AST и проверяет его инварианты.
    ///
    /// # Errors
    ///
    /// [`MarkdownError`] при неподдерживаемой конструкции, нарушенной
    /// вложенности или неудачной валидации AST.
    pub fn parse(&self, source: &str) -> Result<Document, MarkdownError> {
        let mut builder = AstBuilder::new(self.metadata.clone());
        let parser = Parser::new_ext(source, markdown_options(self.options));

        for (event, range) in parser.into_offset_iter() {
            builder.handle(event, SourceSpan::from(range))?;
        }

        let document = builder.finish()?;
        validate_document(&document)?;
        check_spans_within_source(&document.blocks, source.len())?;
        Ok(document)
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new(MarkdownOptions::default())
    }
}

/// Проверка «span не выходит за пределы исходного документа» (ТЗ §14).
///
/// Живёт здесь, а не в `validate_document`: сигнатура валидатора закреплена
/// в ТЗ и длину исходного текста не принимает.
///
/// Обходит дерево целиком, а не только блочные контейнеры: ссылки и
/// изображения несут собственный `Spanned` внутри inline-содержимого
/// (заголовков, абзацев, ячеек таблиц), и их диапазоны точно так же могут
/// выйти за пределы источника, если `pulldown-cmark` когда-нибудь вернёт
/// диапазон, не совпадающий с длиной документа.
fn check_spans_within_source(
    blocks: &[Spanned<Block>],
    source_len: usize,
) -> Result<(), MarkdownError> {
    for block in blocks {
        if block.span.end > source_len {
            return Err(MarkdownError::InternalInvariant {
                message: format!(
                    "span {}..{} exceeds source length {source_len}",
                    block.span.start, block.span.end
                ),
            });
        }
        match &block.value {
            Block::Heading(heading) => check_inline_spans(&heading.content, source_len)?,
            Block::Paragraph(paragraph) => check_inline_spans(&paragraph.content, source_len)?,
            Block::Quote(quote) => check_spans_within_source(&quote.blocks, source_len)?,
            Block::List(list) => {
                for item in &list.items {
                    check_spans_within_source(&item.blocks, source_len)?;
                }
            }
            Block::Table(table) => {
                for cell in table
                    .header
                    .cells
                    .iter()
                    .chain(table.rows.iter().flat_map(|row| row.cells.iter()))
                {
                    check_inline_spans(&cell.content, source_len)?;
                }
            }
            Block::CodeBlock(_) | Block::ThematicBreak => {}
        }
    }
    Ok(())
}

/// Проверяет диапазоны ссылок и изображений внутри inline-содержимого,
/// рекурсивно спускаясь в форматирование и вложенный alt-текст (ТЗ §14).
fn check_inline_spans(inlines: &[Inline], source_len: usize) -> Result<(), MarkdownError> {
    for inline in inlines {
        match inline {
            Inline::Link(link) => {
                if link.span.end > source_len {
                    return Err(MarkdownError::InternalInvariant {
                        message: format!(
                            "span {}..{} exceeds source length {source_len}",
                            link.span.start, link.span.end
                        ),
                    });
                }
                check_inline_spans(&link.value.content, source_len)?;
            }
            Inline::Image(image) => {
                if image.span.end > source_len {
                    return Err(MarkdownError::InternalInvariant {
                        message: format!(
                            "span {}..{} exceeds source length {source_len}",
                            image.span.start, image.span.end
                        ),
                    });
                }
                check_inline_spans(&image.value.alt, source_len)?;
            }
            Inline::Emphasis(content)
            | Inline::Strong(content)
            | Inline::Strikethrough(content) => check_inline_spans(content, source_len)?,
            Inline::Text(_) | Inline::Code(_) | Inline::SoftBreak | Inline::HardBreak => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_extensions_match_specification() {
        let options = markdown_options(MarkdownOptions::default());
        assert!(options.contains(Options::ENABLE_TABLES));
        assert!(options.contains(Options::ENABLE_TASKLISTS));
        assert!(options.contains(Options::ENABLE_STRIKETHROUGH));
        assert!(options.contains(Options::ENABLE_HEADING_ATTRIBUTES));
        assert!(options.contains(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS));
    }

    #[test]
    fn disabled_extensions_stay_disabled() {
        let options = markdown_options(MarkdownOptions::default());
        assert!(!options.contains(Options::ENABLE_FOOTNOTES));
        assert!(!options.contains(Options::ENABLE_OLD_FOOTNOTES));
        assert!(!options.contains(Options::ENABLE_SMART_PUNCTUATION));
        assert!(!options.contains(Options::ENABLE_MATH));
        assert!(!options.contains(Options::ENABLE_DEFINITION_LIST));
        assert!(!options.contains(Options::ENABLE_WIKILINKS));
        assert!(!options.contains(Options::ENABLE_GFM));
    }

    // Кадры builder-а всегда строят span из реального диапазона pulldown-cmark,
    // поэтому конструируем AST напрямую — это единственный способ подать сюда
    // span, выходящий за пределы источника, и проверить, что обход его находит
    // на любой глубине, а не только в блочных контейнерах (ТЗ §14).

    fn out_of_range_link() -> Inline {
        use crate::ast::inline::Link;

        Inline::Link(Spanned::new(
            Link {
                destination: "x".to_owned(),
                title: None,
                content: Vec::new(),
            },
            SourceSpan::new(0, 999),
        ))
    }

    #[test]
    fn link_span_inside_a_paragraph_is_checked() {
        use crate::ast::block::Paragraph;

        let blocks = vec![Spanned::new(
            Block::Paragraph(Paragraph {
                content: vec![out_of_range_link()],
            }),
            SourceSpan::new(0, 3),
        )];
        let err = check_spans_within_source(&blocks, 3).expect_err("span exceeds source");
        assert!(matches!(err, MarkdownError::InternalInvariant { .. }));
    }

    #[test]
    fn link_span_inside_a_heading_is_checked() {
        use crate::ast::block::Heading;
        use crate::ast::block::HeadingLevel;

        let blocks = vec![Spanned::new(
            Block::Heading(Heading {
                level: HeadingLevel::H1,
                content: vec![out_of_range_link()],
                id: None,
            }),
            SourceSpan::new(0, 3),
        )];
        let err = check_spans_within_source(&blocks, 3).expect_err("span exceeds source");
        assert!(matches!(err, MarkdownError::InternalInvariant { .. }));
    }

    #[test]
    fn image_span_nested_in_emphasis_inside_a_table_cell_is_checked() {
        use crate::ast::block::{Alignment, Table, TableCell, TableRow};
        use crate::ast::inline::Image;

        let image = Inline::Image(Spanned::new(
            Image {
                source: "a.png".to_owned(),
                title: None,
                alt: Vec::new(),
            },
            SourceSpan::new(0, 999),
        ));
        let cell = TableCell {
            content: vec![Inline::Emphasis(vec![image])],
        };
        let blocks = vec![Spanned::new(
            Block::Table(Table {
                alignments: vec![Alignment::None],
                header: TableRow { cells: vec![] },
                rows: vec![TableRow { cells: vec![cell] }],
            }),
            SourceSpan::new(0, 3),
        )];
        let err = check_spans_within_source(&blocks, 3).expect_err("span exceeds source");
        assert!(matches!(err, MarkdownError::InternalInvariant { .. }));
    }

    #[test]
    fn spans_within_bounds_are_accepted_everywhere() {
        use crate::ast::block::{
            Alignment, Heading, HeadingLevel, Paragraph, Table, TableCell, TableRow,
        };
        use crate::ast::inline::{Image, Link};

        let link = Inline::Link(Spanned::new(
            Link {
                destination: "x".to_owned(),
                title: None,
                content: Vec::new(),
            },
            SourceSpan::new(0, 3),
        ));
        let image = Inline::Image(Spanned::new(
            Image {
                source: "a.png".to_owned(),
                title: None,
                alt: Vec::new(),
            },
            SourceSpan::new(0, 3),
        ));
        let blocks = vec![
            Spanned::new(
                Block::Heading(Heading {
                    level: HeadingLevel::H1,
                    content: vec![link],
                    id: None,
                }),
                SourceSpan::new(0, 3),
            ),
            Spanned::new(
                Block::Paragraph(Paragraph {
                    content: vec![Inline::Strong(vec![image])],
                }),
                SourceSpan::new(0, 3),
            ),
            Spanned::new(
                Block::Table(Table {
                    alignments: vec![Alignment::None],
                    header: TableRow {
                        cells: vec![TableCell {
                            content: vec![Inline::Text("a".to_owned())],
                        }],
                    },
                    rows: vec![],
                }),
                SourceSpan::new(0, 3),
            ),
        ];
        assert!(check_spans_within_source(&blocks, 3).is_ok());
    }
}
