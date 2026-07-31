//! Детерминированная раскладка диаграмм (ТЗ §10.5).
//!
//! Результат — боксы и линии в пунктах (pt), готовые к отрисовке шаблоном.
//! Метрики шрифтов не используются: ширина подписи оценивается по числу
//! символов, переполнение исключается переносом текста в шаблоне.
//! Детерминизм: упорядоченные контейнеры, фиксированное число проходов,
//! квантование координат до 0.01 pt.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::mermaid::model::{
    Diagram, Direction, FlowGraph, MessageStyle, NodeShape, SequenceDiagram, SequenceItem,
};

/// Горизонтальный отступ внутри бокса.
const PAD_X: f64 = 8.0;
/// Вертикальный отступ внутри бокса.
const PAD_Y: f64 = 5.0;
/// Максимальная ширина текста подписи до переноса.
const MAX_TEXT_W: f64 = 180.0;
/// Высота строки как множитель кегля.
const LINE_H_FACTOR: f64 = 1.3;
/// Горизонтальный зазор между узлами одного слоя.
const HGAP: f64 = 24.0;
/// Вертикальный зазор между слоями.
const VGAP: f64 = 30.0;
/// Проходов barycenter в каждую сторону.
const ORDER_SWEEPS: usize = 2;

/// Размещённая диаграмма: боксы, линии и подписи рёбер в пунктах.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedDiagram {
    /// Ширина.
    pub width: f64,
    /// Высота.
    pub height: f64,
    /// Боксы (узлы, участники).
    pub boxes: Vec<PlacedBox>,
    /// Линии (рёбра, сообщения, линии жизни).
    pub lines: Vec<PlacedLine>,
    /// Подписи рёбер (прямоугольники посчитаны раскладкой).
    pub labels: Vec<PlacedLabel>,
}

/// Размещённый бокс.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedBox {
    /// Левый край, pt.
    pub x: f64,
    /// Верхний край, pt.
    pub y: f64,
    /// Ширина, pt.
    pub w: f64,
    /// Высота, pt.
    pub h: f64,
    /// Подпись.
    pub label: String,
    /// Форма.
    pub shape: PlacedShape,
}

/// Форма бокса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedShape {
    /// Прямоугольник.
    Rect,
    /// Скруглённый прямоугольник.
    Rounded,
    /// Ромб.
    Diamond,
    /// Круг.
    Circle,
    /// Цилиндр (хранилище / БД).
    Cylinder,
    /// Параллелограмм.
    Asymmetric,
    /// Рамка подграфа или alt-фрагмента.
    Group,
}

/// Размещённая линия.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLine {
    /// Начало, pt.
    pub x1: f64,
    /// Начало, pt.
    pub y1: f64,
    /// Конец, pt.
    pub x2: f64,
    /// Конец, pt.
    pub y2: f64,
    /// Стиль.
    pub style: LineStyle,
}

/// Размещённая подпись ребра: шаблон рисует `box(width: w)` в точке (x, y),
/// не измеряя текст сам. Белая подложка выступает за (x, y, w, h) на outset
/// (CHIP_OUTSET), границы диаграммы учитывают это.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLabel {
    /// Левый край текстовой области, pt.
    pub x: f64,
    /// Верхний край текстовой области, pt.
    pub y: f64,
    /// Ширина текстовой области (текст переносится по ней), pt.
    pub w: f64,
    /// Оценка высоты текста, pt.
    pub h: f64,
    /// Текст (длинные слова уже разбиты нулевыми пробелами).
    pub text: String,
}

/// Точка привязки подписи (середина ребра) и исходный текст —
/// промежуточное представление до расчёта прямоугольника.
struct LabelSpot {
    x: f64,
    y: f64,
    text: String,
}

/// Стиль линии.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    /// Сплошная без стрелки.
    Plain,
    /// Сплошная с открытой стрелкой.
    Arrow,
    /// Сплошная с закрашенной стрелкой.
    FilledArrow,
    /// Пунктирная без стрелки (линии жизни).
    DashedPlain,
    /// Пунктирная с открытой стрелкой.
    DashedArrow,
    /// Пунктирная с закрашенной стрелкой.
    DashedFilledArrow,
}

/// Раскладывает диаграмму. `font_size` — в пунктах.
///
/// Координаты естественного размера; вписывание в страницу делает генератор
/// Typst единым коэффициентом `fit`, который сжимает и геометрию, и текст.
/// Границы диаграммы включают подписи рёбер и выступающие сегменты линий —
/// иначе длинная подпись или self-loop вылезали бы за пределы страницы.
/// Результат детерминирован: при одинаковом входе — одинаковые числа.
#[must_use]
pub fn layout(diagram: &Diagram, font_size: f64) -> PlacedDiagram {
    let (mut placed, spots) = match diagram {
        Diagram::Flowchart(graph) => layout_flowchart(graph, font_size),
        Diagram::Sequence(sequence) => layout_sequence(sequence, font_size),
    };
    placed.labels = spots
        .into_iter()
        .map(|spot| place_label(spot, font_size))
        .collect();
    include_painted_bounds(&mut placed);
    quantize(&mut placed);
    placed
}

// ---------------------------------------------------------------------------
// Общие хелперы
// ---------------------------------------------------------------------------

/// Оценка ширины подписи без метрик шрифта: широкие кодпоинты (CJK и
/// далее) считаются полноширинными, остальные — 0.6 кегля. Нулевой пробел
/// (точка разрыва от `break_long_words`) ширины не имеет.
fn estimate_width(label: &str, font_size: f64) -> f64 {
    label
        .chars()
        .map(|c| {
            if c == '\u{200B}' {
                0.0
            } else if (c as u32) >= 0x2E80 {
                font_size
            } else {
                0.6 * font_size
            }
        })
        .sum()
}

/// Размер бокса под подпись и форму. Учитывает явные `\n` и перенос по ширине.
fn box_size(label: &str, font_size: f64, shape: PlacedShape) -> (f64, f64) {
    let line_h = font_size * LINE_H_FACTOR;
    let mut max_line_w = 0.0_f64;
    let mut total_lines = 0.0_f64;
    for line in label.split('\n') {
        let estimate = estimate_width(line, font_size);
        max_line_w = max_line_w.max(estimate.min(MAX_TEXT_W));
        total_lines += (estimate / MAX_TEXT_W).ceil().max(1.0);
    }
    if total_lines == 0.0 {
        total_lines = 1.0;
    }
    let mut w = max_line_w + 2.0 * PAD_X;
    let mut h = total_lines.mul_add(line_h, 2.0 * PAD_Y);
    match shape {
        PlacedShape::Diamond => {
            w *= 1.6;
            h *= 1.5;
        }
        PlacedShape::Circle => {
            let d = w.max(h) * 1.3;
            w = d;
            h = d;
        }
        PlacedShape::Cylinder => {
            h += 10.0;
        }
        PlacedShape::Asymmetric => {
            w += 12.0;
        }
        PlacedShape::Group => {
            // Заголовок рамки чуть компактнее обычного узла.
            h = total_lines.mul_add(line_h * 0.9, 2.0 * PAD_Y);
        }
        PlacedShape::Rect | PlacedShape::Rounded => {}
    }
    (w, h)
}

