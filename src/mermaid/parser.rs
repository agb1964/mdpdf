//! Парсер подмножества Mermaid (ТЗ §10.5).
//!
//! Построчный разбор: Mermaid для flowchart и sequence ориентирован на
//! строки. Комментарии `%%` отрезаются, `;` разделяет операторы.
//! Разбор детерминирован: узлы хранятся в BTreeMap, порядок появления —
//! явным счётчиком.

use crate::mermaid::error::MermaidError;
use crate::mermaid::model::{
    Diagram, Direction, FlowEdge, FlowGraph, FlowNode, FlowSubgraph, MessageStyle, NodeShape,
    Participant, SequenceDiagram, SequenceItem, SequenceMessage,
};

/// Максимальный размер исходника диаграммы (ТЗ §15).
const MAX_SOURCE_BYTES: usize = 64 * 1024;
/// Максимум узлов/участников в диаграмме (ТЗ §15).
const MAX_NODES: usize = 500;
/// Максимум рёбер/сообщений в диаграмме (ТЗ §15).
const MAX_EDGES: usize = 1000;
/// Максимум вложенности subgraph / alt.
const MAX_NESTING: usize = 32;

/// Разбирает исходник диаграммы Mermaid.
///
/// # Errors
///
/// [`MermaidError`], если исходник пуст, превышает лимиты, выходит за
/// пределы подмножества или содержит синтаксическую ошибку.
pub fn parse(source: &str) -> Result<Diagram, MermaidError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(MermaidError::LimitExceeded {
            what: "diagram source exceeds 64 KiB",
        });
    }
    let statements = collect_statements(source);
    let Some((line, first)) = statements.first() else {
        return Err(MermaidError::Empty);
    };
    let mut head = first.splitn(2, char::is_whitespace);
    let keyword = head.next().unwrap_or_default();
    let rest = head.next().unwrap_or_default().trim();
    match keyword {
        "graph" | "flowchart" => {
            let direction = match rest {
                "" | "TD" | "TB" => Direction::TopDown,
                "LR" => Direction::LeftRight,
                "BT" | "RL" => {
                    return Err(MermaidError::UnsupportedFeature {
                        line: *line,
                        feature: format!("direction {rest}"),
                    });
                }
                other => {
                    return Err(MermaidError::Syntax {
                        line: *line,
                        reason: format!("unknown direction {other:?}"),
                    });
                }
            };
            parse_flowchart(&statements[1..], direction)
        }
        "sequenceDiagram" => {
            if !rest.is_empty() {
                return Err(MermaidError::Syntax {
                    line: *line,
                    reason: format!("unexpected text after sequenceDiagram: {rest:?}"),
                });
            }
            parse_sequence(&statements[1..])
        }
        other => Err(MermaidError::UnsupportedDiagramType {
            found: other.to_owned(),
        }),
    }
}

/// Операторы: комментарии отрезаны, пустое выкинуто, `;` — разделитель.
fn collect_statements(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        let line = raw.split_once("%%").map_or(raw, |(head, _)| head);
        for part in line.split(';') {
            let text = part.trim();
            if !text.is_empty() {
                out.push((index + 1, text.to_owned()));
            }
        }
    }
    out
}

