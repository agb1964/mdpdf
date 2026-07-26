//! Кадры стека builder-а (ТЗ §12.2).
//!
//! На каждый вид контейнера — свой кадр со своими полями. Единый кадр
//! с большим количеством `Option`-полей не допускается.

use crate::ast::SourceSpan;
use crate::ast::Spanned;
use crate::ast::block::{Alignment, Block, HeadingLevel, ListItem, ListKind, TableCell, TableRow};
use crate::ast::inline::Inline;

/// Кадр стека контейнеров.
#[derive(Debug)]
pub enum Frame {
    /// Абзац.
    Paragraph(ParagraphFrame),
    /// Заголовок.
    Heading(HeadingFrame),
    /// Курсив.
    Emphasis(InlineFrame),
    /// Полужирный.
    Strong(InlineFrame),
    /// Зачёркнутый.
    Strikethrough(InlineFrame),
    /// Ссылка.
    Link(LinkFrame),
    /// Изображение.
    Image(ImageFrame),
    /// Блок кода.
    CodeBlock(CodeBlockFrame),
    /// Цитата.
    Quote(BlockFrame),
    /// Список.
    List(ListFrame),
    /// Элемент списка.
    ListItem(ListItemFrame),
    /// Таблица.
    Table(TableFrame),
    /// Заголовочная секция таблицы.
    TableHead(TableSectionFrame),
    /// Строка тела таблицы.
    TableRow(TableRowFrame),
    /// Ячейка таблицы.
    TableCell(InlineFrame),
    /// Блок метаданных: содержимое отбрасывается (ТЗ §9).
    Metadata(MetadataFrame),
}

impl Frame {
    /// Диапазон исходного текста, занимаемый контейнером.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Paragraph(frame) => frame.span,
            Self::Heading(frame) => frame.span,
            Self::Emphasis(frame) | Self::Strong(frame) | Self::Strikethrough(frame) => frame.span,
            Self::Link(frame) => frame.span,
            Self::Image(frame) => frame.span,
            Self::CodeBlock(frame) => frame.span,
            Self::Quote(frame) => frame.span,
            Self::List(frame) => frame.span,
            Self::ListItem(frame) => frame.span,
            Self::Table(frame) => frame.span,
            Self::TableHead(frame) => frame.span,
            Self::TableRow(frame) => frame.span,
            Self::TableCell(frame) => frame.span,
            Self::Metadata(frame) => frame.span,
        }
    }

    /// Название контейнера для диагностических сообщений.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Paragraph(_) => "paragraph",
            Self::Heading(_) => "heading",
            Self::Emphasis(_) => "emphasis",
            Self::Strong(_) => "strong",
            Self::Strikethrough(_) => "strikethrough",
            Self::Link(_) => "link",
            Self::Image(_) => "image",
            Self::CodeBlock(_) => "code block",
            Self::Quote(_) => "block quote",
            Self::List(_) => "list",
            Self::ListItem(_) => "list item",
            Self::Table(_) => "table",
            Self::TableHead(_) => "table head",
            Self::TableRow(_) => "table row",
            Self::TableCell(_) => "table cell",
            Self::Metadata(_) => "metadata block",
        }
    }

    /// Вектор inline-элементов контейнера, если он их принимает.
    pub fn inline_content(&mut self) -> Option<&mut Vec<Inline>> {
        match self {
            Self::Paragraph(frame) => Some(&mut frame.content),
            Self::Heading(frame) => Some(&mut frame.content),
            Self::Emphasis(frame) | Self::Strong(frame) | Self::Strikethrough(frame) => {
                Some(&mut frame.content)
            }
            Self::TableCell(frame) => Some(&mut frame.content),
            Self::Link(frame) => Some(&mut frame.content),
            Self::Image(frame) => Some(&mut frame.alt),
            _ => None,
        }
    }

    /// Вектор блоков контейнера, если он их принимает.
    pub fn block_content(&mut self) -> Option<&mut Vec<Spanned<Block>>> {
        match self {
            Self::Quote(frame) => Some(&mut frame.blocks),
            Self::ListItem(frame) => Some(&mut frame.blocks),
            _ => None,
        }
    }
}

