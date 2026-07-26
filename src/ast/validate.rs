//! Валидация построенного AST (ТЗ §14). Наполняется на Milestone 1.

use thiserror::Error;

use crate::ast::SourceSpan;

/// Ошибка валидации AST.
#[derive(Debug, Error)]
pub enum AstValidationError {
    /// Нарушен инвариант структуры документа.
    #[error("invalid document structure: {message}")]
    InvalidStructure {
        /// Описание нарушения.
        message: String,
        /// Диапазон исходного Markdown, если он известен.
        span: Option<SourceSpan>,
    },
}
