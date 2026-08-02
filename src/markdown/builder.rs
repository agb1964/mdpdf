//! Построение AST из потока событий `pulldown-cmark` (ТЗ §12).
//!
//! Конечный автомат со стеком контейнеров. Инварианты (ТЗ §12.3):
//! `Start(Tag)` создаёт кадр, `End(TagEnd)` обязан закрыть кадр совместимого
//! типа, текст добавляется только в контейнер, допускающий inline, блочный
//! элемент — только в блочный контейнер, после завершения потока стек пуст,
//! ни одно событие не теряется молча.

use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};

use crate::ast::block::{
    Alignment, Block, BlockQuote, CodeBlock, Heading, HeadingLevel, List, ListItem, ListKind,
    Paragraph, Table, TableCell, TableRow,
};
use crate::ast::document::Document;
use crate::ast::inline::{Image, Inline, Link};
use crate::ast::metadata::DocumentMetadata;
use crate::ast::{SourceSpan, Spanned, limits};
use crate::markdown::error::MarkdownError;
use crate::markdown::state::{
    BlockFrame, CodeBlockFrame, Frame, HeadingFrame, ImageFrame, InlineFrame, LinkFrame, ListFrame,
    ListItemFrame, MetadataFrame, ParagraphFrame, TableFrame, TableRowFrame, TableSectionFrame,
};

/// Построитель AST (ТЗ §12.1).
#[derive(Debug)]
pub struct AstBuilder {
    document: Document,
    stack: Vec<Frame>,
    /// Число созданных узлов — растёт монотонно, лимит проверяется на входе.
    nodes: usize,
}

impl AstBuilder {
    /// Создаёт построитель с заданными метаданными.
    #[must_use]
    pub fn new(metadata: DocumentMetadata) -> Self {
        Self {
            document: Document {
                metadata,
                blocks: Vec::new(),
            },
            stack: Vec::new(),
            nodes: 0,
        }
    }

    /// Обрабатывает одно событие вместе с его диапазоном.
    ///
    /// # Errors
    ///
    /// [`MarkdownError`], если конструкция не поддерживается или нарушена
    /// вложенность.
    pub fn handle(&mut self, event: Event<'_>, span: SourceSpan) -> Result<(), MarkdownError> {
        match event {
            Event::Start(tag) => self.start(tag, span),
            Event::End(tag) => self.end(tag, span),
            Event::Text(text) => {
                self.count_node(span)?;
                self.text(&text, span)
            }
            Event::Code(code) => {
                self.count_node(span)?;
                self.push_inline(Inline::Code(code.into_string()), span)
            }
            Event::SoftBreak => {
                self.count_node(span)?;
                self.push_inline(Inline::SoftBreak, span)
            }
            Event::HardBreak => {
                self.count_node(span)?;
                self.push_inline(Inline::HardBreak, span)
            }
            Event::Rule => {
                self.count_node(span)?;
                self.close_implicit_paragraph()?;
                self.push_block(Spanned::new(Block::ThematicBreak, span))
            }
            Event::TaskListMarker(checked) => self.task_list_marker(checked, span),
            Event::Html(_) | Event::InlineHtml(_) => Err(MarkdownError::UnsupportedConstruct {
                construct: "inline HTML",
                span,
            }),
            Event::InlineMath(_) | Event::DisplayMath(_) => {
                Err(MarkdownError::UnsupportedConstruct {
                    construct: "math",
                    span,
                })
            }
            Event::FootnoteReference(_) => Err(MarkdownError::UnsupportedConstruct {
                construct: "footnote",
                span,
            }),
        }
    }

    /// Завершает построение.
    ///
    /// # Errors
    ///
    /// [`MarkdownError::IncompleteDocument`], если остался незакрытый контейнер.
    pub fn finish(mut self) -> Result<Document, MarkdownError> {
        if let Some(frame) = self.stack.pop() {
            return Err(MarkdownError::IncompleteDocument {
                open_construct: frame.name(),
                span: frame.span(),
            });
        }
        Ok(self.document)
    }

    /// Проверяет лимиты структуры до создания кадра (ТЗ §40).
    ///
    /// Без этой проверки документ вроде `>>>>…` на пять тысяч уровней роняет
    /// процесс переполнением стека при обходе готового AST — не ошибкой,
    /// а аварийным завершением.
    fn enter_container(&mut self, span: SourceSpan) -> Result<(), MarkdownError> {
        if self.stack.len() >= limits::MAX_NESTING_DEPTH {
            return Err(MarkdownError::LimitExceeded {
                message: format!(
                    "nesting is deeper than {} containers",
                    limits::MAX_NESTING_DEPTH
                ),
                span,
            });
        }
        self.count_node(span)
    }