/// Нормализует пользовательскую подпись: HTML-переносы `<br/>` → `\n`.
/// Обрамляющие кавычки формы `["…"]` снимает разбор формы, а не эта
/// функция — в `|…|` и `[…]` кавычки входят в текст подписи.
fn normalize_label(raw: &str) -> String {
    let trimmed = raw.trim();
    // `<br/>`, `<br />`, `<br>`, регистр неважен.
    let mut out = String::with_capacity(trimmed.len());
    let lower = trimmed.to_ascii_lowercase();
    let bytes = trimmed.as_bytes();
    let lower_bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if lower_bytes[i] == b'<' && lower_bytes.get(i..).is_some_and(|s| s.starts_with(b"<br")) {
            let after_br = i + 3;
            let mut j = after_br;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'/' {
                j += 1;
            }
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'>' {
                out.push('\n');
                i = j + 1;
                continue;
            }
        }
        // trimmed — UTF-8; ASCII-тег обрабатываем по байтам, иначе char.
        let ch = trimmed[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

// ---------------------------------------------------------------------------
// Flowchart
// ---------------------------------------------------------------------------

/// Накопитель блок-схемы: регистрирует узлы с порядком появления и стек
/// подграфов.
struct FlowBuilder {
    graph: FlowGraph,
    next_order: usize,
    /// Индексы открытых подграфов в `graph.subgraphs`.
    subgraph_stack: Vec<usize>,
}

impl FlowBuilder {
    fn new(direction: Direction) -> Self {
        Self {
            graph: FlowGraph {
                direction,
                nodes: std::collections::BTreeMap::new(),
                edges: Vec::new(),
                subgraphs: Vec::new(),
            },
            next_order: 0,
            subgraph_stack: Vec::new(),
        }
    }

    fn is_subgraph_id(&self, id: &str) -> bool {
        self.graph.subgraphs.iter().any(|sg| sg.id == id)
    }

    /// Регистрирует листовой узел. Явные подпись/форма перезаписывают
    /// прежние (как в Mermaid), голое упоминание — только создаёт узел.
    /// Id известного подграфа листовым узлом не становится.
    fn register(&mut self, id: &str, label: Option<String>, shape: Option<NodeShape>) {
        if self.is_subgraph_id(id) {
            // Ребро к/от подграфа: листовой узел не создаём.
            return;
        }
        if let Some(node) = self.graph.nodes.get_mut(id) {
            if let Some(label) = label {
                node.label = label;
            }
            if let Some(shape) = shape {
                node.shape = shape;
            }
            self.track_child(id);
            return;
        }
        self.graph.nodes.insert(
            id.to_owned(),
            FlowNode {
                label: label.unwrap_or_else(|| id.to_owned()),
                shape: shape.unwrap_or(NodeShape::Rect),
                order: self.next_order,
            },
        );
        self.next_order += 1;
        self.track_child(id);
    }

    /// Добавляет id в текущий открытый подграф (без дублей).
    fn track_child(&mut self, id: &str) {
        let Some(&sg_index) = self.subgraph_stack.last() else {
            return;
        };
        if let Some(sg) = self.graph.subgraphs.get_mut(sg_index)
            && !sg.children.iter().any(|c| c == id)
        {
            sg.children.push(id.to_owned());
        }
    }

    fn start_subgraph(
        &mut self,
        line: usize,
        id: String,
        label: String,
    ) -> Result<(), MermaidError> {
        if self.subgraph_stack.len() >= MAX_NESTING {
            return Err(MermaidError::LimitExceeded {
                what: "subgraph nesting exceeds 32",
            });
        }
        if self.is_subgraph_id(&id) {
            return Err(MermaidError::Syntax {
                line,
                reason: format!("duplicate subgraph id {id:?}"),
            });
        }
        // Если id уже был листовым узлом (ребро до объявления subgraph),
        // он становится контейнером.
        self.graph.nodes.remove(&id);
        let index = self.graph.subgraphs.len();
        self.graph.subgraphs.push(FlowSubgraph {
            id: id.clone(),
            label,
            children: Vec::new(),
        });
        self.track_child(&id);
        self.subgraph_stack.push(index);
        Ok(())
    }

    fn end_subgraph(&mut self, line: usize) -> Result<(), MermaidError> {
        if self.subgraph_stack.pop().is_none() {
            return Err(MermaidError::Syntax {
                line,
                reason: "end without open subgraph".to_owned(),
            });
        }
        Ok(())
    }
}

fn parse_flowchart(
    statements: &[(usize, String)],
    direction: Direction,
) -> Result<Diagram, MermaidError> {
    let mut builder = FlowBuilder::new(direction);
    for (line, statement) in statements {
        parse_flow_statement(&mut builder, *line, statement)?;
        if builder.graph.nodes.len() + builder.graph.subgraphs.len() > MAX_NODES {
            return Err(MermaidError::LimitExceeded {
                what: "more than 500 nodes",
            });
        }
        if builder.graph.edges.len() > MAX_EDGES {
            return Err(MermaidError::LimitExceeded {
                what: "more than 1000 edges",
            });
        }
    }
    if !builder.subgraph_stack.is_empty() {
        return Err(MermaidError::Syntax {
            line: statements.last().map_or(1, |(l, _)| *l),
            reason: "unclosed subgraph".to_owned(),
        });
    }
    if builder.graph.nodes.is_empty() && builder.graph.subgraphs.is_empty() {
        return Err(MermaidError::Empty);
    }
    Ok(Diagram::Flowchart(builder.graph))
}

/// Оператор блок-схемы: subgraph/end, объявление узла или цепочка рёбер.
fn parse_flow_statement(
    builder: &mut FlowBuilder,
    line: usize,
    statement: &str,
) -> Result<(), MermaidError> {
    const UNSUPPORTED: [(&str, &str); 5] = [
        ("style", "style"),
        ("classDef", "classDef"),
        ("class", "class"),
        ("click", "click"),
        ("linkStyle", "linkStyle"),
    ];
    for (prefix, feature) in UNSUPPORTED {
        // Ключевое слово — только целиком: `ending`, `styleGuide` и
        // `classroom` — обычные идентификаторы, а не конструкции.
        let is_keyword = statement == prefix
            || statement
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(char::is_whitespace));
        if is_keyword {
            return Err(MermaidError::UnsupportedFeature {
                line,
                feature: feature.to_owned(),
            });
        }
    }

    if statement == "end"
        || statement
            .strip_prefix("end")
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
    {
        return builder.end_subgraph(line);
    }

    if let Some(rest) = statement.strip_prefix("subgraph") {
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            // `subgraphX` — не ключевое слово.
        } else {
            let (id, label) = parse_subgraph_header(rest.trim(), line)?;
            return builder.start_subgraph(line, id, label);
        }
    }

    let mut rest = statement;
    let mut prev = scan_flow_node(builder, &mut rest, line)?;
    while !rest.trim_start().is_empty() {
        let edge = scan_flow_edge(&mut rest, line)?;
        let next = scan_flow_node(builder, &mut rest, line)?;
        builder.graph.edges.push(FlowEdge {
            from: prev,
            to: next.clone(),
            label: edge.label,
            arrow: edge.arrow,
        });
        prev = next;
    }
    Ok(())
}

