//! Ошибки этапа 1 (ТЗ §16). Расширяется на Milestone 1.

use thiserror::Error;

use crate::ast::SourceSpan;
use crate::ast::validate::AstValidationError;

/// Ошибка разбора Markdown.
#[derive(Debug, Error)]
pub enum MarkdownError {
    /// Некорректный вход.
    #[error("invalid input: {message}")]
    InvalidInput {
        /// Описание проблемы.
        message: String,
        /// Диапазон исходного Markdown, если он известен.
        span: Option<SourceSpan>,
    },

    /// Конструкция Markdown не поддерживается первой версией.
    #[error("unsupported Markdown construct: {construct}")]
    UnsupportedConstruct {
        /// Название конструкции.
        construct: &'static str,
        /// Диапазон исходного Markdown.
        span: SourceSpan,
    },

    /// Событие закрытия не соответствует открытому контейнеру.
    #[error("invalid nesting: expected {expected}, found {actual}")]
    InvalidNesting {
        /// Ожидаемый контейнер.
        expected: &'static str,
        /// Фактический контейнер.
        actual: &'static str,
        /// Диапазон исходного Markdown.
        span: SourceSpan,
    },

    /// Поток событий закончился с незакрытым контейнером.
    #[error("incomplete document: {open_construct} was never closed")]
    IncompleteDocument {
        /// Незакрытый контейнер.
        open_construct: &'static str,
        /// Диапазон исходного Markdown.
        span: SourceSpan,
    },

    /// Ошибка валидации построенного AST.
    #[error(transparent)]
    AstValidation(#[from] AstValidationError),

    /// Превышен лимит структуры документа (ТЗ §40).
    #[error("document limit exceeded: {message}")]
    LimitExceeded {
        /// Какой лимит и на чём превышен.
        message: String,
        /// Диапазон исходного Markdown.
        span: SourceSpan,
    },

    /// Нарушен внутренний инвариант парсера.
    #[error("internal parser invariant violated: {message}")]
    InternalInvariant {
        /// Описание инварианта.
        message: String,
    },
}

impl MarkdownError {
    /// Диапазон исходного Markdown, если ошибка к нему привязана.
    ///
    /// Используется для построения префикса `файл:строка:столбец` (ТЗ §16).
    #[must_use]
    pub const fn span(&self) -> Option<SourceSpan> {
        match self {
            Self::InvalidInput { span, .. } => *span,
            Self::UnsupportedConstruct { span, .. }
            | Self::InvalidNesting { span, .. }
            | Self::IncompleteDocument { span, .. }
            | Self::LimitExceeded { span, .. } => Some(*span),
            Self::AstValidation(error) => Some(error.span()),
            Self::InternalInvariant { .. } => None,
        }
    }
}
