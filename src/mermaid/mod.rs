//! Парсер и раскладка подмножества Mermaid (ТЗ §10.5).
//!
//! Слой ничего не знает о Typst, `pulldown-cmark` и файловой системе:
//! на входе — текст диаграммы, на выходе — модель и размещённые
//! примитивы в пунктах. JavaScript и внешние процессы не используются.

pub mod error;
pub mod layout;
pub mod model;
pub mod parser;

pub use error::MermaidError;
pub use layout::{
    LineStyle, PlacedBox, PlacedDiagram, PlacedLabel, PlacedLine, PlacedShape, layout,
};
pub use model::{
    Diagram, Direction, FlowEdge, FlowGraph, FlowNode, FlowSubgraph, MessageStyle, NodeShape,
    Participant, SequenceDiagram, SequenceItem, SequenceMessage,
};
pub use parser::parse;