/// `subgraph id["title"]` / `subgraph id[title]` / `subgraph id` / `subgraph title`.
fn parse_subgraph_header(rest: &str, line: usize) -> Result<(String, String), MermaidError> {
    if rest.is_empty() {
        return Err(MermaidError::Syntax {
            line,
            reason: "subgraph requires an id or title".to_owned(),
        });
    }
    let mut cursor = rest;
    // Пробуем id + опциональная форма/подпись.
    if let Ok(id) = scan_id(&mut cursor, line) {
        let after_id = cursor.trim_start();
        if after_id.is_empty() {
            return Ok((id.clone(), id));
        }
        if after_id.starts_with('[') || after_id.starts_with('(') || after_id.starts_with('{') {
            let (label, shape) = scan_node_shape(&mut cursor, line)?;
            let _ = shape;
            let label = label.unwrap_or_else(|| id.clone());
            if !cursor.trim().is_empty() {
                return Err(MermaidError::Syntax {
                    line,
                    reason: format!("unexpected text after subgraph header: {cursor:?}"),
                });
            }
            return Ok((id, label));
        }
        // `subgraph id remaining title words`
        let label = normalize_label(after_id);
        if !label.is_empty() {
            return Ok((id, label));
        }
        return Ok((id.clone(), id));
    }
    // Без id: весь rest — подпись, id = slug.
    let label = normalize_label(rest);
    let id = subgraph_id_from_title(&label);
    if id.is_empty() {
        return Err(MermaidError::Syntax {
            line,
            reason: "subgraph title yields empty id".to_owned(),
        });
    }
    Ok((id, label))
}

/// Стабильный id из заголовка без явного идентификатора.
fn subgraph_id_from_title(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
        } else if (c.is_whitespace() || c == '-') && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
}

/// Узел: идентификатор плюс необязательная форма с подписью.
fn scan_flow_node(
    builder: &mut FlowBuilder,
    rest: &mut &str,
    line: usize,
) -> Result<String, MermaidError> {
    let id = scan_id(rest, line)?;
    let (label, shape) = scan_node_shape(rest, line)?;
    builder.register(&id, label, shape);
    Ok(id)
}

/// Идентификатор: буквы/цифры/подчёркивание (Unicode-буквы допустимы).
fn scan_id(rest: &mut &str, line: usize) -> Result<String, MermaidError> {
    *rest = rest.trim_start();
    let len = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if len == 0 {
        return Err(MermaidError::Syntax {
            line,
            reason: format!("expected node id, found {rest:?}"),
        });
    }
    let id = rest[..len].to_owned();
    *rest = &rest[len..];
    Ok(id)
}

