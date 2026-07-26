//! Этап 1: Markdown → собственное AST (ТЗ §8–§17).
//!
//! Модуль не генерирует Typst, не компилирует PDF и не выполняет файловый вывод.
//! Типы `pulldown-cmark` наружу не экспортируются.

pub mod builder;
pub mod error;
pub mod parser;
pub mod state;
