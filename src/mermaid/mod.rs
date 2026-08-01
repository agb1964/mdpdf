//! Рендеринг диаграмм Mermaid (ТЗ §10.5).
//!
//! Тонкая обёртка над `mermaid-rs-renderer`: на входе — текст диаграммы,
//! на выходе — SVG. Слой ничего не знает о Typst, `pulldown-cmark` и
//! файловой системе; JavaScript, Chromium и внешние процессы не
//! используются (ТЗ §2).

pub mod error;
pub mod layout_fix;
#[cfg(test)]
mod layout_probe;
pub mod limits;
pub mod render;

pub use error::MermaidError;
pub use render::{RenderedDiagram, render};