/// Соответствие формы узла модели размещённой форме.
fn placed_shape(shape: NodeShape) -> PlacedShape {
    match shape {
        NodeShape::Rect => PlacedShape::Rect,
        NodeShape::Rounded => PlacedShape::Rounded,
        NodeShape::Diamond => PlacedShape::Diamond,
        NodeShape::Circle => PlacedShape::Circle,
        NodeShape::Cylinder => PlacedShape::Cylinder,
        NodeShape::Asymmetric => PlacedShape::Asymmetric,
    }
}

/// Точка пересечения луча из центра бокса к цели с границей бокса
/// (аппроксимация формы её описанным прямоугольником).
fn border_point(cx: f64, cy: f64, hw: f64, hh: f64, tx: f64, ty: f64) -> (f64, f64) {
    let dx = tx - cx;
    let dy = ty - cy;
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let sx = if dx == 0.0 {
        f64::INFINITY
    } else {
        hw / dx.abs()
    };
    let sy = if dy == 0.0 {
        f64::INFINITY
    } else {
        hh / dy.abs()
    };
    let s = sx.min(sy);
    (cx + dx * s, cy + dy * s)
}

/// Квантование до 0.01 pt: стабильный текстовый вид чисел.
fn quantize(placed: &mut PlacedDiagram) {
    fn q(v: f64) -> f64 {
        (v * 100.0).round() / 100.0
    }
    placed.width = q(placed.width);
    placed.height = q(placed.height);
    for placed_box in &mut placed.boxes {
        placed_box.x = q(placed_box.x);
        placed_box.y = q(placed_box.y);
        placed_box.w = q(placed_box.w);
        placed_box.h = q(placed_box.h);
    }
    for line in &mut placed.lines {
        line.x1 = q(line.x1);
        line.y1 = q(line.y1);
        line.x2 = q(line.x2);
        line.y2 = q(line.y2);
    }
    for label in &mut placed.labels {
        label.x = q(label.x);
        label.y = q(label.y);
        label.w = q(label.w);
        label.h = q(label.h);
    }
}

// ---------------------------------------------------------------------------
// Подписи рёбер
// ---------------------------------------------------------------------------

/// Кегль подписи ребра как доля основного (шаблон: `text(size: 0.7em * fit)`).
const CHIP_FONT_FACTOR: f64 = 0.7;
/// Белая подложка подписи (шаблон: `outset: 1.5pt * fit`).
const CHIP_OUTSET: f64 = 1.5;
/// Запас вокруг сегмента, выступившего за базовый canvas: покрывает
/// полуширину стрелки (3.5pt) и толщину обводки в шаблоне.
const LINE_OUTSET: f64 = 4.0;
/// Слова длиннее этого порога режутся нулевыми пробелами: Typst сам слова
/// не разрывает (проверено рендером), иначе длинное слово уезжает за
/// пределы подложки и страницы.
const MAX_WORD: usize = 20;
/// Кусок длинного слова между точками разрыва. Даже полноширинные символы
/// при кегле подписи укладываются в MAX_TEXT_W: 20 * 0.7 * 11 < 180.
const WORD_CHUNK: usize = 18;

/// Разбивает слова длиннее MAX_WORD нулевыми пробелами (U+200B) — точками
/// разрыва, невидимыми в PDF. Неразрывные Unicode-пробелы нормализуются в
/// обычный пробел: иначе Typst не переносит по ним строку и длинная подпись
/// переполняет ограниченный по ширине бокс. Детерминированно; оценка ширины
/// считает нулевой пробел бесплатным.
fn break_long_words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pos = 0usize; // позиция внутри текущего слова (1-based)
    for c in text.chars() {
        if matches!(c, '\u{00A0}' | '\u{2007}' | '\u{202F}') {
            pos = 0;
            out.push(' ');
        } else if c.is_whitespace() {
            pos = 0;
            out.push(c);
        } else {
            pos += 1;
            if pos > MAX_WORD && (pos - MAX_WORD) % WORD_CHUNK == 1 {
                out.push('\u{200B}');
            }
            out.push(c);
        }
    }
    out
}

/// Оценка размера подписи ребра вместе с подложкой. Перенос как у узлов:
/// ширина текста ограничена MAX_TEXT_W (шаблон рисует `box(width: w)`),
/// более длинный текст даёт несколько строк. Константы должны совпадать с
/// отрисовкой подписи в `mdpdf-diagram` (assets/template.typ).
fn chip_size(label: &str, font_size: f64) -> (f64, f64) {
    let chip_font = CHIP_FONT_FACTOR * font_size;
    let mut max_line_w = 0.0_f64;
    let mut total_lines = 0.0_f64;
    for line in label.split('\n') {
        let estimate = estimate_width(line, chip_font);
        max_line_w = max_line_w.max(estimate.min(MAX_TEXT_W));
        total_lines += (estimate / MAX_TEXT_W).ceil().max(1.0);
    }
    if total_lines == 0.0 {
        total_lines = 1.0;
    }
    (
        max_line_w + 2.0 * CHIP_OUTSET,
        total_lines.mul_add(chip_font * LINE_H_FACTOR, 2.0 * CHIP_OUTSET),
    )
}

/// Подпись → прямоугольник: центр в точке привязки (середина ребра),
/// размер по `chip_size`, текст с разбитыми длинными словами.
fn place_label(spot: LabelSpot, font_size: f64) -> PlacedLabel {
    let text = break_long_words(&spot.text);
    let (painted_w, painted_h) = chip_size(&text, font_size);
    PlacedLabel {
        x: spot.x - painted_w / 2.0 + CHIP_OUTSET,
        y: spot.y - painted_h / 2.0 + CHIP_OUTSET,
        w: painted_w - 2.0 * CHIP_OUTSET,
        h: painted_h - 2.0 * CHIP_OUTSET,
        text,
    }
}

/// Расширяет границы диаграммы под подписи рёбер и сегменты, выступающие за
/// базовый canvas (например, правая часть self-loop). Для выступающих линий
/// оставляется запас под обводку и стрелку. Координаты сдвигаются так, чтобы
/// ничего не уходило в минус.
fn include_painted_bounds(placed: &mut PlacedDiagram) {
    let mut min_x = 0.0_f64;
    let mut min_y = 0.0_f64;
    let mut max_x = placed.width;
    let mut max_y = placed.height;
    for line in &placed.lines {
        let line_min_x = line.x1.min(line.x2);
        let line_min_y = line.y1.min(line.y2);
        let line_max_x = line.x1.max(line.x2);
        let line_max_y = line.y1.max(line.y2);
        if line_min_x < 0.0 {
            min_x = min_x.min(line_min_x - LINE_OUTSET);
        }
        if line_min_y < 0.0 {
            min_y = min_y.min(line_min_y - LINE_OUTSET);
        }
        if line_max_x > placed.width {
            max_x = max_x.max(line_max_x + LINE_OUTSET);
        }
        if line_max_y > placed.height {
            max_y = max_y.max(line_max_y + LINE_OUTSET);
        }
    }
    for label in &placed.labels {
        min_x = min_x.min(label.x - CHIP_OUTSET);
        min_y = min_y.min(label.y - CHIP_OUTSET);
        max_x = max_x.max(label.x + label.w + CHIP_OUTSET);
        max_y = max_y.max(label.y + label.h + CHIP_OUTSET);
    }
    let shift_x = -min_x.min(0.0);
    let shift_y = -min_y.min(0.0);
    if shift_x > 0.0 || shift_y > 0.0 {
        for placed_box in &mut placed.boxes {
            placed_box.x += shift_x;
            placed_box.y += shift_y;
        }
        for line in &mut placed.lines {
            line.x1 += shift_x;
            line.x2 += shift_x;
            line.y1 += shift_y;
            line.y2 += shift_y;
        }
        for label in &mut placed.labels {
            label.x += shift_x;
            label.y += shift_y;
        }
    }
    placed.width = max_x + shift_x;
    placed.height = max_y + shift_y;
}

