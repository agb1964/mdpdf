//! Этап 2: AST → Typst source (ТЗ §18–§29).
//!
//! Генератор чистый: не читает файлы, не компилирует Typst, не обращается к сети,
//! не зависит от `pulldown-cmark` и не изменяет AST. Вывод детерминирован (ТЗ §25).

pub mod blocks;
pub mod error;
pub mod escape;
pub mod generator;
pub mod inlines;
pub mod writer;

/// Встроенный Typst-шаблон (ТЗ §21).
pub const TEMPLATE: &str = include_str!("../../assets/template.typ");

/// Префикс виртуальных путей локальных ресурсов (ТЗ §24.6).
pub const RESOURCE_PREFIX: &str = "/mdpdf-resources/";
