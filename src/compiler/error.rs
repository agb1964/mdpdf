//! Ошибки этапа 3. Расширяется на Milestone 3.

use thiserror::Error;

use crate::ast::SourceSpan;
use crate::compiler::diagnostics::Diagnostic;

/// Ошибка компиляции Typst → PDF.
#[derive(Debug, Error)]
pub enum CompileError {
    /// Typst сообщил об ошибках (ТЗ §37).
    #[error("PDF compilation failed")]
    Typst {
        /// Диагностики уровня `Error`.
        diagnostics: Vec<Diagnostic>,
    },

    /// Запрошен файл, не зарегистрированный в виртуальной ФС, или нарушена
    /// политика доступа к каталогу документа (ТЗ §33).
    #[error("access to {path} is not allowed: {message}")]
    ResourceAccess {
        /// Запрошенный путь.
        path: String,
        /// Диапазон в исходном Markdown, если он известен.
        span: Option<SourceSpan>,
        /// Описание нарушения.
        message: String,
    },

    /// Изображение не удалось прочитать или его формат не поддерживается (ТЗ §33.3).
    #[error("image {path} could not be loaded: {message}")]
    Image {
        /// Путь изображения в исходном виде.
        path: String,
        /// Диапазон в исходном Markdown, если он известен.
        span: Option<SourceSpan>,
        /// Описание проблемы.
        message: String,
    },

    /// Превышен лимит ресурсов (ТЗ §40).
    #[error("resource limit exceeded: {message}")]
    LimitExceeded {
        /// Описание превышенного лимита.
        message: String,
    },

    /// Экспортер вернул некорректный PDF (ТЗ §39).
    #[error("PDF export produced invalid output: {message}")]
    InvalidPdf {
        /// Описание проблемы.
        message: String,
    },

    /// Встроенный шрифт не удалось разобрать (ТЗ §34).
    #[error("embedded font could not be parsed: {message}")]
    Font {
        /// Описание проблемы.
        message: String,
    },

    /// Нарушен внутренний инвариант компилятора.
    #[error("internal compiler invariant violated: {message}")]
    InternalInvariant {
        /// Описание инварианта.
        message: String,
    },
}

impl CompileError {
    /// Диапазон в исходном Markdown, если ошибку удалось к нему привязать.
    ///
    /// Позволяет показать `input.md:18:1: ...` вместо позиции в сгенерированном
    /// Typst (ТЗ §37).
    #[must_use]
    pub const fn markdown_span(&self) -> Option<SourceSpan> {
        match self {
            Self::ResourceAccess { span, .. } | Self::Image { span, .. } => *span,
            _ => None,
        }
    }
}
