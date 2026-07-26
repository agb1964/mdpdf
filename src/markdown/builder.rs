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
use crate::ast::{SourceSpan, Spanned};
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
            Event::Text(text) => self.text(&text, span),
            Event::Code(code) => self.push_inline(Inline::Code(code.into_string()), span),
            Event::SoftBreak => self.push_inline(Inline::SoftBreak, span),
            Event::HardBreak => self.push_inline(Inline::HardBreak, span),
            Event::Rule => {
                self.close_implicit_paragraph()?;
                self.push_block(Spanned::new(Block::ThematicBreak, span))
            }
            Event::TaskListMarker(checked) => self.task_list_marker(checked, span),
            Event::Html(_) | Event::InlineHtml(_) => Err(MarkdownError::UnsupportedConstruct {
                construct: "inline HTML".to_owned(),
                span,
            }),
            Event::InlineMath(_) | Event::DisplayMath(_) => {
                Err(MarkdownError::UnsupportedConstruct {
                    construct: "math".to_owned(),
                    span,
                })
            }
            Event::FootnoteReference(_) => Err(MarkdownError::UnsupportedConstruct {
                construct: "footnote".to_owned(),
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
                open_construct: frame.name().to_owned(),
                span: frame.span(),
            });
        }
        Ok(self.document)
    }

    fn start(&mut self, tag: Tag<'_>, span: SourceSpan) -> Result<(), MarkdownError> {
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
                    construct: "HTML block".to_owned(),
                    span,
                });
            }
            Tag::FootnoteDefinition(_) => {
                return Err(MarkdownError::UnsupportedConstruct {
                    construct: "footnote definition".to_owned(),
                    span,
                });
            }
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                return Err(MarkdownError::UnsupportedConstruct {
                    construct: "definition list".to_owned(),
                    span,
                });
            }
            Tag::Superscript | Tag::Subscript => {
                return Err(MarkdownError::UnsupportedConstruct {
                    construct: "superscript or subscript".to_owned(),
                    span,
                });
            }
        }
        Ok(())
    }

    fn end(&mut self, tag: TagEnd, span: SourceSpan) -> Result<(), MarkdownError> {
        match tag {
            TagEnd::Item | TagEnd::BlockQuote(_) => self.close_implicit_paragraph()?,
            _ => {}
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
                expected: expected.to_owned(),
                actual: "nothing".to_owned(),
                span,
            });
        };
        if frame.name() == expected {
            return Ok(frame);
        }
        Err(MarkdownError::InvalidNesting {
            expected: expected.to_owned(),
            actual: frame.name().to_owned(),
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
                expected: "list".to_owned(),
                actual: other.map_or("nothing", |frame| frame.name()).to_owned(),
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
                expected: "table".to_owned(),
                actual: other.map_or("nothing", |frame| frame.name()).to_owned(),
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
                expected: "table".to_owned(),
                actual: other.map_or("nothing", |frame| frame.name()).to_owned(),
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
                expected: "table head or table row".to_owned(),
                actual: other.map_or("nothing", |frame| frame.name()).to_owned(),
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
                expected: "list item".to_owned(),
                actual: other.map_or("nothing", |frame| frame.name()).to_owned(),
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
                let actual = self
                    .stack
                    .last()
                    .map_or("nothing", crate::markdown::state::Frame::name);
                return Err(MarkdownError::InvalidNesting {
                    expected: "inline container".to_owned(),
                    actual: actual.to_owned(),
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
                expected: "block container".to_owned(),
                actual: name.to_owned(),
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
        CodeBlockKind::Fenced(info) => info
            .split_whitespace()
            .next()
            .map(str::to_owned)
            .filter(|language| !language.is_empty()),
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
