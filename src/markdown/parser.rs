//! Парсер Markdown и единая точка настройки расширений (ТЗ §9, §15).
//!
//! Типы `pulldown-cmark` наружу не экспортируются: набор расширений задаётся
//! через собственный [`MarkdownOptions`].

use pulldown_cmark::{Options, Parser};

use crate::ast::block::Block;
use crate::ast::document::Document;
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
            Block::Quote(quote) => check_spans_within_source(&quote.blocks, source_len)?,
            Block::List(list) => {
                for item in &list.items {
                    check_spans_within_source(&item.blocks, source_len)?;
                }
            }
            _ => {}
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
}