    /// Учитывает созданный узел и проверяет их общее число (ТЗ §40).
    fn count_node(&mut self, span: SourceSpan) -> Result<(), MarkdownError> {
        self.nodes += 1;
        if self.nodes > limits::MAX_AST_NODES {
            return Err(MarkdownError::LimitExceeded {
                message: format!("document has more than {} nodes", limits::MAX_AST_NODES),
                span,
            });
        }
        Ok(())
    }

    fn start(&mut self, tag: Tag<'_>, span: SourceSpan) -> Result<(), MarkdownError> {
        self.enter_container(span)?;
        match tag {
            Tag::Paragraph => {
                self.close_implicit_paragraph()?;
                self.stack.push(Frame::Paragraph(ParagraphFrame {
                    span,
                    content: Vec::new(),
                    implicit: false,
                }));
            }
            Tag::Heading { level, id, .. } => {
                self.close_implicit_paragraph()?;
                self.stack.push(Frame::Heading(HeadingFrame {
                    span,
                    level: convert_heading_level(level),
                    id: id.map(pulldown_cmark::CowStr::into_string),
                    content: Vec::new(),
                }));
            }
            Tag::BlockQuote(_) => {
                self.close_implicit_paragraph()?;
                self.stack.push(Frame::Quote(BlockFrame {
                    span,
                    blocks: Vec::new(),
                }));
            }
            Tag::CodeBlock(kind) => {
                self.close_implicit_paragraph()?;
                self.stack.push(Frame::CodeBlock(CodeBlockFrame {
                    span,
                    language: code_block_language(&kind),
                    code: String::new(),
                }));
            }
            Tag::List(start) => {
                self.close_implicit_paragraph()?;
                let kind = start.map_or(ListKind::Unordered, |start| ListKind::Ordered { start });
                self.stack.push(Frame::List(ListFrame {
                    span,
                    kind,
                    items: Vec::new(),
                }));
            }
            Tag::Item => {
                self.close_implicit_paragraph()?;
                self.stack.push(Frame::ListItem(ListItemFrame {
                    span,
                    checked: None,
                    blocks: Vec::new(),
                }));
            }
            Tag::Table(alignments) => {
                self.close_implicit_paragraph()?;
                self.stack.push(Frame::Table(TableFrame {
                    span,
                    alignments: alignments.iter().map(convert_alignment).collect(),
                    header: None,
                    rows: Vec::new(),
                }));
            }
            Tag::TableHead => self.stack.push(Frame::TableHead(TableSectionFrame {
                span,
                cells: Vec::new(),
            })),
            Tag::TableRow => self.stack.push(Frame::TableRow(TableRowFrame {
                span,
                cells: Vec::new(),
            })),
            Tag::TableCell => self.stack.push(Frame::TableCell(InlineFrame {
                span,
                content: Vec::new(),
            })),
            Tag::Emphasis => self.stack.push(Frame::Emphasis(InlineFrame {
                span,
                content: Vec::new(),
            })),
            Tag::Strong => self.stack.push(Frame::Strong(InlineFrame {
                span,
                content: Vec::new(),
            })),
            Tag::Strikethrough => self.stack.push(Frame::Strikethrough(InlineFrame {
                span,
                content: Vec::new(),
            })),
            Tag::Link {
                dest_url, title, ..
            } => self.stack.push(Frame::Link(LinkFrame {
                span,
                destination: dest_url.into_string(),
                title: optional_text(&title),
                content: Vec::new(),
            })),
            Tag::Image {
                dest_url, title, ..
            } => self.stack.push(Frame::Image(ImageFrame {
                span,
                source: dest_url.into_string(),
                title: optional_text(&title),
                alt: Vec::new(),
            })),
            Tag::MetadataBlock(_) => {
                self.close_implicit_paragraph()?;
                self.stack.push(Frame::Metadata(MetadataFrame { span }));
            }
            Tag::HtmlBlock => {
                return Err(MarkdownError::UnsupportedConstruct {
                    construct: "HTML block",
                    span,
                });
            }
            Tag::FootnoteDefinition(_) => {
                return Err(MarkdownError::UnsupportedConstruct {
                    construct: "footnote definition",
                    span,
                });
            }
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                return Err(MarkdownError::UnsupportedConstruct {
                    construct: "definition list",
                    span,
                });
            }
            Tag::Superscript | Tag::Subscript => {
                return Err(MarkdownError::UnsupportedConstruct {
                    construct: "superscript or subscript",
                    span,
                });
            }
        }
        Ok(())
    }

    fn end(&mut self, tag: TagEnd, span: SourceSpan) -> Result<(), MarkdownError> {
        if matches!(tag, TagEnd::Item | TagEnd::BlockQuote(_)) {
            self.close_implicit_paragraph()?;
        }

        let frame = self.pop_frame(&tag, span)?;
        match frame {
            Frame::Paragraph(frame) => {
                let block = Block::Paragraph(Paragraph {
                    content: frame.content,
                });
                self.push_block(Spanned::new(block, frame.span))
            }
            Frame::Heading(frame) => {
                let block = Block::Heading(Heading {
                    level: frame.level,
                    content: frame.content,
                    id: frame.id,
                });
                self.push_block(Spanned::new(block, frame.span))
            }
            Frame::CodeBlock(frame) => {
                let block = Block::CodeBlock(CodeBlock {
                    language: frame.language,
                    code: normalize_code(frame.code),
                });
                self.push_block(Spanned::new(block, frame.span))
            }
            Frame::Quote(frame) => {
                let block = Block::Quote(BlockQuote {
                    blocks: frame.blocks,
                });
                self.push_block(Spanned::new(block, frame.span))
            }
            Frame::List(frame) => {
                let block = Block::List(List {
                    kind: frame.kind,
                    items: frame.items,
                });
                self.push_block(Spanned::new(block, frame.span))
            }
            Frame::ListItem(frame) => self.finish_list_item(frame, span),
            Frame::Table(frame) => self.finish_table(frame),
            Frame::TableHead(frame) => self.finish_table_head(frame, span),
            Frame::TableRow(frame) => self.finish_table_row(frame, span),
            Frame::TableCell(frame) => self.finish_table_cell(frame, span),
            Frame::Emphasis(frame) => self.push_inline(Inline::Emphasis(frame.content), frame.span),
            Frame::Strong(frame) => self.push_inline(Inline::Strong(frame.content), frame.span),
            Frame::Strikethrough(frame) => {
                self.push_inline(Inline::Strikethrough(frame.content), frame.span)
            }
            Frame::Link(frame) => {
                let link = Link {
                    destination: frame.destination,
                    title: frame.title,
                    content: frame.content,
                };
                self.push_inline(Inline::Link(Spanned::new(link, frame.span)), frame.span)
            }
            Frame::Image(frame) => {
                let image = Image {
                    source: frame.source,
                    title: frame.title,
                    alt: frame.alt,
                };
                self.push_inline(Inline::Image(Spanned::new(image, frame.span)), frame.span)
            }
            Frame::Metadata(_) => Ok(()),
        }
    }

    fn pop_frame(&mut self, tag: &TagEnd, span: SourceSpan) -> Result<Frame, MarkdownError> {
        let expected = expected_frame_name(tag);
        let Some(frame) = self.stack.pop() else {
            return Err(MarkdownError::InvalidNesting {
                expected,
                actual: "nothing",
                span,
            });
        };
        if frame.name() == expected {
            return Ok(frame);
        }
        Err(MarkdownError::InvalidNesting {
            expected,
            actual: frame.name(),
            span,
        })
    }

    fn finish_list_item(
        &mut self,
        frame: ListItemFrame,
        span: SourceSpan,
    ) -> Result<(), MarkdownError> {
        match self.stack.last_mut() {
            Some(Frame::List(list)) => {
                list.items.push(ListItem {
                    checked: frame.checked,
                    blocks: frame.blocks,
                });
                Ok(())
            }
            other => Err(MarkdownError::InvalidNesting {
                expected: "list",
                actual: other.map_or("nothing", |frame| frame.name()),
                span,
            }),
        }
    }

    fn finish_table(&mut self, frame: TableFrame) -> Result<(), MarkdownError> {
        let header = frame.header.unwrap_or(TableRow { cells: Vec::new() });
        let columns = header.cells.len();
        // Недостающие ячейки дополняются пустыми; лишние остаются и отлавливаются
        // валидацией (ТЗ §10.9).
        let rows = frame
            .rows
            .into_iter()
            .map(|mut row| {
                if row.cells.len() < columns {
                    row.cells.resize_with(columns, TableCell::default);
                }
                row
            })
            .collect();
        let block = Block::Table(Table {
            alignments: frame.alignments,
            header,
            rows,
        });
        self.push_block(Spanned::new(block, frame.span))
    }

    fn finish_table_head(
        &mut self,
        frame: TableSectionFrame,
        span: SourceSpan,
    ) -> Result<(), MarkdownError> {
        match self.stack.last_mut() {
            Some(Frame::Table(table)) => {
                table.header = Some(TableRow { cells: frame.cells });
                Ok(())
            }
            other => Err(MarkdownError::InvalidNesting {
                expected: "table",
                actual: other.map_or("nothing", |frame| frame.name()),
                span,
            }),
        }
    }

    fn finish_table_row(
        &mut self,
        frame: TableRowFrame,
        span: SourceSpan,
    ) -> Result<(), MarkdownError> {
        match self.stack.last_mut() {
            Some(Frame::Table(table)) => {
                table.rows.push(TableRow { cells: frame.cells });
                Ok(())
            }
            other => Err(MarkdownError::InvalidNesting {
                expected: "table",
                actual: other.map_or("nothing", |frame| frame.name()),
                span,
            }),
        }
    }

    fn finish_table_cell(
        &mut self,
        frame: InlineFrame,
        span: SourceSpan,
    ) -> Result<(), MarkdownError> {
        let cell = TableCell {
            content: frame.content,
        };
        match self.stack.last_mut() {
            Some(Frame::TableHead(head)) => {
                head.cells.push(cell);
                Ok(())
            }
            Some(Frame::TableRow(row)) => {
                row.cells.push(cell);
                Ok(())
            }
            other => Err(MarkdownError::InvalidNesting {
                expected: "table head or table row",
                actual: other.map_or("nothing", |frame| frame.name()),
                span,
            }),
        }
    }

    fn text(&mut self, text: &str, span: SourceSpan) -> Result<(), MarkdownError> {
        match self.stack.last_mut() {
            Some(Frame::CodeBlock(frame)) => {
                frame.code.push_str(text);
                Ok(())
            }
            // Содержимое блока метаданных отбрасывается (ТЗ §9, §10.2).
            Some(Frame::Metadata(_)) => Ok(()),
            _ => self.push_inline(Inline::Text(text.to_owned()), span),
        }
    }

    fn task_list_marker(&mut self, checked: bool, span: SourceSpan) -> Result<(), MarkdownError> {
        match self.stack.last_mut() {
            Some(Frame::ListItem(item)) => {
                item.checked = Some(checked);
                Ok(())
            }
            other => Err(MarkdownError::InvalidNesting {
                expected: "list item",
                actual: other.map_or("nothing", |frame| frame.name()),
                span,
            }),
        }
    }

    fn push_inline(&mut self, inline: Inline, span: SourceSpan) -> Result<(), MarkdownError> {
        if self
            .stack
            .last_mut()
            .and_then(Frame::inline_content)
            .is_none()
        {
            if !self.accepts_blocks() {
                let actual = self.stack.last().map_or("nothing", Frame::name);
                return Err(MarkdownError::InvalidNesting {
                    expected: "inline container",
                    actual,
                    span,
                });
            }
            // Плотный список и цитата отдают inline без явного Start(Paragraph).
            self.stack.push(Frame::Paragraph(ParagraphFrame {
                span,
                content: Vec::new(),
                implicit: true,
            }));
        }

        match self.stack.last_mut().and_then(Frame::inline_content) {
            Some(content) => {
                content.push(inline);
                Ok(())
            }
            None => Err(MarkdownError::InternalInvariant {
                message: "inline container disappeared after being opened".to_owned(),
            }),
        }
    }

    fn push_block(&mut self, block: Spanned<Block>) -> Result<(), MarkdownError> {
        let Some(frame) = self.stack.last_mut() else {
            self.document.blocks.push(block);
            return Ok(());
        };
        let name = frame.name();
        match frame.block_content() {
            Some(blocks) => {
                blocks.push(block);
                Ok(())
            }
            None => Err(MarkdownError::InvalidNesting {
                expected: "block container",
                actual: name,
                span: block.span,
            }),
        }
    }

    fn close_implicit_paragraph(&mut self) -> Result<(), MarkdownError> {
        let implicit = matches!(
            self.stack.last(),
            Some(Frame::Paragraph(frame)) if frame.implicit
        );
        if !implicit {
            return Ok(());
        }
        let Some(Frame::Paragraph(frame)) = self.stack.pop() else {
            return Err(MarkdownError::InternalInvariant {
                message: "implicit paragraph disappeared from the stack".to_owned(),
            });
        };
        let block = Block::Paragraph(Paragraph {
            content: frame.content,
        });
        self.push_block(Spanned::new(block, frame.span))
    }

    fn accepts_blocks(&self) -> bool {
        matches!(
            self.stack.last(),
            None | Some(Frame::Quote(_) | Frame::ListItem(_))
        )
    }
}