/// Необязательный суффикс формы после идентификатора.
fn scan_node_shape(
    rest: &mut &str,
    line: usize,
) -> Result<(Option<String>, Option<NodeShape>), MermaidError> {
    let s = rest.trim_start();
    let (label, shape, after) = if let Some(inner) = s.strip_prefix("((") {
        let (label, after) = take_until(inner, "))", line)?;
        (label, NodeShape::Circle, after)
    } else if let Some(inner) = s.strip_prefix("[(") {
        let (label, after) = take_until(inner, ")]", line)?;
        (label, NodeShape::Cylinder, after)
    } else if s.starts_with("[[") {
        return Err(MermaidError::UnsupportedFeature {
            line,
            feature: "subroutine shape".to_owned(),
        });
    } else if let Some(inner) = s.strip_prefix("[/") {
        let (label, after) = take_until(inner, "/]", line)?;
        (label, NodeShape::Asymmetric, after)
    } else if let Some(inner) = s.strip_prefix("[\"") {
        let (label, after) = take_until(inner, "\"]", line)?;
        (label, NodeShape::Rect, after)
    } else if let Some(inner) = s.strip_prefix("['") {
        let (label, after) = take_until(inner, "']", line)?;
        (label, NodeShape::Rect, after)
    } else if let Some(inner) = s.strip_prefix('[') {
        let (label, after) = take_until(inner, "]", line)?;
        (label, NodeShape::Rect, after)
    } else if let Some(inner) = s.strip_prefix('(') {
        let (label, after) = take_until(inner, ")", line)?;
        (label, NodeShape::Rounded, after)
    } else if let Some(inner) = s.strip_prefix('{') {
        let (label, after) = take_until(inner, "}", line)?;
        (label, NodeShape::Diamond, after)
    } else {
        return Ok((None, None));
    };
    *rest = after;
    let label = normalize_label(label);
    Ok((
        if label.is_empty() { None } else { Some(label) },
        Some(shape),
    ))
}

/// Текст до терминатора включительно; возвращает текст и остаток.
fn take_until<'a>(
    s: &'a str,
    terminator: &str,
    line: usize,
) -> Result<(&'a str, &'a str), MermaidError> {
    let Some(end) = s.find(terminator) else {
        return Err(MermaidError::Syntax {
            line,
            reason: format!("unterminated label, expected {terminator:?}"),
        });
    };
    Ok((&s[..end], &s[end + terminator.len()..]))
}

/// Разобранное ребро цепочки.
struct ScannedEdge {
    label: Option<String>,
    arrow: bool,
}

/// Ребро: `-->`, `---`, `-- text -->`, `-->|text|`.
fn scan_flow_edge(rest: &mut &str, line: usize) -> Result<ScannedEdge, MermaidError> {
    *rest = rest.trim_start();
    let s = *rest;
    if s.starts_with("-.") || s.starts_with("==") {
        return Err(MermaidError::UnsupportedFeature {
            line,
            feature: "dotted/thick edge".to_owned(),
        });
    }
    if s.starts_with('&') {
        return Err(MermaidError::UnsupportedFeature {
            line,
            feature: "& fork".to_owned(),
        });
    }
    if let Some(after) = s.strip_prefix("-->") {
        let (label, after) = scan_pipe_label(after, line)?;
        *rest = after;
        return Ok(ScannedEdge { label, arrow: true });
    }
    if let Some(after) = s.strip_prefix("---") {
        if after.starts_with('-') {
            return Err(MermaidError::UnsupportedFeature {
                line,
                feature: "long edge".to_owned(),
            });
        }
        let (label, after) = scan_pipe_label(after, line)?;
        *rest = after;
        return Ok(ScannedEdge {
            label,
            arrow: false,
        });
    }
    if let Some(after) = s.strip_prefix("--") {
        if after.starts_with('-') {
            return Err(MermaidError::UnsupportedFeature {
                line,
                feature: "long edge".to_owned(),
            });
        }
        // Форма `-- text -->` / `-- text ---`.
        let arrow_pos = after.find("-->");
        let open_pos = after.find("---");
        let (pos, len, arrow) = match (arrow_pos, open_pos) {
            (Some(a), Some(o)) if a <= o => (a, 3, true),
            (Some(a), Some(_)) => (arrow_pos.unwrap_or(a), 3, true),
            (Some(a), None) => (a, 3, true),
            (None, Some(o)) => (o, 3, false),
            (None, None) => {
                return Err(MermaidError::Syntax {
                    line,
                    reason: format!("expected edge terminator in {s:?}"),
                });
            }
        };
        let label = after[..pos].trim();
        if label.is_empty() {
            return Err(MermaidError::Syntax {
                line,
                reason: format!("missing edge label in {s:?}"),
            });
        }
        *rest = &after[pos + len..];
        return Ok(ScannedEdge {
            label: Some(normalize_label(label)),
            arrow,
        });
    }
    Err(MermaidError::Syntax {
        line,
        reason: format!("expected edge operator, found {s:?}"),
    })
}

