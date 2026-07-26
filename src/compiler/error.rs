//! Ошибки этапа 3. Расширяется на Milestone 3.

use thiserror::Error;

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

    /// Запрошен файл, не зарегистрированный в виртуальной ФС (ТЗ §33).
    #[error("access to {path} is not allowed")]
    ResourceAccess {
        /// Запрошенный путь.
        path: String,
    },

    /// Изображение не удалось прочитать или его формат не поддерживается (ТЗ §33.3).
    #[error("image {path} could not be loaded: {message}")]
    Image {
        /// Логический путь изображения.
        path: String,
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