// ---------------------------------------------------------------------------
// Flowchart
// ---------------------------------------------------------------------------

fn layout_flowchart(graph: &FlowGraph, font_size: f64) -> (PlacedDiagram, Vec<LabelSpot>) {
    let lr = graph.direction == Direction::LeftRight;
    // Подписи узлов нормализуются один раз: оценка размера и отображаемый
    // текст используют одну строку (длинные слова разбиты нулевыми
    // пробелами — Typst сам слова не разрывает).
    let labels: BTreeMap<&str, String> = graph
        .nodes
        .iter()
        .map(|(id, node)| (id.as_str(), break_long_words(&node.label)))
        .collect();
    let sizes: BTreeMap<&str, (f64, f64)> = graph
        .nodes
        .iter()
        .map(|(id, node)| {
            let shape = placed_shape(node.shape);
            let label = labels.get(id.as_str()).map_or("", String::as_str);
            let size = box_size(label, font_size, shape);
            // LR раскладывается как TD над транспонированными размерами,
            // а финальная транспозиция возвращает боксам естественную
            // ориентацию: текст внутри остаётся горизонтальным.
            (id.as_str(), if lr { (size.1, size.0) } else { size })
        })
        .collect();

    let excluded = find_back_edges(graph);
    let layers = assign_layers(graph, &excluded);
    let order = order_layers(graph, &layers);
    let gaps = layer_gaps(graph, &layers, font_size, lr);
    let (mut boxes, mut width, mut height) = place_boxes(graph, &order, &sizes, &gaps, &labels);
    let mut clusters = place_subgraphs(graph, &boxes, font_size);
    // Рамки подграфов расширяют холст (включая отрицательный заголовок).
    for cluster in clusters.values() {
        width = width.max(cluster.x + cluster.w);
        height = height.max(cluster.y + cluster.h);
    }
    let mut targets = boxes.clone();
    targets.extend(clusters.clone());
    let (mut lines, mut spots) = place_edges(graph, &targets);

    if graph.direction == Direction::LeftRight {
        for placed_box in boxes.values_mut() {
            std::mem::swap(&mut placed_box.x, &mut placed_box.y);
            std::mem::swap(&mut placed_box.w, &mut placed_box.h);
        }
        for placed_box in clusters.values_mut() {
            std::mem::swap(&mut placed_box.x, &mut placed_box.y);
            std::mem::swap(&mut placed_box.w, &mut placed_box.h);
        }
        for line in &mut lines {
            std::mem::swap(&mut line.x1, &mut line.y1);
            std::mem::swap(&mut line.x2, &mut line.y2);
        }
        for spot in &mut spots {
            std::mem::swap(&mut spot.x, &mut spot.y);
        }
        std::mem::swap(&mut width, &mut height);
    }

    // Рамки подграфов — раньше узлов, чтобы узлы рисовались поверх.
    let mut all_boxes: Vec<PlacedBox> = clusters.into_values().collect();
    all_boxes.extend(boxes.into_values());

    (
        PlacedDiagram {
            width,
            height,
            boxes: all_boxes,
            lines,
            labels: Vec::new(),
        },
        spots,
    )
}

/// Отступ рамки подграфа вокруг потомков.
const SUBGRAPH_PAD: f64 = 12.0;

/// Рамки подграфов по уже размещённым листовым узлам. Вложенность
/// сходится за число проходов ≤ числа подграфов (снизу вверх).
fn place_subgraphs(
    graph: &FlowGraph,
    boxes: &BTreeMap<String, PlacedBox>,
    font_size: f64,
) -> BTreeMap<String, PlacedBox> {
    let title_h = font_size * LINE_H_FACTOR + 4.0;
    let mut clusters: BTreeMap<String, PlacedBox> = BTreeMap::new();
    if graph.subgraphs.is_empty() {
        return clusters;
    }
    for _ in 0..graph.subgraphs.len() {
        for sg in &graph.subgraphs {
            let label = break_long_words(&sg.label);
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            let mut any = false;
            for child in &sg.children {
                let rect = boxes.get(child).or_else(|| clusters.get(child));
                if let Some(b) = rect {
                    any = true;
                    min_x = min_x.min(b.x);
                    min_y = min_y.min(b.y);
                    max_x = max_x.max(b.x + b.w);
                    max_y = max_y.max(b.y + b.h);
                }
            }
            let (x, y, w, h) = if any {
                let title_w = estimate_width(&label, font_size * 0.75).min(MAX_TEXT_W);
                let x = min_x - SUBGRAPH_PAD;
                let y = min_y - SUBGRAPH_PAD - title_h;
                let w = (max_x + SUBGRAPH_PAD - x).max(title_w + 2.0 * SUBGRAPH_PAD);
                let h = max_y + SUBGRAPH_PAD - y;
                (x, y, w, h)
            } else {
                let (w, h) = box_size(&label, font_size, PlacedShape::Group);
                (0.0, 0.0, w.max(80.0), h.max(title_h + SUBGRAPH_PAD))
            };
            clusters.insert(
                sg.id.clone(),
                PlacedBox {
                    x,
                    y,
                    w,
                    h,
                    label,
                    shape: PlacedShape::Group,
                },
            );
        }
    }
    clusters
}

/// Листовые узлы, на которые «смотрит» id: сам узел или все листья
/// подграфа (включая вложенные). Нужно, чтобы `A --> Server` и циклы
/// через id подграфа участвовали в layering как обычные рёбра между
/// листьями — иначе back-edge DFS не видит цикл, а релаксация слоёв
/// раздувает номер слоя до O(nodes·edges).
fn leaf_endpoints<'a>(graph: &'a FlowGraph, id: &'a str) -> Vec<&'a str> {
    if graph.nodes.contains_key(id) {
        return vec![id];
    }
    let Some(sg) = graph.subgraphs.iter().find(|sg| sg.id == id) else {
        return Vec::new();
    };
    let mut leaves = Vec::new();
    let mut stack: Vec<&str> = sg.children.iter().map(String::as_str).collect();
    while let Some(child) = stack.pop() {
        if graph.nodes.contains_key(child) {
            leaves.push(child);
            continue;
        }
        if let Some(nested) = graph.subgraphs.iter().find(|s| s.id == child) {
            stack.extend(nested.children.iter().map(String::as_str));
        }
    }
    leaves.sort_unstable();
    leaves.dedup();
    leaves
}

