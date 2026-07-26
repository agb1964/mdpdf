//! Валидация построенного AST (ТЗ §14).
//!
//! Проверки, которые уже гарантированы системой типов, здесь не дублируются:
//! уровень заголовка невозможно задать неверно ([`HeadingLevel`] — перечисление),
//! заголовок не может содержать блочные узлы (у него `Vec<Inline>`), кодовый
//! блок всегда корректный UTF-8 (`String`).
//!
//! Проверка «span не выходит за пределы исходного документа» выполняется в
//! [`crate::markdown::parser`]: сигнатура [`validate_document`] закреплена в ТЗ
//! и исходный текст не принимает.

use thiserror::Error;

use crate::ast::SourceSpan;
use crate::ast::block::{Block, List, ListKind, Table};
use crate::ast::document::Document;
use crate::ast::inline::Inline;

/// Ошибка валидации AST.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AstValidationError {
    /// Таблица без столбцов.
    #[error("table has no columns")]
    TableWithoutColumns {
        /// Диапазон таблицы.
        span: SourceSpan,
    },

    /// Число выравниваний не совпадает с числом столбцов.
    #[error("table has {columns} columns but {alignments} alignments")]
    AlignmentCountMismatch {
        /// Число столбцов.
        columns: usize,
        /// Число выравниваний.
        alignments: usize,
        /// Диапазон таблицы.
        span: SourceSpan,
    },

    /// Строка таблицы содержит не столько ячеек, сколько столбцов.
    #[error("table row has {actual} cells but table has {expected} columns")]
    RowWidthMismatch {
        /// Ожидаемое число ячеек.
        expected: usize,
        /// Фактическое число ячеек.
        actual: usize,
        /// Диапазон таблицы.
        span: SourceSpan,
    },

    /// Список без элементов.
    #[error("list has no items")]
    EmptyList {
        /// Диапазон списка.
        span: SourceSpan,
    },

    /// Нумерованный список с номером 0.
    #[error("ordered list cannot start at 0")]
    ZeroOrderedListStart {
        /// Диапазон списка.
        span: SourceSpan,
    },

    /// Ссылка внутри ссылки.
    #[error("nested link is not allowed")]
    NestedLink {
        /// Диапазон вложенной ссылки.
        span: SourceSpan,
    },

    /// Изображение внутри изображения.
    #[error("nested image is not allowed")]
    NestedImage {
        /// Диапазон вложенного изображения.
        span: SourceSpan,
    },

    /// Сетевой адрес изображения (ТЗ §10.12).
    #[error("network image source is not allowed: {source_url}")]
    NetworkImage {
        /// Исходный адрес.
        source_url: String,
        /// Диапазон изображения.
        span: SourceSpan,
    },

    /// Диапазон задан задом наперёд.
    #[error("invalid span: start {} is greater than end {}", .span.start, .span.end)]
    InvalidSpan {
        /// Некорректный диапазон.
        span: SourceSpan,
    },
}

/// Проверяет инварианты документа.
///
/// Пустой Markdown — корректный документ.
///
/// # Errors
///
/// Возвращает первую найденную ошибку из [`AstValidationError`].
pub fn validate_document(document: &Document) -> Result<(), AstValidationError> {
    validate_blocks(&document.blocks)
}

fn validate_blocks(blocks: &[crate::ast::Spanned<Block>]) -> Result<(), AstValidationError> {
    for block in blocks {
        validate_span(block.span)?;
        match &block.value {
            Block::Heading(heading) => validate_inlines(&heading.content, false, false)?,
            Block::Paragraph(paragraph) => validate_inlines(&paragraph.content, false, false)?,
            Block::Quote(quote) => validate_blocks(&quote.blocks)?,
            Block::List(list) => validate_list(list, block.span)?,
            Block::Table(table) => validate_table(table, block.span)?,
            Block::CodeBlock(_) | Block::ThematicBreak => {}
        }
    }
    Ok(())
}

fn validate_list(list: &List, span: SourceSpan) -> Result<(), AstValidationError> {
    if list.items.is_empty() {
        return Err(AstValidationError::EmptyList { span });
    }
    if matches!(list.kind, ListKind::Ordered { start: 0 }) {
        return Err(AstValidationError::ZeroOrderedListStart { span });
    }
    for item in &list.items {
        validate_blocks(&item.blocks)?;
    }
    Ok(())
}

fn validate_table(table: &Table, span: SourceSpan) -> Result<(), AstValidationError> {
    let columns = table.header.cells.len();
    if columns == 0 {
        return Err(AstValidationError::TableWithoutColumns { span });
    }
    if table.alignments.len() != columns {
        return Err(AstValidationError::AlignmentCountMismatch {
            columns,
            alignments: table.alignments.len(),
            span,
        });
    }
    for row in std::iter::once(&table.header).chain(&table.rows) {
        if row.cells.len() != columns {
            return Err(AstValidationError::RowWidthMismatch {
                expected: columns,
                actual: row.cells.len(),
                span,
            });
        }
        for cell in &row.cells {
            validate_inlines(&cell.content, false, false)?;
        }
    }
    Ok(())
}

