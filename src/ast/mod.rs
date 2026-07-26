//! Собственное типизированное AST документа (ТЗ §10).
//!
//! AST не содержит Typst-синтаксиса и не зависит от `pulldown-cmark`.
//!
//! Отступление от буквы ТЗ: §10.1 объявляет `blocks: Vec<Block>`, а §11 требует
//! диагностический диапазон у каждого узла верхнего уровня и прямо разрешает
//! обёртку [`Spanned`]. Используется обёртка — так структуры узлов остаются
//! ровно такими, как в §10.

pub mod block;
pub mod document;
pub mod inline;
pub mod metadata;
pub mod validate;

use serde::{Deserialize, Serialize};

/// Диапазон в исходном Markdown, в байтах UTF-8 (ТЗ §11).
///
/// Преобразование в номера строк и столбцов выполняется только при построении
/// диагностического сообщения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Начало диапазона, байты.
    pub start: usize,
    /// Конец диапазона, байты (не включительно).
    pub end: usize,
}

impl SourceSpan {
    /// Создаёт диапазон.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Позиция `start` как пара «строка, столбец», считая с единицы.
    ///
    /// Вызывается только при формировании сообщения об ошибке (ТЗ §11).
    #[must_use]
    pub fn line_column(&self, source: &str) -> (usize, usize) {
        let upto = source.get(..self.start.min(source.len())).unwrap_or(source);
        let line = upto.matches('\n').count() + 1;
        let column = upto
            .rsplit('\n')
            .next()
            .map_or(1, |tail| tail.chars().count() + 1);
        (line, column)
    }
}

impl From<core::ops::Range<usize>> for SourceSpan {
    fn from(range: core::ops::Range<usize>) -> Self {
        Self::new(range.start, range.end)
    }
}

/// Узел вместе с его диапазоном в исходном тексте.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spanned<T> {
    /// Значение узла.
    pub value: T,
    /// Диапазон исходного Markdown.
    pub span: SourceSpan,
}

impl<T> Spanned<T> {
    /// Оборачивает значение вместе с диапазоном.
    pub const fn new(value: T, span: SourceSpan) -> Self {
        Self { value, span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_column_start_at_one() {
        let source = "первая\nвторая\n";
        assert_eq!(SourceSpan::new(0, 1).line_column(source), (1, 1));
    }

    #[test]
    fn column_counts_characters_not_bytes() {
        let source = "абв\nгде";
        // Начало второй строки: 6 байт кириллицы + перевод строки.
        assert_eq!(SourceSpan::new(7, 8).line_column(source), (2, 1));
        // Третий символ второй строки.
        assert_eq!(SourceSpan::new(11, 13).line_column(source), (2, 3));
    }

    #[test]
    fn out_of_range_start_is_clamped() {
        let source = "abc";
        assert_eq!(SourceSpan::new(99, 100).line_column(source), (1, 4));
    }
}
