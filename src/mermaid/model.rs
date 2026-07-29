//! Модель диаграмм Mermaid (ТЗ §10.5).
//!
//! Собственные типы подмножества: никаких зависимостей от Typst,
//! `pulldown-cmark` и файловой системы.

use std::collections::BTreeMap;

/// Распознанная диаграмма.
#[derive(Debug, Clone, PartialEq)]
pub enum Diagram {
    /// Блок-схема (`graph`/`flowchart`).
    Flowchart(FlowGraph),
    /// Диаграмма последовательности (`sequenceDiagram`).
    Sequence(SequenceDiagram),
}

/// Направление блок-схемы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Сверху вниз (`TD`/`TB`).
    TopDown,
    /// Слева направо (`LR`).
    LeftRight,
}

/// Блок-схема. Узлы упорядочены по идентификатору (BTreeMap) ради
/// детерминированности обхода; порядок появления хранится в
/// [`FlowNode::order`] для tie-break при раскладке.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowGraph {
    /// Направление.
    pub direction: Direction,
    /// Узлы по идентификатору.
    pub nodes: BTreeMap<String, FlowNode>,
    /// Рёбра в порядке объявления.
    pub edges: Vec<FlowEdge>,
}

/// Узел блок-схемы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowNode {
    /// Подпись (по умолчанию — идентификатор).
    pub label: String,
    /// Форма.
    pub shape: NodeShape,
    /// Порядок первого появления в исходнике.
    pub order: usize,
}

/// Форма узла.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeShape {
    /// `id[label]`.
    Rect,
    /// `id(label)`.
    Rounded,
    /// `id{label}`.
    Diamond,
    /// `id((label))`.
    Circle,
}

/// Ребро блок-схемы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowEdge {
    /// Идентификатор узла-источника.
    pub from: String,
    /// Идентификатор узла-приёмника.
    pub to: String,
    /// Подпись ребра.
    pub label: Option<String>,
    /// `true` — со стрелкой (`-->`), `false` — без (`---`).
    pub arrow: bool,
}

/// Диаграмма последовательности.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SequenceDiagram {
    /// Участники в порядке объявления/первого появления.
    pub participants: Vec<Participant>,
    /// Сообщения сверху вниз.
    pub messages: Vec<SequenceMessage>,
}

/// Участник диаграммы последовательности.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// Идентификатор.
    pub id: String,
    /// Подпись (по умолчанию — идентификатор).
    pub label: String,
}

/// Сообщение диаграммы последовательности.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceMessage {
    /// Идентификатор отправителя.
    pub from: String,
    /// Идентификатор получателя.
    pub to: String,
    /// Подпись сообщения.
    pub label: String,
    /// Стиль линии.
    pub style: MessageStyle,
}

/// Стиль линии сообщения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStyle {
    /// `->` — сплошная, открытая стрелка.
    Solid,
    /// `->>` — сплошная, закрашенная стрелка.
    SolidFilled,
    /// `-->` — пунктирная, открытая стрелка.
    Dashed,
    /// `-->>` — пунктирная, закрашенная стрелка.
    DashedFilled,
}
