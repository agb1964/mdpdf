//! Блочные элементы документа (ТЗ §10.3–10.9).

use serde::{Deserialize, Serialize};

use crate::ast::Spanned;
use crate::ast::inline::Inline;

/// Блочный элемент.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    /// Заголовок.
    Heading(Heading),
    /// Абзац.
    Paragraph(Paragraph),
    /// Блок кода.
    CodeBlock(CodeBlock),
    /// Цитата.
    Quote(BlockQuote),
    /// Список.
    List(List),
    /// Таблица.
    Table(Table),
    /// Горизонтальная линия.
    ThematicBreak,
}

/// Заголовок (ТЗ §10.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heading {
    /// Уровень заголовка.
    pub level: HeadingLevel,
    /// Содержимое — только inline-элементы.
    pub content: Vec<Inline>,
    /// Явный идентификатор из heading attributes, если он задан.
    pub id: Option<String>,
}

/// Уровень заголовка. Недопустимые уровни непредставимы (ТЗ §10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeadingLevel {
    /// `#`
    H1,
    /// `##`
    H2,
    /// `###`
    H3,
    /// `####`
    H4,
    /// `#####`
    H5,
    /// `######`
    H6,
}

impl HeadingLevel {
    /// Числовой уровень 1–6.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::H1 => 1,
            Self::H2 => 2,
            Self::H3 => 3,
            Self::H4 => 4,
            Self::H5 => 5,
            Self::H6 => 6,
        }
    }
}

/// Абзац (ТЗ §10.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paragraph {
    /// Содержимое абзаца.
    pub content: Vec<Inline>,
}

/// Блок кода (ТЗ §10.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeBlock {
    /// Язык из info string, очищенный от пробелов. Неизвестный язык допустим.
    pub language: Option<String>,
    /// Содержимое без интерпретации.
    pub code: String,
}

/// Цитата (ТЗ §10.7). Может быть вложенной.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockQuote {
    /// Содержимое цитаты.
    pub blocks: Vec<Spanned<Block>>,
}

/// Список (ТЗ §10.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct List {
    /// Вид списка.
    pub kind: ListKind,
    /// Элементы.
    pub items: Vec<ListItem>,
}

/// Вид списка. Конкретный маркер (`-`, `*`, `+`) не хранится: он не влияет
/// на семантику (ТЗ §10.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListKind {
    /// Маркированный список.
    Unordered,
    /// Нумерованный список с начальным номером.
    Ordered {
        /// Номер первого элемента.
        start: u64,
    },
}

/// Элемент списка (ТЗ §10.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    /// Состояние task-list item: `None` — обычный элемент.
    pub checked: Option<bool>,
    /// Содержимое элемента.
    pub blocks: Vec<Spanned<Block>>,
}

/// Таблица (ТЗ §10.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// Выравнивание по столбцам.
    pub alignments: Vec<Alignment>,
    /// Строка заголовка.
    pub header: TableRow,
    /// Строки тела таблицы.
    pub rows: Vec<TableRow>,
}

/// Выравнивание ячеек столбца.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    /// Не задано.
    None,
    /// По левому краю.
    Left,
    /// По центру.
    Center,
    /// По правому краю.
    Right,
}

/// Строка таблицы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    /// Ячейки строки.
    pub cells: Vec<TableCell>,
}

/// Ячейка таблицы. В первой версии содержит только inline-элементы (ТЗ §10.9).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    /// Содержимое ячейки.
    pub content: Vec<Inline>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_levels_are_numbered_one_to_six() {
        let levels = [
            HeadingLevel::H1,
            HeadingLevel::H2,
            HeadingLevel::H3,
            HeadingLevel::H4,
            HeadingLevel::H5,
            HeadingLevel::H6,
        ];
        let numbers: Vec<u8> = levels.iter().map(|level| level.number()).collect();
        assert_eq!(numbers, vec![1, 2, 3, 4, 5, 6]);
    }
}