const fn expected_frame_name(tag: &TagEnd) -> &'static str {
    match tag {
        TagEnd::Paragraph => "paragraph",
        TagEnd::Heading(_) => "heading",
        TagEnd::BlockQuote(_) => "block quote",
        TagEnd::CodeBlock => "code block",
        TagEnd::HtmlBlock => "html block",
        TagEnd::List(_) => "list",
        TagEnd::Item => "list item",
        TagEnd::FootnoteDefinition => "footnote definition",
        TagEnd::DefinitionList => "definition list",
        TagEnd::DefinitionListTitle => "definition list title",
        TagEnd::DefinitionListDefinition => "definition list definition",
        TagEnd::Table => "table",
        TagEnd::TableHead => "table head",
        TagEnd::TableRow => "table row",
        TagEnd::TableCell => "table cell",
        TagEnd::Emphasis => "emphasis",
        TagEnd::Strong => "strong",
        TagEnd::Strikethrough => "strikethrough",
        TagEnd::Superscript => "superscript",
        TagEnd::Subscript => "subscript",
        TagEnd::Link => "link",
        TagEnd::Image => "image",
        TagEnd::MetadataBlock(_) => "metadata block",
    }
}

const fn convert_heading_level(level: pulldown_cmark::HeadingLevel) -> HeadingLevel {
    match level {
        pulldown_cmark::HeadingLevel::H1 => HeadingLevel::H1,
        pulldown_cmark::HeadingLevel::H2 => HeadingLevel::H2,
        pulldown_cmark::HeadingLevel::H3 => HeadingLevel::H3,
        pulldown_cmark::HeadingLevel::H4 => HeadingLevel::H4,
        pulldown_cmark::HeadingLevel::H5 => HeadingLevel::H5,
        pulldown_cmark::HeadingLevel::H6 => HeadingLevel::H6,
    }
}

