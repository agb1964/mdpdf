//! Ошибки этапа 2 (ТЗ §27). Расширяется на Milestone 2.

use thiserror::Error;

use crate::ast::SourceSpan;
use crate::typst_gen::escape::EscapeError;

/// Ошибка генерации Typst.
#[derive(Debug, Error)]
pub enum TypstGenerationError {
    /// Документ нарушает предположения генератора.
    #[error("invalid document: {message}")]
    InvalidDocument {
        /// Описание проблемы.
        message: String,
        /// Диапазон исходного Markdown, если он известен.
        span: Option<SourceSpan>,
    },

    /// Узел AST не поддерживается первой версией.
    #[error("unsupported node: {node}")]
    UnsupportedNode {
        /// Название узла.
        node: String,
        /// Диапазон исходного Markdown, если он известен.
        span: Option<SourceSpan>,
    },

    /// Некорректный адрес ссылки.
    #[error("invalid url {value}: {source}")]
    InvalidUrl {
        /// Исходное значение.
        value: String,
        /// Диапазон исходного Markdown, если он известен.
        span: Option<SourceSpan>,
        /// Причина: значение не удалось экранировать.
        #[source]
        source: EscapeError,
    },

    /// Некорректный путь изображения.
    #[error("invalid image path {value}: {message}")]
    InvalidImagePath {
        /// Исходное значение.
        value: String,
        /// Диапазон исходного Markdown, если он известен.
        span: Option<SourceSpan>,
        /// Описание проблемы.
        message: String,
    },

    /// Недопустимое значение параметра рендеринга (ТЗ §20.2).
    #[error("invalid option {name}: {message}")]
    InvalidOption {
        /// Имя параметра.
        name: String,
        /// Описание проблемы.
        message: String,
    },

    /// Значение не удалось безопасно экранировать.
    #[error("escaping failed in {context}: {message}")]
    Escaping {
        /// Контекст экранирования.
        context: String,
        /// Описание проблемы.
        message: String,
    },

    /// Нарушен внутренний инвариант генератора.
    #[error("internal generator invariant violated: {message}")]
    InternalInvariant {
        /// Описание инварианта.
        message: String,
    },
}