/// Подпись `|text|` после оператора ребра.
fn scan_pipe_label(s: &str, line: usize) -> Result<(Option<String>, &str), MermaidError> {
    let Some(inner) = s.strip_prefix('|') else {
        return Ok((None, s));
    };
    let (label, after) = take_until(inner, "|", line)?;
    Ok((Some(normalize_label(label)), after))
}

// ---------------------------------------------------------------------------
// Sequence
// ---------------------------------------------------------------------------

/// Ключевые слова sequence вне подмножества (сравнение в нижнем регистре).
const SEQUENCE_UNSUPPORTED: [&str; 15] = [
    "loop",
    "opt",
    "par",
    "and",
    "critical",
    "break",
    "activate",
    "deactivate",
    "autonumber",
    "actor",
    "box",
    "rect",
    "create",
    "destroy",
    "title",
];

fn parse_sequence(statements: &[(usize, String)]) -> Result<Diagram, MermaidError> {
    let mut diagram = SequenceDiagram::default();
    let mut alt_depth = 0usize;
    for (line, statement) in statements {
        let first_word = statement
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if SEQUENCE_UNSUPPORTED.contains(&first_word.as_str()) {
            return Err(MermaidError::UnsupportedFeature {
                line: *line,
                feature: first_word,
            });
        }
        // Хвост после первого слова — по фактическому префиксу исходной
        // строки (ключевые слова case-insensitive, срез — по длине слова).
        let rest_after_kw = statement
            .split_once(char::is_whitespace)
            .map(|(_, rest)| rest.trim_start())
            .unwrap_or("");
        if first_word == "participant" {
            parse_participant(&mut diagram, *line, rest_after_kw)?;
        } else if first_word == "note" {
            parse_note(&mut diagram, *line, statement)?;
        } else if first_word == "alt" {
            if alt_depth >= MAX_NESTING {
                return Err(MermaidError::LimitExceeded {
                    what: "alt nesting exceeds 32",
                });
            }
            diagram.items.push(SequenceItem::AltStart {
                label: rest_after_kw.to_owned(),
            });
            alt_depth += 1;
        } else if first_word == "else" {
            if alt_depth == 0 {
                return Err(MermaidError::Syntax {
                    line: *line,
                    reason: "else without open alt".to_owned(),
                });
            }
            diagram.items.push(SequenceItem::Else {
                label: rest_after_kw.to_owned(),
            });
        } else if first_word == "end" {
            if alt_depth == 0 {
                return Err(MermaidError::Syntax {
                    line: *line,
                    reason: "end without open alt".to_owned(),
                });
            }
            diagram.items.push(SequenceItem::End);
            alt_depth -= 1;
        } else {
            parse_sequence_message(&mut diagram, *line, statement)?;
        }
        if diagram.participants.len() > MAX_NODES {
            return Err(MermaidError::LimitExceeded {
                what: "more than 500 participants",
            });
        }
        if diagram.items.len() > MAX_EDGES {
            return Err(MermaidError::LimitExceeded {
                what: "more than 1000 messages",
            });
        }
    }
    if alt_depth != 0 {
        return Err(MermaidError::Syntax {
            line: statements.last().map_or(1, |(l, _)| *l),
            reason: "unclosed alt".to_owned(),
        });
    }
    if diagram.participants.is_empty() {
        return Err(MermaidError::Empty);
    }
    Ok(Diagram::Sequence(diagram))
}

/// `participant A` или `participant A as Подпись`.
fn parse_participant(
    diagram: &mut SequenceDiagram,
    line: usize,
    rest: &str,
) -> Result<(), MermaidError> {
    let (id, label) = match rest.split_once(" as ") {
        Some((id, label)) => (id.trim(), label.trim()),
        None => (rest.trim(), rest.trim()),
    };
    if id.is_empty() || !id.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(MermaidError::Syntax {
            line,
            reason: format!("invalid participant id in {rest:?}"),
        });
    }
    declare_participant(diagram, id, &normalize_label(label));
    Ok(())
}

