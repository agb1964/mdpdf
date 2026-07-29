//! Ошибки разбора диаграмм Mermaid (ТЗ §10.5).
//!
//! Ошибка разбора не роняет сборку: генератор деградирует до обычного
//! блока кода и выводит предупреждение.

use thiserror::Error;

/// Ошибка разбора диаграммы Mermaid.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MermaidError {
    /// Диаграмма пуста.
    #[error("diagram is empty")]
    Empty,
    /// Тип диаграммы вне подмножества.
    #[error("unsupported diagram type: {found}")]
    UnsupportedDiagramType {
        /// Первое слово исходника.
        found: String,
    },
    /// Конструкция вне подмножества.
    #[error("line {line}: unsupported feature: {feature}")]
    UnsupportedFeature {
        /// Номер строки (1-based).
        line: usize,
        /// Краткое имя конструкции.
        feature: String,
    },
    /// Синтаксическая ошибка.
    #[error("line {line}: {reason}")]
    Syntax {
        /// Номер строки (1-based).
        line: usize,
        /// Описание.
        reason: String,
    },
    /// Превышен защитный лимит (ТЗ §15).
    #[error("diagram limit exceeded: {what}")]
    LimitExceeded {
        /// Что превышено.
        what: &'static str,
    },
}
