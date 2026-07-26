//! Этап 3: Typst source → PDF (ТЗ §30–§41).
//!
//! Это **единственный** модуль проекта, которому разрешено импортировать
//! `typst::*` и `typst_pdf::*` (ТЗ §31). Внешний Typst CLI не вызывается,
//! сеть и системные шрифты не используются.

pub mod diagnostics;
pub mod error;
pub mod files;
pub mod fonts;
pub mod pdf;
pub mod world;

/// Лимиты ресурсов первой версии (ТЗ §40).
pub mod limits {
    /// Максимальное число узлов AST.
    pub const MAX_AST_NODES: usize = 1_000_000;
    /// Максимальная глубина вложенности.
    pub const MAX_NESTING_DEPTH: usize = 128;
    /// Максимальное число изображений в документе.
    pub const MAX_IMAGES: usize = 1_000;
    /// Максимальный размер одного изображения, байты.
    pub const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
    /// Максимальный суммарный размер изображений, байты.
    pub const MAX_TOTAL_IMAGE_BYTES: usize = 256 * 1024 * 1024;
    /// Максимальная длина URL, байты.
    pub const MAX_URL_BYTES: usize = 16 * 1024;
    /// Максимальная длина одного текстового узла, байты.
    pub const MAX_TEXT_NODE_BYTES: usize = 16 * 1024 * 1024;
}
