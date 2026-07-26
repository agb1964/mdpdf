//! Собственное типизированное AST документа (ТЗ §10).
//!
//! AST не содержит Typst-синтаксиса и не зависит от `pulldown-cmark`.
//! Модель наполняется на Milestone 1.

pub mod block;
pub mod document;
pub mod inline;
pub mod metadata;
pub mod validate;

/// Диапазон в исходном Markdown, в байтах UTF-8 (ТЗ §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceSpan {
    /// Начало диапазона, байты.
    pub start: usize,
    /// Конец диапазона, байты (не включительно).
    pub end: usize,
}

/// Узел вместе с его диапазоном в исходном тексте.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Spanned<T> {
    /// Значение узла.
    pub value: T,
    /// Диапазон исходного Markdown.
    pub span: SourceSpan,
}