/// Исходные рёбра, развёрнутые в пары листьев: `(edge_index, from, to)`.
fn expanded_leaf_edges<'a>(graph: &'a FlowGraph) -> Vec<(usize, &'a str, &'a str)> {
    let mut out = Vec::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        let froms = leaf_endpoints(graph, &edge.from);
        let tos = leaf_endpoints(graph, &edge.to);
        if froms.is_empty() || tos.is_empty() {
            continue;
        }
        for &from in &froms {
            for &to in &tos {
                out.push((index, from, to));
            }
        }
    }
    out
}

/// Рёбра, которые нужно исключить из layering: петли и обратные рёбра
/// циклов. DFS по **развёрнутым** рёбрам (листья подграфов) от истоков
/// (in-degree 0), затем от оставшихся — иначе старт с произвольного id
/// или цикл через `--> Server` ломает иерархию / раздувает слои.
fn find_back_edges(graph: &FlowGraph) -> Vec<bool> {
    let expanded = expanded_leaf_edges(graph);
    let mut excluded = vec![false; graph.edges.len()];
    let mut adjacency: BTreeMap<&str, Vec<(usize, &str)>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> =
        graph.nodes.keys().map(|id| (id.as_str(), 0)).collect();
    for &(edge_index, from, to) in &expanded {
        adjacency.entry(from).or_default().push((edge_index, to));
        *indegree.entry(to).or_insert(0) += 1;
    }
    // Истоки первыми (детерминированный порядок id), затем остальные.
    let mut starts: Vec<&str> = indegree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| *id)
        .collect();
    starts.sort_unstable();
    let mut rest: Vec<&str> = indegree
        .iter()
        .filter(|(_, deg)| **deg > 0)
        .map(|(id, _)| *id)
        .collect();
    rest.sort_unstable();
    starts.extend(rest);

    // 0 — не посещён, 1 — в стеке, 2 — обработан.
    let mut color: BTreeMap<&str, u8> = graph.nodes.keys().map(|id| (id.as_str(), 0u8)).collect();
    for start in starts {
        if color.get(start).copied().unwrap_or(0) != 0 {
            continue;
        }
        let mut stack: Vec<(&str, usize)> = vec![(start, 0)];
        color.insert(start, 1);
        while let Some((node, next_index)) = stack.last().copied() {
            let next = adjacency
                .get(node)
                .and_then(|list| list.get(next_index))
                .copied();
            match next {
                Some((edge_index, target)) => {
                    if let Some(top) = stack.last_mut() {
                        top.1 += 1;
                    }
                    match color.get(target).copied().unwrap_or(0) {
                        // Петля и ребро в узел из стека — обратные.
                        // Помечаем исходное mermaid-ребро целиком.
                        1 => excluded[edge_index] = true,
                        0 => {
                            color.insert(target, 1);
                            stack.push((target, 0));
                        }
                        _ => {}
                    }
                }
                None => {
                    color.insert(node, 2);
                    stack.pop();
                }
            }
        }
    }
    excluded
}

