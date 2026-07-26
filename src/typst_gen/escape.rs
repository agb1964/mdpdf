//! Экранирование пользовательского ввода (ТЗ §23).
//!
//! Для каждого контекста — отдельная функция; одна универсальная функция
//! на все случаи запрещена:
//!
//! * `string_literal` — строковый литерал Typst;
//! * `text_content` — content-контекст;
//! * `url_literal` — адрес ссылки;
//! * `path_literal` — виртуальный путь ресурса.
//!
//! Ни один пользовательский фрагмент не должен интерпретироваться как Typst-код.
//! Наполняется на Milestone 2.

use thiserror::Error;

/// Ошибка экранирования.
#[derive(Debug, Error)]
pub enum EscapeError {
    /// Значение недопустимо в данном контексте.
    #[error("value cannot be escaped for {context}: {message}")]
    Invalid {
        /// Контекст экранирования.
        context: &'static str,
        /// Описание проблемы.
        message: String,
    },
}