/// `Note over A: text` / `Note over A,B: text` (регистр Note/note).
fn parse_note(
    diagram: &mut SequenceDiagram,
    line: usize,
    statement: &str,
) -> Result<(), MermaidError> {
    // Снимаем первое слово (Note/note).
    let rest = statement
        .split_once(char::is_whitespace)
        .map(|(_, r)| r.trim_start())
        .unwrap_or("");
    let rest_lower = rest.to_ascii_lowercase();
    let Some(after_over) = rest_lower
        .strip_prefix("over")
        .filter(|s| s.is_empty() || s.starts_with(char::is_whitespace))
        .map(|_| rest["over".len()..].trim_start())
    else {
        return Err(MermaidError::UnsupportedFeature {
            line,
            feature: "note placement (only \"over\" is supported)".to_owned(),
        });
    };
    let Some((targets, text)) = after_over.split_once(':') else {
        return Err(MermaidError::Syntax {
            line,
            reason: format!("expected ':' after Note over targets in {statement:?}"),
        });
    };
    let over: Vec<String> = targets
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if over.is_empty() {
        return Err(MermaidError::Syntax {
            line,
            reason: "Note over requires at least one participant".to_owned(),
        });
    }
    for id in &over {
        if !id.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(MermaidError::Syntax {
                line,
                reason: format!("invalid participant id in Note over: {id:?}"),
            });
        }
        register_participant(diagram, id, id);
    }
    diagram.items.push(SequenceItem::Note {
        over,
        text: normalize_label(text.trim()),
    });
    Ok(())
}

/// Явно объявляет участника. Если он уже был неявно создан сообщением,
/// обновляет подпись, но сохраняет порядок первого появления.
fn declare_participant(diagram: &mut SequenceDiagram, id: &str, label: &str) {
    if let Some(participant) = diagram.participants.iter_mut().find(|p| p.id == id) {
        participant.label = label.to_owned();
        return;
    }
    diagram.participants.push(Participant {
        id: id.to_owned(),
        label: label.to_owned(),
    });
}

/// Регистрирует участника, если он ещё не известен.
fn register_participant(diagram: &mut SequenceDiagram, id: &str, label: &str) {
    if diagram.participants.iter().any(|p| p.id == id) {
        return;
    }
    diagram.participants.push(Participant {
        id: id.to_owned(),
        label: label.to_owned(),
    });
}