/// Longest-path layering по развёрнутым листовым рёбрам. Рёбра к id
/// подграфа сдвигают его членов; обратные (из `find_back_edges`)
/// пропускаются — DAG, сходимость за ≤ |V| проходов.
fn assign_layers(graph: &FlowGraph, excluded: &[bool]) -> BTreeMap<String, usize> {
    let mut layer: BTreeMap<String, usize> = graph.nodes.keys().map(|id| (id.clone(), 0)).collect();
    let expanded = expanded_leaf_edges(graph);
    for _ in 0..graph.nodes.len() {
        let mut changed = false;
        for &(edge_index, from_id, to_id) in &expanded {
            if excluded.get(edge_index).copied().unwrap_or(false) {
                continue;
            }
            let from = layer.get(from_id).copied().unwrap_or(0);
            let to = layer.get(to_id).copied().unwrap_or(0);
            if to < from + 1 {
                layer.insert(to_id.to_owned(), from + 1);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    layer
}

/// Порядок узлов внутри слоёв: сначала по появлению, затем проходы
/// barycenter вниз/вверх. Стабильная сортировка — детерминированный
/// tie-break.
fn order_layers(graph: &FlowGraph, layers: &BTreeMap<String, usize>) -> Vec<Vec<String>> {
    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut order: Vec<Vec<String>> = (0..=max_layer).map(|_| Vec::new()).collect();
    let mut by_appearance: Vec<(&String, usize)> = graph
        .nodes
        .iter()
        .map(|(id, node)| (id, node.order))
        .collect();
    by_appearance.sort_by_key(|(_, order_index)| *order_index);
    for (id, _) in by_appearance {
        let layer = layers.get(id).copied().unwrap_or(0);
        if let Some(bucket) = order.get_mut(layer) {
            bucket.push(id.clone());
        }
    }

    for _ in 0..ORDER_SWEEPS {
        // Вниз: тянем к предкам.
        for level in 1..order.len() {
            reorder_by_barycenter(graph, &mut order, level, level - 1, true);
        }
        // Вверх: тянем к потомкам.
        for level in (0..order.len().saturating_sub(1)).rev() {
            reorder_by_barycenter(graph, &mut order, level, level + 1, false);
        }
    }
    order
}

/// Переупорядочивает слой `level` по средней позиции соседей из слоя
/// `reference`. `predecessors` — считать соседями источники рёбер,
/// иначе — приёмники.
fn reorder_by_barycenter(
    graph: &FlowGraph,
    order: &mut [Vec<String>],
    level: usize,
    reference: usize,
    predecessors: bool,
) {
    let positions: BTreeMap<&str, usize> = order
        .get(reference)
        .map(|layer| {
            layer
                .iter()
                .enumerate()
                .map(|(index, id)| (id.as_str(), index))
                .collect()
        })
        .unwrap_or_default();
    let Some(current) = order.get(level) else {
        return;
    };
    let mut keys: Vec<(String, f64)> = current
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut sum = 0.0;
            let mut count = 0usize;
            for edge in &graph.edges {
                let neighbor = if predecessors {
                    (edge.to == *id).then_some(edge.from.as_str())
                } else {
                    (edge.from == *id).then_some(edge.to.as_str())
                };
                if let Some(neighbor) = neighbor
                    && let Some(position) = positions.get(neighbor)
                {
                    sum += *position as f64;
                    count += 1;
                }
            }
            // Узел без соседей в опорном слое держит текущую позицию.
            let key = if count == 0 {
                index as f64
            } else {
                sum / count as f64
            };
            (id.clone(), key)
        })
        .collect();
    keys.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    if let Some(slot) = order.get_mut(level) {
        *slot = keys.into_iter().map(|(id, _)| id).collect();
    }
}

/// Зазор после каждого слоя: базовый VGAP, но не меньше размера подписи
/// ребра между соседними слоями — иначе многострочная подпись наезжала бы
/// на узлы. Для LR критична ширина подписи: после транспозиции зазор
/// становится горизонтальным.
fn layer_gaps(
    graph: &FlowGraph,
    layers: &BTreeMap<String, usize>,
    font_size: f64,
    lr: bool,
) -> Vec<f64> {
    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut gaps = vec![VGAP; max_layer + 1];
    for edge in &graph.edges {
        let Some(label) = edge.label.as_deref() else {
            continue;
        };
        let from = layers.get(edge.from.as_str()).copied().unwrap_or(0);
        let to = layers.get(edge.to.as_str()).copied().unwrap_or(0);
        // Подпись ребра между несоседними слоями всё равно может наехать на
        // промежуточный узел — осознанное ограничение прямых линий (§10.5).
        if from + 1 != to {
            continue;
        }
        let (w, h) = chip_size(label, font_size);
        let need = if lr { w } else { h } + 8.0;
        let gap = &mut gaps[from];
        if need > *gap {
            *gap = need;
        }
    }
    gaps
}

/// Координаты боксов (TD-ориентация): слои сверху вниз, узлы пакуются
/// слева направо, узкие слои центрируются.
fn place_boxes(
    graph: &FlowGraph,
    order: &[Vec<String>],
    sizes: &BTreeMap<&str, (f64, f64)>,
    gaps: &[f64],
    labels: &BTreeMap<&str, String>,
) -> (BTreeMap<String, PlacedBox>, f64, f64) {
    let mut boxes: BTreeMap<String, PlacedBox> = BTreeMap::new();
    let mut layer_widths: Vec<f64> = Vec::with_capacity(order.len());
    let mut y = 0.0;
    for (level, layer) in order.iter().enumerate() {
        let layer_h = layer
            .iter()
            .map(|id| sizes.get(id.as_str()).copied().unwrap_or((0.0, 0.0)).1)
            .fold(0.0, f64::max);
        let mut x = 0.0;
        for id in layer {
            let (w, h) = sizes.get(id.as_str()).copied().unwrap_or((0.0, 0.0));
            let node = graph.nodes.get(id);
            boxes.insert(
                id.clone(),
                PlacedBox {
                    x,
                    y,
                    w,
                    h,
                    label: labels
                        .get(id.as_str())
                        .cloned()
                        .unwrap_or_else(|| break_long_words(id)),
                    shape: node.map_or(PlacedShape::Rect, |n| placed_shape(n.shape)),
                },
            );
            x += w + HGAP;
        }
        layer_widths.push(if layer.is_empty() { 0.0 } else { x - HGAP });
        y += layer_h + gaps.get(level).copied().unwrap_or(VGAP);
    }
    let width = layer_widths.iter().copied().fold(0.0, f64::max);
    let height = if order.is_empty() {
        0.0
    } else {
        y - gaps.get(order.len() - 1).copied().unwrap_or(VGAP)
    };
    for (level, layer) in order.iter().enumerate() {
        let offset = (width - layer_widths.get(level).copied().unwrap_or(0.0)) / 2.0;
        for id in layer {
            if let Some(placed_box) = boxes.get_mut(id) {
                placed_box.x += offset;
            }
        }
    }
    (boxes, width, height)
}

/// Рёбра как прямые линии между границами боксов (ТЗ §10.5: без обхода
/// узлов). Петля рисуется тремя сегментами справа от узла. Подписи
/// возвращаются как точки привязки (середины сегментов).
fn place_edges(
    graph: &FlowGraph,
    boxes: &BTreeMap<String, PlacedBox>,
) -> (Vec<PlacedLine>, Vec<LabelSpot>) {
    let mut lines = Vec::with_capacity(graph.edges.len());
    let mut spots = Vec::new();
    for edge in &graph.edges {
        let style = if edge.arrow {
            LineStyle::Arrow
        } else {
            LineStyle::Plain
        };
        let Some(from) = boxes.get(&edge.from) else {
            continue;
        };
        let Some(to) = boxes.get(&edge.to) else {
            continue;
        };
        if edge.from == edge.to {
            let x = from.x + from.w;
            let y_top = from.y + from.h * 0.35;
            let y_bottom = from.y + from.h * 0.65;
            let arm = 20.0;
            lines.push(PlacedLine {
                x1: x,
                y1: y_top,
                x2: x + arm,
                y2: y_top,
                style: LineStyle::Plain,
            });
            lines.push(PlacedLine {
                x1: x + arm,
                y1: y_top,
                x2: x + arm,
                y2: y_bottom,
                style: LineStyle::Plain,
            });
            lines.push(PlacedLine {
                x1: x + arm,
                y1: y_bottom,
                x2: x,
                y2: y_bottom,
                style,
            });
            if let Some(label) = edge.label.clone() {
                spots.push(LabelSpot {
                    x: x + arm / 2.0,
                    y: y_top,
                    text: label,
                });
            }
            continue;
        }
        let from_center = (from.x + from.w / 2.0, from.y + from.h / 2.0);
        let to_center = (to.x + to.w / 2.0, to.y + to.h / 2.0);
        let (x1, y1) = border_point(
            from_center.0,
            from_center.1,
            from.w / 2.0,
            from.h / 2.0,
            to_center.0,
            to_center.1,
        );
        let (x2, y2) = border_point(
            to_center.0,
            to_center.1,
            to.w / 2.0,
            to.h / 2.0,
            from_center.0,
            from_center.1,
        );
        if let Some(label) = edge.label.clone() {
            spots.push(LabelSpot {
                x: (x1 + x2) / 2.0,
                y: (y1 + y2) / 2.0,
                text: label,
            });
        }
        lines.push(PlacedLine {
            x1,
            y1,
            x2,
            y2,
            style,
        });
    }
    (lines, spots)
}

// ---------------------------------------------------------------------------
// Sequence
// ---------------------------------------------------------------------------

fn layout_sequence(diagram: &SequenceDiagram, font_size: f64) -> (PlacedDiagram, Vec<LabelSpot>) {
    const COL_GAP: f64 = 40.0;
    const SELF_ARM: f64 = 26.0;
    const SELF_DROP: f64 = 12.0;
    const FRAG_PAD: f64 = 8.0;
    const BAR_H_FACTOR: f64 = 1.1;

    let line_h = font_size * LINE_H_FACTOR;
    let row_h = line_h + 18.0;
    let bar_h = font_size * BAR_H_FACTOR + 6.0;

    // Заголовки участников. Подписи нормализуются один раз: оценка размера
    // и отображаемый текст используют одну строку.
    let labels: Vec<String> = diagram
        .participants
        .iter()
        .map(|p| break_long_words(&p.label))
        .collect();
    let sizes: Vec<(f64, f64)> = labels
        .iter()
        .map(|label| box_size(label, font_size, PlacedShape::Rect))
        .collect();
    let header_h = sizes.iter().map(|(_, h)| *h).fold(0.0, f64::max);

    let mut centers: BTreeMap<&str, f64> = BTreeMap::new();
    let mut boxes = Vec::with_capacity(diagram.participants.len() + 4);
    let mut cursor = 0.0;
    for ((participant, label), (w, h)) in diagram.participants.iter().zip(&labels).zip(&sizes) {
        let center = cursor + w / 2.0;
        centers.insert(participant.id.as_str(), center);
        boxes.push(PlacedBox {
            x: cursor,
            y: 0.0,
            w: *w,
            h: *h,
            label: label.clone(),
            shape: PlacedShape::Rect,
        });
        cursor += w + COL_GAP;
    }
    let width = if sizes.is_empty() {
        0.0
    } else {
        cursor - COL_GAP
    };

    // Стек открытых alt-фрагментов: y начала, подпись alt, y/подпись else.
    struct OpenAlt {
        y0: f64,
        label: String,
        else_y: Option<f64>,
        else_label: String,
    }
    let mut open_alts: Vec<OpenAlt> = Vec::new();
    let mut fragment_boxes: Vec<PlacedBox> = Vec::new();

    // События сверху вниз. Высота строки резервируется под многострочную
    // подпись — иначе она наезжала бы на соседние сообщения.
    let mut lines = Vec::with_capacity(diagram.items.len() + diagram.participants.len());
    let mut spots = Vec::with_capacity(diagram.items.len());
    let mut y = header_h + 10.0;
    for item in &diagram.items {
        match item {
            SequenceItem::Message(message) => {
                let is_self = message.from == message.to;
                let (_, chip_h) = chip_size(&message.label, font_size);
                let base_row = row_h.max(chip_h + 8.0);
                let current_row = if is_self {
                    base_row + SELF_DROP
                } else {
                    base_row
                };
                let y_mid = y + current_row / 2.0;
                let from = centers.get(message.from.as_str()).copied().unwrap_or(0.0);
                let to = centers.get(message.to.as_str()).copied().unwrap_or(0.0);
                let (arrow_style, plain_style) = message_styles(message.style);
                if is_self {
                    lines.push(PlacedLine {
                        x1: from,
                        y1: y_mid,
                        x2: from + SELF_ARM,
                        y2: y_mid,
                        style: plain_style,
                    });
                    lines.push(PlacedLine {
                        x1: from + SELF_ARM,
                        y1: y_mid,
                        x2: from + SELF_ARM,
                        y2: y_mid + SELF_DROP,
                        style: plain_style,
                    });
                    lines.push(PlacedLine {
                        x1: from + SELF_ARM,
                        y1: y_mid + SELF_DROP,
                        x2: from,
                        y2: y_mid + SELF_DROP,
                        style: arrow_style,
                    });
                    if !message.label.is_empty() {
                        spots.push(LabelSpot {
                            x: from + SELF_ARM / 2.0,
                            y: y_mid,
                            text: message.label.clone(),
                        });
                    }
                } else {
                    lines.push(PlacedLine {
                        x1: from,
                        y1: y_mid,
                        x2: to,
                        y2: y_mid,
                        style: arrow_style,
                    });
                    if !message.label.is_empty() {
                        spots.push(LabelSpot {
                            x: (from + to) / 2.0,
                            y: y_mid,
                            text: message.label.clone(),
                        });
                    }
                }
                y += current_row;
            }
            SequenceItem::Note { over, text } => {
                let note_text = break_long_words(text);
                let (note_w, note_h) = box_size(&note_text, font_size, PlacedShape::Rounded);
                let xs: Vec<f64> = over
                    .iter()
                    .filter_map(|id| centers.get(id.as_str()).copied())
                    .collect();
                let (x0, x1) = if xs.is_empty() {
                    (0.0, width.max(note_w))
                } else {
                    let min_c = xs.iter().copied().fold(f64::INFINITY, f64::min);
                    let max_c = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    let left = (min_c - note_w / 2.0).max(0.0);
                    let right = (max_c + note_w / 2.0).max(left + note_w);
                    (left, right)
                };
                let box_w = (x1 - x0).max(note_w);
                let row = note_h + 10.0;
                boxes.push(PlacedBox {
                    x: x0,
                    y: y + 4.0,
                    w: box_w,
                    h: note_h,
                    label: note_text,
                    shape: PlacedShape::Rounded,
                });
                y += row;
            }
            SequenceItem::AltStart { label } => {
                open_alts.push(OpenAlt {
                    y0: y,
                    label: if label.is_empty() {
                        "alt".to_owned()
                    } else {
                        format!("alt {label}")
                    },
                    else_y: None,
                    else_label: String::new(),
                });
                y += bar_h;
            }
            SequenceItem::Else { label } => {
                if let Some(frame) = open_alts.last_mut() {
                    frame.else_y = Some(y);
                    frame.else_label = if label.is_empty() {
                        "else".to_owned()
                    } else {
                        format!("else {label}")
                    };
                }
                y += bar_h;
            }
            SequenceItem::End => {
                if let Some(frame) = open_alts.pop() {
                    let y1 = y + FRAG_PAD;
                    let frag_label = break_long_words(&frame.label);
                    fragment_boxes.push(PlacedBox {
                        x: 0.0,
                        y: frame.y0,
                        w: width.max(40.0),
                        h: (y1 - frame.y0).max(bar_h),
                        label: frag_label,
                        shape: PlacedShape::Group,
                    });
                    if let Some(else_y) = frame.else_y {
                        // Разделитель веток alt/else.
                        lines.push(PlacedLine {
                            x1: FRAG_PAD,
                            y1: else_y,
                            x2: width.max(40.0) - FRAG_PAD,
                            y2: else_y,
                            style: LineStyle::DashedPlain,
                        });
                        if !frame.else_label.is_empty() {
                            spots.push(LabelSpot {
                                x: FRAG_PAD + 24.0,
                                y: else_y + bar_h / 2.0,
                                text: frame.else_label,
                            });
                        }
                    }
                }
                y += FRAG_PAD;
            }
        }
    }
    let height = y + 8.0;

    // Линии жизни.
    for center in centers.values() {
        lines.push(PlacedLine {
            x1: *center,
            y1: header_h,
            x2: *center,
            y2: height,
            style: LineStyle::DashedPlain,
        });
    }

    // Рамки alt — под заголовками участников и сообщениями.
    let mut all_boxes = fragment_boxes;
    all_boxes.append(&mut boxes);

    (
        PlacedDiagram {
            width,
            height,
            boxes: all_boxes,
            lines,
            labels: Vec::new(),
        },
        spots,
    )
}

/// Стрелочный и «тихий» (без стрелки) варианты стиля сообщения.
fn message_styles(style: MessageStyle) -> (LineStyle, LineStyle) {
    match style {
        MessageStyle::Solid => (LineStyle::Arrow, LineStyle::Plain),
        MessageStyle::SolidFilled => (LineStyle::FilledArrow, LineStyle::Plain),
        MessageStyle::Dashed => (LineStyle::DashedArrow, LineStyle::DashedPlain),
        MessageStyle::DashedFilled => (LineStyle::DashedFilledArrow, LineStyle::DashedPlain),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::parser::parse;

    fn placed(source: &str) -> PlacedDiagram {
        let diagram = parse(source).unwrap_or_else(|err| panic!("parse failed: {err}"));
        layout(&diagram, 11.0)
    }

    fn box_of<'a>(placed: &'a PlacedDiagram, label: &str) -> &'a PlacedBox {
        placed
            .boxes
            .iter()
            .find(|b| b.label == label)
            .unwrap_or_else(|| panic!("box {label:?} not found"))
    }

    #[test]
    fn layout_is_deterministic() {
        let source = "graph TD\nA[Start] --> B{Ok?}\nB -->|yes| C[End]\nB -->|no| D[Stop]\nC --> E[Done]\nD --> E";
        assert_eq!(placed(source), placed(source));
    }

    #[test]
    fn chain_goes_down_in_td() {
        let placed = placed("graph TD\na --> b --> c");
        let a = box_of(&placed, "a");
        let b = box_of(&placed, "b");
        let c = box_of(&placed, "c");
        assert!(a.y < b.y && b.y < c.y, "TD layers must grow downward");
        assert_eq!(placed.lines.len(), 2);
        assert!(placed.lines.iter().all(|l| l.style == LineStyle::Arrow));
    }

    #[test]
    fn lr_grows_right_and_keeps_text_boxes_horizontal() {
        let placed = placed("graph LR\nalpha --> beta");
        let a = box_of(&placed, "alpha");
        let b = box_of(&placed, "beta");
        assert!(a.x < b.x, "LR layers must grow rightward");
        assert!(
            a.w > a.h,
            "text box must keep natural horizontal orientation in LR: {a:?}"
        );
    }

    #[test]
    fn edge_labels_are_carried_over() {
        let placed = placed("graph TD\na -->|да| b");
        assert_eq!(placed.labels.len(), 1);
        assert_eq!(placed.labels[0].text, "да");
    }

    #[test]
    fn cycles_do_not_hang_and_keep_edges() {
        let placed = placed("graph TD\na --> b\nb --> c\nc --> a");
        assert_eq!(placed.lines.len(), 3, "cycle edge is still drawn");
    }

    #[test]
    fn cycle_feedback_prefers_source_rooted_tree() {
        // Реальный паттерн: клиент → шлюз → приложение, плюс обратный
        // Bot API. Старт DFS с App (алфавит) раньше помечал Caddy→App
        // как back-edge и ставил App в слой 0 рядом с Patient.
        let placed = placed(
            "flowchart TB\n\
             Patient[Пациент] -->|x| TG[Telegram]\n\
             TG -->|HTTPS| Caddy\n\
             Staff[Персонал] -->|y| Caddy\n\
             subgraph Server[\"Сервер\"]\n\
             Caddy[Caddy] --> App[NestJS]\n\
             App --> DB[(PostgreSQL)]\n\
             end\n\
             App -->|Bot API| TG",
        );
        let patient = box_of(&placed, "Пациент");
        let tg = box_of(&placed, "Telegram");
        let caddy = box_of(&placed, "Caddy");
        let app = box_of(&placed, "NestJS");
        let db = box_of(&placed, "PostgreSQL");
        assert!(
            patient.y < tg.y && tg.y < caddy.y && caddy.y < app.y && app.y < db.y,
            "hierarchy Patient→TG→Caddy→App→DB; got y: patient={} tg={} caddy={} app={} db={}",
            patient.y,
            tg.y,
            caddy.y,
            app.y,
            db.y
        );
    }

    #[test]
    fn edge_to_subgraph_raises_member_layers() {
        let placed = placed(
            "flowchart TB\n\
             Client[Client] --> Server\n\
             subgraph Server[\"Srv\"]\n\
             API[API] --> DB[(DB)]\n\
             end",
        );
        let client = box_of(&placed, "Client");
        let api = box_of(&placed, "API");
        let db = box_of(&placed, "DB");
        assert!(
            client.y < api.y && api.y < db.y,
            "Client→subgraph must push members down; y client={} api={} db={}",
            client.y,
            api.y,
            db.y
        );
    }

    #[test]
    fn self_edge_is_drawn_as_loop() {
        let placed = placed("graph TD\na -->|x| a");
        assert_eq!(placed.lines.len(), 3);
        assert!(placed.lines[2].style == LineStyle::Arrow);
        assert_lines_stay_within_diagram_bounds(&placed);
        let loop_right = placed
            .lines
            .iter()
            .map(|line| line.x1.max(line.x2))
            .fold(0.0, f64::max);
        assert!(loop_right + LINE_OUTSET <= placed.width + 0.011);
    }

    #[test]
    fn wide_graph_keeps_natural_size_for_template_scaling() {
        // Вписывание в страницу — не раскладка, а масштаб в шаблоне
        // (scale сжимает и текст); layout отдаёт естественные размеры.
        let edges: Vec<String> = (0..20).map(|i| format!("root --> child{i}")).collect();
        let placed = placed(&format!("graph TD\n{}", edges.join("\n")));
        assert!(placed.width > 400.0, "natural width is kept: {placed:?}");
    }

    #[test]
    fn sequence_has_header_lifelines_and_rows() {
        let placed = placed(
            "sequenceDiagram\nparticipant A as Alice\nparticipant B\nA->>B: привет\nB-->A: ответ",
        );
        assert_eq!(placed.boxes.len(), 2);
        assert!(placed.boxes.iter().all(|b| b.y == 0.0));
        let lifelines = placed
            .lines
            .iter()
            .filter(|l| l.style == LineStyle::DashedPlain)
            .count();
        assert_eq!(lifelines, 2);
        let messages: Vec<_> = placed
            .lines
            .iter()
            .filter(|l| l.style != LineStyle::DashedPlain)
            .collect();
        assert_eq!(messages.len(), 2);
        assert!(messages[0].y1 < messages[1].y1, "messages go top-down");
        assert_eq!(messages[0].style, LineStyle::FilledArrow);
        assert_eq!(messages[1].style, LineStyle::DashedArrow);
    }

    #[test]
    fn sequence_self_message_uses_three_segments() {
        let placed = placed("sequenceDiagram\nA->>A: x");
        let segments = placed
            .lines
            .iter()
            .filter(|l| l.style != LineStyle::DashedPlain)
            .count();
        assert_eq!(segments, 3);
        assert_lines_stay_within_diagram_bounds(&placed);
        let loop_right = placed
            .lines
            .iter()
            .filter(|line| line.style != LineStyle::DashedPlain)
            .map(|line| line.x1.max(line.x2))
            .fold(0.0, f64::max);
        assert!(loop_right + LINE_OUTSET <= placed.width + 0.011);
    }

    #[test]
    fn coordinates_are_non_negative_and_quantized() {
        let placed = placed("graph TD\nA[Старт] --> B[Финиш]");
        for b in &placed.boxes {
            assert!(b.x >= 0.0 && b.y >= 0.0);
            assert!((b.x * 100.0).fract().abs() < 1e-9, "x must be quantized");
        }
        for l in &placed.lines {
            assert!(l.x1 >= 0.0 && l.y1 >= 0.0 && l.x2 >= 0.0 && l.y2 >= 0.0);
        }
    }

    fn assert_lines_stay_within_diagram_bounds(placed: &PlacedDiagram) {
        for line in &placed.lines {
            assert!(
                line.x1 >= -0.011 && line.x2 >= -0.011 && line.y1 >= -0.011 && line.y2 >= -0.011,
                "line starts outside diagram: {line:?}"
            );
            assert!(
                line.x1 <= placed.width + 0.011
                    && line.x2 <= placed.width + 0.011
                    && line.y1 <= placed.height + 0.011
                    && line.y2 <= placed.height + 0.011,
                "line ends outside diagram: {line:?}, diagram: {placed:?}"
            );
        }
    }

    #[test]
    fn edge_labels_stay_within_diagram_bounds() {
        let long_label = format!("graph TD\na -->|{}| b", "длинная подпись ".repeat(30));
        let cases = [
            "graph TD\na -->|короткая| b".to_owned(),
            long_label,
            "graph LR\na -->|ветка| b".to_owned(),
            "sequenceDiagram\nA->>A: подумать".to_owned(),
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: привет".to_owned(),
        ];
        for source in &cases {
            let placed = placed(source);
            for label in &placed.labels {
                assert!(
                    label.x - CHIP_OUTSET >= -0.011,
                    "label left of canvas: {source}"
                );
                assert!(
                    label.x + label.w + CHIP_OUTSET <= placed.width + 0.011,
                    "label right of canvas: {source}"
                );
                assert!(
                    label.y - CHIP_OUTSET >= -0.011,
                    "label above canvas: {source}"
                );
                assert!(
                    label.y + label.h + CHIP_OUTSET <= placed.height + 0.011,
                    "label below canvas: {source}"
                );
            }
        }
    }

    #[test]
    fn long_words_are_broken_with_zero_width_spaces() {
        let diagram = placed(&format!("graph TD\na -->|{}| b", "x".repeat(400)));
        assert_eq!(diagram.labels.len(), 1);
        let text = &diagram.labels[0].text;
        assert!(
            text.contains('\u{200B}'),
            "long word must be broken: {text:?}"
        );
        for chunk in text.split(|c: char| c == '\u{200B}' || c.is_whitespace()) {
            assert!(
                chunk.chars().count() <= MAX_WORD,
                "chunk {chunk:?} longer than {MAX_WORD}"
            );
        }
        // Короткие слова не трогаем.
        let short = placed("graph TD\na -->|да| b");
        assert_eq!(short.labels[0].text, "да");
    }

    #[test]
    fn non_breaking_spaces_are_normalized_in_every_label_kind() {
        for separator in ['\u{00A0}', '\u{2007}', '\u{202F}'] {
            let text = ["123456789012345678"; 12].join(&separator.to_string());

            let node = placed(&format!("graph TD\na[{text}]"));
            let edge = placed(&format!("graph TD\na -->|{text}| b"));
            let participant = placed(&format!(
                "sequenceDiagram\nparticipant A as {text}\nA->>A: ok"
            ));
            let message = placed(&format!("sequenceDiagram\nA->>B: {text}"));

            for normalized in [
                &node.boxes[0].label,
                &edge.labels[0].text,
                &participant.boxes[0].label,
                &message.labels[0].text,
            ] {
                assert!(
                    !normalized.contains(separator),
                    "non-breaking separator must be normalized: {normalized:?}"
                );
                assert!(
                    normalized.contains(' '),
                    "normalized text must remain visually separated: {normalized:?}"
                );
            }
        }
    }

    #[test]
    fn long_word_in_node_label_is_broken() {
        let source = format!("graph TD\na[{}] --> b", "x".repeat(500));
        let placed = placed(&source);
        let node = placed
            .boxes
            .iter()
            .find(|b| b.label.contains('x'))
            .unwrap_or_else(|| panic!("node box not found: {placed:?}"));
        assert!(
            node.label.contains('\u{200B}'),
            "long word in node label must be broken: {:?}",
            node.label
        );
        // Текст сохраняется полностью: без точек переноса — исходное слово.
        let restored: String = node.label.chars().filter(|c| *c != '\u{200B}').collect();
        assert_eq!(restored, "x".repeat(500));
        // Ширина бокса ограничена переносом по MAX_TEXT_W.
        assert!(
            node.w <= MAX_TEXT_W + 2.0 * PAD_X + 0.01,
            "box width {} exceeds cap",
            node.w
        );
    }

    #[test]
    fn long_word_in_participant_alias_is_broken() {
        let source = format!(
            "sequenceDiagram\nparticipant A as {}\nA->>A: ok",
            "x".repeat(500)
        );
        let placed = placed(&source);
        let header = &placed.boxes[0];
        assert!(
            header.label.contains('\u{200B}'),
            "long alias must be broken: {:?}",
            header.label
        );
        let restored: String = header.label.chars().filter(|c| *c != '\u{200B}').collect();
        assert_eq!(restored, "x".repeat(500));
        assert!(
            header.w <= MAX_TEXT_W + 2.0 * PAD_X + 0.01,
            "participant box width {} exceeds cap",
            header.w
        );
    }

    #[test]
    fn long_edge_label_is_capped_like_node_text() {
        let source = format!("graph TD\na -->|{}| b", "x".repeat(500));
        let placed = placed(&source);
        // Подпись переносится по MAX_TEXT_W: ширина диаграммы ограничена
        // cap'ом, а не растёт пропорционально числу символов.
        assert!(
            placed.width <= 184.0,
            "chip width is capped: {}",
            placed.width
        );
        assert!(
            placed.height > 100.0,
            "wrapped label adds height: {}",
            placed.height
        );
    }

    #[test]
    fn long_label_reserves_gap_between_layers() {
        let label = "x".repeat(300);
        let spaced = placed(&format!("graph TD\na -->|{label}| b"));
        let plain = placed("graph TD\na -->|k| b");
        let gap = |p: &PlacedDiagram| {
            let a = box_of(p, "a");
            let b = box_of(p, "b");
            b.y - (a.y + a.h)
        };
        let (_, chip_h) = chip_size(&label, 11.0);
        assert!(
            gap(&spaced) >= chip_h,
            "gap {} must fit chip height {chip_h}",
            gap(&spaced)
        );
        assert!(
            gap(&spaced) > gap(&plain) + 50.0,
            "label reserves extra gap: {} vs {}",
            gap(&spaced),
            gap(&plain)
        );
    }

    #[test]
    fn long_sequence_label_reserves_row_height() {
        let label = "x".repeat(300);
        let spaced = placed(&format!("sequenceDiagram\nA->>B: {label}\nB->>A: ok"));
        let plain = placed("sequenceDiagram\nA->>B: k\nB->>A: ok");
        let (_, chip_h) = chip_size(&label, 11.0);
        let row_h = 11.0_f64.mul_add(LINE_H_FACTOR, 18.0);
        // Рост высоты диаграммы = рост строки под многострочную подпись.
        let expected = chip_h + 8.0 - row_h;
        let extra = spaced.height - plain.height;
        assert!(
            (extra - expected).abs() < 0.05,
            "extra row height {extra} must be ~{expected}"
        );
    }

    #[test]
    fn subgraph_draws_group_frame_around_members() {
        let placed = placed("graph TD\nsubgraph S[\"Box\"]\nA[One] --> B[Two]\nend\nC --> S");
        let group = placed
            .boxes
            .iter()
            .find(|b| b.shape == PlacedShape::Group)
            .expect("subgraph frame");
        assert_eq!(group.label, "Box");
        let a = box_of(&placed, "One");
        let b = box_of(&placed, "Two");
        assert!(group.x <= a.x && group.x + group.w >= a.x + a.w);
        assert!(group.x <= b.x && group.x + group.w >= b.x + b.w);
        // Ребро к id подграфа присутствует.
        assert!(!placed.lines.is_empty());
    }

    #[test]
    fn sequence_note_and_alt_produce_boxes() {
        let placed = placed(
            "sequenceDiagram\nA->>B: hi\nNote over B: check\nalt ok\nB-->>A: 200\nelse\nB-->>A: 401\nend",
        );
        assert!(
            placed.boxes.iter().any(|b| b.shape == PlacedShape::Group),
            "alt frame expected: {placed:?}"
        );
        assert!(
            placed.boxes.iter().any(|b| b.label.contains("check")),
            "note box expected: {placed:?}"
        );
    }
}