/// Кадр абзаца.
#[derive(Debug)]
pub struct ParagraphFrame {
    /// Диапазон абзаца.
    pub span: SourceSpan,
    /// Накопленные inline-элементы.
    pub content: Vec<Inline>,
    /// Абзац открыт неявно — в плотном списке или цитате без явного `Start`.
    pub implicit: bool,
}

/// Кадр заголовка.
#[derive(Debug)]
pub struct HeadingFrame {
    /// Диапазон заголовка.
    pub span: SourceSpan,
    /// Уровень.
    pub level: HeadingLevel,
    /// Явный идентификатор.
    pub id: Option<String>,
    /// Накопленные inline-элементы.
    pub content: Vec<Inline>,
}

/// Кадр inline-контейнера без дополнительных полей.
#[derive(Debug)]
pub struct InlineFrame {
    /// Диапазон контейнера.
    pub span: SourceSpan,
    /// Накопленные inline-элементы.
    pub content: Vec<Inline>,
}

/// Кадр ссылки.
#[derive(Debug)]
pub struct LinkFrame {
    /// Диапазон ссылки.
    pub span: SourceSpan,
    /// Адрес.
    pub destination: String,
    /// Заголовок.
    pub title: Option<String>,
    /// Текст ссылки.
    pub content: Vec<Inline>,
}

/// Кадр изображения.
#[derive(Debug)]
pub struct ImageFrame {
    /// Диапазон изображения.
    pub span: SourceSpan,
    /// Путь к файлу в исходном виде.
    pub source: String,
    /// Заголовок.
    pub title: Option<String>,
    /// Alt-текст.
    pub alt: Vec<Inline>,
}

/// Кадр блока кода.
#[derive(Debug)]
pub struct CodeBlockFrame {
    /// Диапазон блока.
    pub span: SourceSpan,
    /// Язык из info string.
    pub language: Option<String>,
    /// Накопленный код.
    pub code: String,
}

/// Кадр блочного контейнера.
#[derive(Debug)]
pub struct BlockFrame {
    /// Диапазон контейнера.
    pub span: SourceSpan,
    /// Накопленные блоки.
    pub blocks: Vec<Spanned<Block>>,
}

/// Кадр списка.
#[derive(Debug)]
pub struct ListFrame {
    /// Диапазон списка.
    pub span: SourceSpan,
    /// Вид списка.
    pub kind: ListKind,
    /// Накопленные элементы.
    pub items: Vec<ListItem>,
}

/// Кадр элемента списка.
#[derive(Debug)]
pub struct ListItemFrame {
    /// Диапазон элемента.
    pub span: SourceSpan,
    /// Состояние task-list item.
    pub checked: Option<bool>,
    /// Накопленные блоки.
    pub blocks: Vec<Spanned<Block>>,
}

/// Кадр таблицы.
#[derive(Debug)]
pub struct TableFrame {
    /// Диапазон таблицы.
    pub span: SourceSpan,
    /// Выравнивания столбцов.
    pub alignments: Vec<Alignment>,
    /// Строка заголовка.
    pub header: Option<TableRow>,
    /// Строки тела.
    pub rows: Vec<TableRow>,
}

/// Кадр заголовочной секции таблицы: ячейки идут без промежуточной строки.
#[derive(Debug)]
pub struct TableSectionFrame {
    /// Диапазон секции.
    pub span: SourceSpan,
    /// Накопленные ячейки.
    pub cells: Vec<TableCell>,
}

/// Кадр строки таблицы.
#[derive(Debug)]
pub struct TableRowFrame {
    /// Диапазон строки.
    pub span: SourceSpan,
    /// Накопленные ячейки.
    pub cells: Vec<TableCell>,
}

/// Кадр блока метаданных.
#[derive(Debug)]
pub struct MetadataFrame {
    /// Диапазон блока.
    pub span: SourceSpan,
}