/// Сообщение: `A->B: text`, `A->>B: text`, `A-->B: text`, `A-->>B: text`.
fn parse_sequence_message(
    diagram: &mut SequenceDiagram,
    line: usize,
    statement: &str,
) -> Result<(), MermaidError> {
    let mut rest = statement;
    let from = scan_id(&mut rest, line)?;
    let (style, after) = if let Some(after) = rest.strip_prefix("-->>") {
        (MessageStyle::DashedFilled, after)
    } else if let Some(after) = rest.strip_prefix("->>") {
        (MessageStyle::SolidFilled, after)
    } else if let Some(after) = rest.strip_prefix("-->") {
        (MessageStyle::Dashed, after)
    } else if let Some(after) = rest.strip_prefix("->") {
        (MessageStyle::Solid, after)
    } else if rest.starts_with("--x") || rest.starts_with("-x") {
        return Err(MermaidError::UnsupportedFeature {
            line,
            feature: "cross message".to_owned(),
        });
    } else {
        return Err(MermaidError::Syntax {
            line,
            reason: format!("expected message operator in {statement:?}"),
        });
    };
    let mut after = after;
    let to = scan_id(&mut after, line)?;
    let after = after.trim_start();
    let Some(label) = after.strip_prefix(':') else {
        return Err(MermaidError::Syntax {
            line,
            reason: format!("expected ':' after message target in {statement:?}"),
        });
    };
    register_participant(diagram, &from, &from);
    register_participant(diagram, &to, &to);
    diagram.items.push(SequenceItem::Message(SequenceMessage {
        from,
        to,
        label: normalize_label(label.trim()),
        style,
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(source: &str) -> FlowGraph {
        match parse(source) {
            Ok(Diagram::Flowchart(graph)) => graph,
            other => panic!("expected flowchart, got {other:?}"),
        }
    }

    fn sequence(source: &str) -> SequenceDiagram {
        match parse(source) {
            Ok(Diagram::Sequence(diagram)) => diagram,
            other => panic!("expected sequence, got {other:?}"),
        }
    }

    #[test]
    fn parses_flowchart_with_shapes_and_labels() {
        let graph = flow("graph TD\nA[Start] --> B{Ok?}\nB -->|yes| C(End)\nB -->|no| D((Stop))");
        assert_eq!(graph.direction, Direction::TopDown);
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.nodes["A"].shape, NodeShape::Rect);
        assert_eq!(graph.nodes["A"].label, "Start");
        assert_eq!(graph.nodes["B"].shape, NodeShape::Diamond);
        assert_eq!(graph.nodes["C"].shape, NodeShape::Rounded);
        assert_eq!(graph.nodes["D"].shape, NodeShape::Circle);
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.edges[1].label.as_deref(), Some("yes"));
        assert!(graph.edges[1].arrow);
    }

    #[test]
    fn bare_ids_get_identity_labels_and_lr_direction() {
        let graph = flow("flowchart LR\nalpha --> beta");
        assert_eq!(graph.direction, Direction::LeftRight);
        assert_eq!(graph.nodes["alpha"].label, "alpha");
        assert_eq!(graph.nodes["beta"].label, "beta");
    }

    #[test]
    fn parses_chains_semicolons_and_comments() {
        let graph = flow("graph TD; a --> b --> c %% comment\n%% whole line\nc --- d");
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[1].to, "c");
        assert!(!graph.edges[2].arrow);
        assert_eq!(graph.nodes.len(), 4);
    }

    #[test]
    fn parses_text_edge_form() {
        let graph = flow("graph TD\na -- goes to --> b");
        assert_eq!(graph.edges[0].label.as_deref(), Some("goes to"));
        assert!(graph.edges[0].arrow);
    }

    #[test]
    fn explicit_label_survives_later_bare_mention() {
        let graph = flow("graph TD\nA[Начало] --> B\nB --> A");
        assert_eq!(graph.nodes["A"].label, "Начало");
    }

    #[test]
    fn rejects_unsupported_diagram_types() {
        assert!(matches!(
            parse("gantt\ntitle Plan"),
            Err(MermaidError::UnsupportedDiagramType { ref found }) if found == "gantt"
        ));
        assert!(matches!(
            parse("classDiagram\nA <|-- B"),
            Err(MermaidError::UnsupportedDiagramType { .. })
        ));
    }

    #[test]
    fn identifiers_with_keyword_prefixes_are_accepted() {
        let graph = flow("graph TD\nending --> done\nstyleGuide --> classroom\nclassy --> end2");
        assert_eq!(graph.nodes.len(), 6);
        assert!(graph.nodes.contains_key("ending"));
        assert!(graph.nodes.contains_key("styleGuide"));
        assert!(graph.nodes.contains_key("classroom"));
    }

    #[test]
    fn rejects_unsupported_flowchart_constructs() {
        for source in [
            "graph BT\na --> b",
            "graph TD\na --> b\nstyle a fill:#f9f",
            "graph TD\na -.-> b",
            "graph TD\na ==> b",
            "graph TD\na --> b & c",
            "graph TD\na ----> b",
            "graph TD\na[[sub]] --> b",
        ] {
            assert!(
                matches!(
                    parse(source),
                    Err(MermaidError::UnsupportedFeature { .. })
                        | Err(MermaidError::UnsupportedDiagramType { .. })
                ),
                "source {source:?} must be rejected as unsupported"
            );
        }
    }

    #[test]
    fn parses_subgraph_with_members_and_edge_targets() {
        let graph =
            flow("graph LR\nA --> Server\nsubgraph Server[\"Srv\"]\nB[Box] --> C\nend\nB --> D");
        assert_eq!(graph.subgraphs.len(), 1);
        assert_eq!(graph.subgraphs[0].id, "Server");
        assert_eq!(graph.subgraphs[0].label, "Srv");
        assert!(graph.subgraphs[0].children.contains(&"B".to_owned()));
        assert!(graph.subgraphs[0].children.contains(&"C".to_owned()));
        assert!(!graph.nodes.contains_key("Server"));
        assert!(graph.nodes.contains_key("A"));
        assert!(graph.nodes.contains_key("B"));
        assert_eq!(graph.edges[0].to, "Server");
    }

    #[test]
    fn parses_nested_subgraphs() {
        let graph = flow(
            "graph TB\nsubgraph Process[\"P\"]\nHTTP\nsubgraph Domain[\"D\"]\nAUTH\nend\nend\nHTTP --> Domain",
        );
        assert_eq!(graph.subgraphs.len(), 2);
        let process = graph.subgraphs.iter().find(|s| s.id == "Process").unwrap();
        let domain = graph.subgraphs.iter().find(|s| s.id == "Domain").unwrap();
        assert!(process.children.contains(&"HTTP".to_owned()));
        assert!(process.children.contains(&"Domain".to_owned()));
        assert!(domain.children.contains(&"AUTH".to_owned()));
        assert_eq!(graph.edges[0].to, "Domain");
    }

    #[test]
    fn parses_cylinder_asymmetric_quoted_and_br() {
        let graph =
            flow("graph TD\nDB[(PostgreSQL)] --> F[/files/]\nN[\"line1<br/>line2\"] --> DB");
        assert_eq!(graph.nodes["DB"].shape, NodeShape::Cylinder);
        assert_eq!(graph.nodes["DB"].label, "PostgreSQL");
        assert_eq!(graph.nodes["F"].shape, NodeShape::Asymmetric);
        assert_eq!(graph.nodes["F"].label, "files");
        assert_eq!(graph.nodes["N"].label, "line1\nline2");
    }

    #[test]
    fn rejects_syntax_errors() {
        assert!(matches!(
            parse("graph TD\na[unclosed --> b"),
            Err(MermaidError::Syntax { .. })
        ));
        assert!(matches!(
            parse("graph TD\na -- --> b"),
            Err(MermaidError::Syntax { .. })
        ));
        assert!(matches!(
            parse("graph TD\nsubgraph x\na --> b"),
            Err(MermaidError::Syntax { .. })
        ));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(matches!(parse(""), Err(MermaidError::Empty)));
        assert!(matches!(parse("graph TD"), Err(MermaidError::Empty)));
        assert!(matches!(
            parse("%% only a comment"),
            Err(MermaidError::Empty)
        ));
    }

    #[test]
    fn enforces_node_limit() {
        let edges: Vec<String> = (0..300).map(|i| format!("n{i} --> m{i}")).collect();
        let source = format!("graph TD\n{}", edges.join("\n"));
        assert!(matches!(
            parse(&source),
            Err(MermaidError::LimitExceeded { .. })
        ));
    }

    #[test]
    fn parses_sequence_with_participants_and_styles() {
        let diagram = sequence(
            "sequenceDiagram\nparticipant A as Alice\nparticipant B\nA->>B: запрос\nB-->A: ответ\nA->A: подумать",
        );
        assert_eq!(diagram.participants.len(), 2);
        assert_eq!(diagram.participants[0].label, "Alice");
        assert_eq!(diagram.participants[1].label, "B");
        assert_eq!(diagram.items.len(), 3);
        match &diagram.items[0] {
            SequenceItem::Message(m) => {
                assert_eq!(m.style, MessageStyle::SolidFilled);
            }
            other => panic!("expected message, got {other:?}"),
        }
        match &diagram.items[1] {
            SequenceItem::Message(m) => assert_eq!(m.style, MessageStyle::Dashed),
            other => panic!("expected message, got {other:?}"),
        }
        match &diagram.items[2] {
            SequenceItem::Message(m) => {
                assert_eq!(m.from, "A");
                assert_eq!(m.to, "A");
            }
            other => panic!("expected message, got {other:?}"),
        }
    }

    #[test]
    fn sequence_registers_unknown_participants_from_messages() {
        let diagram = sequence("sequenceDiagram\nClient->Server: ping");
        let ids: Vec<&str> = diagram.participants.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["Client", "Server"]);
        match &diagram.items[0] {
            SequenceItem::Message(m) => assert_eq!(m.style, MessageStyle::Solid),
            other => panic!("expected message, got {other:?}"),
        }
    }

    #[test]
    fn explicit_participant_updates_an_implicit_label_without_reordering() {
        let diagram =
            sequence("sequenceDiagram\nA->>B: first\nparticipant A as Alice\nA->>B: second");
        let participants: Vec<(&str, &str)> = diagram
            .participants
            .iter()
            .map(|participant| (participant.id.as_str(), participant.label.as_str()))
            .collect();
        assert_eq!(participants, [("A", "Alice"), ("B", "B")]);
    }

    #[test]
    fn parses_sequence_note_and_alt() {
        let diagram = sequence(
            "sequenceDiagram\nparticipant App as Mini App\nparticipant API\nApp->>API: hi\nNote over API: check\nalt ok\nAPI-->>App: 200\nelse\nAPI-->>App: 401\nend",
        );
        assert!(
            diagram
                .items
                .iter()
                .any(|i| matches!(i, SequenceItem::Note { .. }))
        );
        assert!(
            diagram
                .items
                .iter()
                .any(|i| matches!(i, SequenceItem::AltStart { .. }))
        );
        assert!(
            diagram
                .items
                .iter()
                .any(|i| matches!(i, SequenceItem::Else { .. }))
        );
        assert!(diagram.items.iter().any(|i| matches!(i, SequenceItem::End)));
    }

    #[test]
    fn rejects_unsupported_sequence_constructs() {
        for source in [
            "sequenceDiagram\nloop every\nA->B: x\nend",
            "sequenceDiagram\nA-xB: cross",
            "sequenceDiagram\nactor A",
            "sequenceDiagram\nNote left of A: hi",
        ] {
            assert!(
                matches!(parse(source), Err(MermaidError::UnsupportedFeature { .. })),
                "source {source:?} must be rejected as unsupported"
            );
        }
    }
}
