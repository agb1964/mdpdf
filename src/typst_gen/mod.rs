//! Этап 2: AST → Typst source (ТЗ §18–§29).
//!
//! Генератор чистый: не читает файлы, не компилирует Typst, не обращается к сети,
//! не зависит от `pulldown-cmark` и не изменяет AST. Вывод детерминирован (ТЗ §25).

pub mod blocks;
pub mod diagram;
pub mod error;
pub mod escape;
pub mod generator;
pub mod inlines;
pub mod writer;

/// Встроенный Typst-шаблон (ТЗ §21).
pub const TEMPLATE: &str = include_str!("../../assets/template.typ");

/// Префикс виртуальных путей локальных ресурсов (ТЗ §24.6).
pub const RESOURCE_PREFIX: &str = "/mdpdf-resources/";

/// Виртуальный путь следующего ресурса.
///
/// Номера выдаются последовательно в порядке обхода AST из общего счётчика,
/// поэтому вывод детерминирован (ТЗ §25). `stem_prefix` разделяет семейства
/// ресурсов (`""` для картинок из Markdown, `"mermaid-"` для диаграмм);
/// номера внутри одного семейства из-за общего счётчика не обязаны идти
/// подряд — важна только воспроизводимость.
pub(crate) fn next_logical_path(
    resources: &[generator::ResourceReference],
    stem_prefix: &str,
    extension: &str,
) -> String {
    let index = resources.len() + 1;
    format!("{RESOURCE_PREFIX}{stem_prefix}{index:06}.{extension}")
}
