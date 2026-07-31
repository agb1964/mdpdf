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
    /// Узлы по идентификатору (листовые; id подграфов сюда не входят).
    pub nodes: BTreeMap<String, FlowNode>,
    /// Рёбра в порядке объявления. Концы могут ссылаться на id подграфа.
    pub edges: Vec<FlowEdge>,
    /// Подграфы в порядке объявления (включая вложенные).
    pub subgraphs: Vec<FlowSubgraph>,
}

/// Подграф (`subgraph` … `end`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSubgraph {
    /// Идентификатор (цель рёбер и ключ вложенности).
    pub id: String,
    /// Подпись рамки.
    pub label: String,
    /// Прямые потомки: id узлов и/или вложенных подграфов, в порядке
    /// первого появления.
    pub children: Vec<String>,
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
    /// `id[(label)]` — цилиндр (БД).
    Cylinder,
    /// `id[/label/]` — параллелограмм.
    Asymmetric,
}

/// Ребро блок-схемы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowEdge {
    /// Идентификатор узла-источника (или подграфа).
    pub from: String,
    /// Идентификатор узла-приёмника (или подграфа).
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
    /// События сверху вниз: сообщения, заметки, фрагменты alt/else/end.
    pub items: Vec<SequenceItem>,
}

/// Участник диаграммы последовательности.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// Идентификатор.
    pub id: String,
    /// Подпись (по умолчанию — идентификатор).
    pub label: String,
}

/// Событие sequence-диаграммы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceItem {
    /// Сообщение между участниками.
    Message(SequenceMessage),
    /// `Note over A` / `Note over A,B`.
    Note {
        /// Участники, над которыми рисуется заметка.
        over: Vec<String>,
        /// Текст заметки.
        text: String,
    },
    /// Начало `alt` с условием.
    AltStart {
        /// Текст условия (может быть пустым).
        label: String,
    },
    /// Ветка `else` внутри alt.
    Else {
        /// Текст условия else (может быть пустым).
        label: String,
    },
    /// `end` — закрытие alt.
    End,
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