fn validate_inlines(
    inlines: &[Inline],
    inside_link: bool,
    inside_image: bool,
) -> Result<(), AstValidationError> {
    for inline in inlines {
        match inline {
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strikethrough(children) => {
                validate_inlines(children, inside_link, inside_image)?;
            }
            Inline::Link(link) => {
                validate_span(link.span)?;
                if inside_link {
                    return Err(AstValidationError::NestedLink { span: link.span });
                }
                validate_inlines(&link.value.content, true, inside_image)?;
            }
            Inline::Image(image) => {
                validate_span(image.span)?;
                if inside_image {
                    return Err(AstValidationError::NestedImage { span: image.span });
                }
                if is_network_source(&image.value.source) {
                    return Err(AstValidationError::NetworkImage {
                        source_url: image.value.source.clone(),
                        span: image.span,
                    });
                }
                validate_inlines(&image.value.alt, inside_link, true)?;
            }
            Inline::Text(_) | Inline::Code(_) | Inline::SoftBreak | Inline::HardBreak => {}
        }
    }
    Ok(())
}

const fn validate_span(span: SourceSpan) -> Result<(), AstValidationError> {
    if span.start > span.end {
        return Err(AstValidationError::InvalidSpan { span });
    }
    Ok(())
}

/// Сетевой или `data:`-адрес изображения. Проверка регистронезависимая.
#[must_use]
pub fn is_network_source(source: &str) -> bool {
    let trimmed = source.trim();
    ["http://", "https://", "data:"].iter().any(|scheme| {
        trimmed.len() >= scheme.len() && trimmed[..scheme.len()].eq_ignore_ascii_case(scheme)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Spanned;
    use crate::ast::block::{ListItem, TableCell, TableRow};
    use crate::ast::inline::{Image, Link};

    fn span() -> SourceSpan {
        SourceSpan::new(0, 1)
    }

    fn document(blocks: Vec<Spanned<Block>>) -> Document {
        Document {
            metadata: crate::ast::metadata::DocumentMetadata::default(),
            blocks,
        }
    }

    #[test]
    fn empty_document_is_valid() {
        assert!(validate_document(&document(vec![])).is_ok());
    }

    #[test]
    fn empty_list_is_rejected() {
        let list = Block::List(List {
            kind: ListKind::Unordered,
            items: vec![],
        });
        let err =
            validate_document(&document(vec![Spanned::new(list, span())])).expect_err("empty list");
        assert!(matches!(err, AstValidationError::EmptyList { .. }));
    }

    #[test]
    fn ordered_list_starting_at_zero_is_rejected() {
        let list = Block::List(List {
            kind: ListKind::Ordered { start: 0 },
            items: vec![ListItem {
                checked: None,
                blocks: vec![],
            }],
        });
        let err =
            validate_document(&document(vec![Spanned::new(list, span())])).expect_err("start 0");
        assert!(matches!(
            err,
            AstValidationError::ZeroOrderedListStart { .. }
        ));
    }

    #[test]
    fn table_row_width_must_match_header() {
        let table = Block::Table(Table {
            alignments: vec![crate::ast::block::Alignment::None],
            header: TableRow {
                cells: vec![TableCell::default()],
            },
            rows: vec![TableRow {
                cells: vec![TableCell::default(), TableCell::default()],
            }],
        });
        let err =
            validate_document(&document(vec![Spanned::new(table, span())])).expect_err("row width");
        assert!(matches!(
            err,
            AstValidationError::RowWidthMismatch {
                expected: 1,
                actual: 2,
                ..
            }
        ));
    }

    #[test]
    fn nested_link_is_rejected() {
        let inner = Inline::Link(Spanned::new(
            Link {
                destination: "b".to_owned(),
                title: None,
                content: vec![],
            },
            span(),
        ));
        let outer = Inline::Link(Spanned::new(
            Link {
                destination: "a".to_owned(),
                title: None,
                content: vec![inner],
            },
            span(),
        ));
        let paragraph = Block::Paragraph(crate::ast::block::Paragraph {
            content: vec![outer],
        });
        let err = validate_document(&document(vec![Spanned::new(paragraph, span())]))
            .expect_err("nested link");
        assert!(matches!(err, AstValidationError::NestedLink { .. }));
    }

    #[test]
    fn network_image_is_rejected() {
        let image = Inline::Image(Spanned::new(
            Image {
                source: "HTTPS://example.com/a.png".to_owned(),
                title: None,
                alt: vec![],
            },
            span(),
        ));
        let paragraph = Block::Paragraph(crate::ast::block::Paragraph {
            content: vec![image],
        });
        let err = validate_document(&document(vec![Spanned::new(paragraph, span())]))
            .expect_err("network image");
        assert!(matches!(err, AstValidationError::NetworkImage { .. }));
    }

    #[test]
    fn local_image_paths_are_accepted() {
        assert!(!is_network_source("images/schema.png"));
        assert!(!is_network_source("./a.png"));
        assert!(is_network_source("http://a/b.png"));
        assert!(is_network_source("  https://a/b.png"));
        assert!(is_network_source("data:image/png;base64,AAAA"));
    }

    #[test]
    fn reversed_span_is_rejected() {
        let paragraph = Block::Paragraph(crate::ast::block::Paragraph { content: vec![] });
        let err = validate_document(&document(vec![Spanned::new(
            paragraph,
            SourceSpan::new(5, 1),
        )]))
        .expect_err("reversed span");
        assert!(matches!(err, AstValidationError::InvalidSpan { .. }));
    }
}
