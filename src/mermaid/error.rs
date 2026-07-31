//! Ошибки рендеринга диаграмм Mermaid (ТЗ §10.5).
//!
//! Ни одна из них не роняет сборку: генератор деградирует до обычного
//! блока кода и выводит предупреждение (ТЗ §10.5.5).

use thiserror::Error;

/// Ошибка рендеринга диаграммы Mermaid.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MermaidError {
    /// Исходник больше лимита (ТЗ §15).
    #[error("diagram source is {size} bytes, limit is {limit}")]
    SourceTooLarge {
        /// Фактический размер.
        size: usize,
        /// Лимит.
        limit: usize,
    },
    /// Отрендеренный SVG больше лимита (ТЗ §15).
    #[error("rendered SVG is {size} bytes, limit is {limit}")]
    SvgTooLarge {
        /// Фактический размер.
        size: usize,
        /// Лимит.
        limit: usize,
    },
    /// Рендерер не смог разобрать или разложить диаграмму.
    ///
    /// Текст берётся из `Display` ошибки `mermaid-rs-renderer`; сам тип
    /// ошибки наружу не протекает, `anyhow` в публичный API не попадает.
    #[error("{message}")]
    Render {
        /// Сообщение рендерера.
        message: String,
    },
    /// В выходном SVG нашлась ссылка на внешний ресурс (ТЗ §33.3).
    ///
    /// Так выглядит, например, `click A "https://…"`: конструкция
    /// синтаксически корректна, но нарушает политику ресурсов.
    #[error("SVG references an external resource ({reference})")]
    ExternalReference {
        /// Значение атрибута для сообщения.
        reference: String,
    },
    /// Рендерер запаниковал.
    ///
    /// Полноценный вариант ошибки, а не заплатка: контракт §10.5.5 требует
    /// «никогда не фатально», а раскладка рендерера — сторонний код на
    /// `f32`, на который направлен fuzz-таргет.
    #[error("diagram renderer panicked")]
    Panicked,
}