const fn convert_alignment(alignment: &pulldown_cmark::Alignment) -> Alignment {
    match alignment {
        pulldown_cmark::Alignment::None => Alignment::None,
        pulldown_cmark::Alignment::Left => Alignment::Left,
        pulldown_cmark::Alignment::Center => Alignment::Center,
        pulldown_cmark::Alignment::Right => Alignment::Right,
    }
}

/// Язык из info string: первый токен, очищенный от пробелов. Дополнительные
/// параметры после языка первая версия игнорирует (ТЗ §10.6).
fn code_block_language(kind: &CodeBlockKind<'_>) -> Option<String> {
    match kind {
        CodeBlockKind::Indented => None,
        CodeBlockKind::Fenced(info) => info.split_whitespace().next().map(str::to_owned),
    }
}

/// Пустой заголовок ссылки или изображения не хранится.
fn optional_text(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

/// Единообразная нормализация завершающего перевода строки (ТЗ §10.6).
fn normalize_code(mut code: String) -> String {
    if code.ends_with('\n') {
        code.pop();
    }
    code
}

// --- инварианты builder-а (ТЗ §12.3) ------------------------------------------
//
// Тесты живут здесь, а не в tests/markdown_parser.rs, потому что напрямую
// используют `pulldown_cmark::{Event, Tag, TagEnd}` и `AstBuilder` — типы,
// которые модуль `builder` (ТЗ §15) не имеет права раскрывать наружу крейта.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::metadata::DocumentMetadata;

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
        use crate::ast::validate::AstValidationError;

        let with_span = MarkdownError::UnsupportedConstruct {
            construct: "x",
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

        // Ошибка валидации тоже несёт позицию: без неё диагностика осталась бы
        // без `файл:строка:столбец`, а код завершения — 4 вместо 5 (ТЗ §16, §43).
        let validation =
            MarkdownError::AstValidation(AstValidationError::EmptyList { span: span() });
        assert_eq!(validation.span(), Some(span()));
    }
}
