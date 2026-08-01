//! Измерительный стенд качества размещения подписей flowchart.
//!
//! Сравнивает базовую линию рендерера (`compute_layout` как есть) с
//! пост-правками [`layout_fix`](crate::mermaid::layout_fix) по корпусу
//! диаграмм и печатает таблицу метрик (`cargo test -- --nocapture`).
//! Здесь же живёт антидеградационный сторож: пост-правки не должны
//! делать раскладку хуже базовой линии.
//!
//! Метрики считаются по готовому `Layout` (до сериализации в SVG):
//!
//! * `node px²` — суммарная площадь пересечения подписей рёбер с bbox
//!   видимых узлов;
//! * `label px²` — суммарная площадь попарных пересечений подписей;
//! * `path hits` — сколько чужих полилиний рёбер проходит через прямоугольник
//!   подписи;
//! * `detach px` — максимальное расстояние от центра подписи до
//!   собственной полилинии (отрыв подписи от своего ребра);
//! * `score` — штраф `flowchart_quality_metrics` рендерера (пересечения
//!   рёбер, влезания в узлы; меньше — лучше).

use mermaid_rs_renderer::Layout;
use mermaid_rs_renderer::layout::flowchart_quality_metrics;

use crate::mermaid::layout_fix::{self, CLEAN_OVERLAP_PX2};
use crate::mermaid::render::layout_of;

/// Сводные метрики качества размещения подписей одной раскладки.
#[derive(Debug, Default, Clone, Copy)]
struct Metrics {
    /// Число подписей рёбер с якорем.
    labels: usize,
    /// Площадь пересечения подписей с bbox узлов, px².
    label_node_px2: f32,
    /// Площадь попарных пересечений подписей, px².
    label_label_px2: f32,
    /// Худшее пересечение одной подписи с узлами, px².
    max_label_node_px2: f32,
    /// Худшее попарное пересечение подписей, px².
    max_label_label_px2: f32,
    /// Чужие полилинии, пересекающие прямоугольник подписи, шт.
    foreign_path_hits: usize,
    /// Максимальный отрыв подписи от собственной полилинии, px.
    max_detachment_px: f32,
    /// Худшее превышение отрыва над полуразмером подписи, px.
    max_detachment_overreach: f32,
    /// Штраф качества рендерера (меньше — лучше).
    quality_score: f32,
}

/// Прямоугольник `(x, y, width, height)`.
type Rect = (f32, f32, f32, f32);

/// Прямоугольник подписи ребра по якорю-центру; `None`, если подписи
/// или якоря нет.
fn label_rect(edge: &mermaid_rs_renderer::EdgeLayout) -> Option<Rect> {
    let label = edge.label.as_ref()?;
    if label.width <= 0.0 || label.height <= 0.0 {
        return None;
    }
    let (cx, cy) = edge.label_anchor?;
    Some((
        cx - label.width * 0.5,
        cy - label.height * 0.5,
        label.width,
        label.height,
    ))
}

/// Площадь пересечения двух прямоугольников.
fn rect_overlap(a: Rect, b: Rect) -> f32 {
    let iw = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
    let ih = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
    if iw > 0.0 && ih > 0.0 { iw * ih } else { 0.0 }
}

/// Точка внутри прямоугольника (включая границу).
fn point_in_rect(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r.0 && p.0 <= r.0 + r.2 && p.1 >= r.1 && p.1 <= r.1 + r.3
}

/// Ориентация тройки точек (знак векторного произведения).
fn orientation(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Отрезки пересекаются (включая касания).
fn segments_intersect(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> bool {
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);
    (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0)
}

/// Отрезок пересекает прямоугольник.
fn segment_hits_rect(a: (f32, f32), b: (f32, f32), r: Rect) -> bool {
    if point_in_rect(a, r) || point_in_rect(b, r) {
        return true;
    }
    let corners = [
        (r.0, r.1),
        (r.0 + r.2, r.1),
        (r.0 + r.2, r.1 + r.3),
        (r.0, r.1 + r.3),
    ];
    (0..4).any(|i| segments_intersect(a, b, corners[i], corners[(i + 1) % 4]))
}

/// Хотя бы один сегмент полилинии пересекает прямоугольник.
fn polyline_hits_rect(points: &[(f32, f32)], r: Rect) -> bool {
    points.windows(2).any(|w| segment_hits_rect(w[0], w[1], r))
}

/// Расстояние от точки до отрезка.
fn point_segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len2 = dx * dx + dy * dy;
    let t = if len2 > f32::EPSILON {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let qx = a.0 + dx * t - p.0;
    let qy = a.1 + dy * t - p.1;
    (qx * qx + qy * qy).sqrt()
}

/// Минимальное расстояние от точки до полилинии.
fn distance_to_polyline(p: (f32, f32), points: &[(f32, f32)]) -> f32 {
    if points.len() == 1 {
        let dx = points[0].0 - p.0;
        let dy = points[0].1 - p.1;
        return (dx * dx + dy * dy).sqrt();
    }
    points
        .windows(2)
        .map(|w| point_segment_distance(p, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}

/// Считает метрики по готовой раскладке flowchart.
fn collect(layout: &Layout) -> Metrics {
    let mut metrics = Metrics::default();

    let nodes: Vec<Rect> = layout
        .nodes
        .values()
        .filter(|node| !node.hidden)
        .map(|node| (node.x, node.y, node.width, node.height))
        .collect();

    let labels: Vec<(usize, Rect)> = layout
        .edges
        .iter()
        .enumerate()
        .filter_map(|(idx, edge)| label_rect(edge).map(|rect| (idx, rect)))
        .collect();
    metrics.labels = labels.len();

    for &(_, rect) in &labels {
        let node_overlap: f32 = nodes.iter().map(|&n| rect_overlap(rect, n)).sum();
        metrics.label_node_px2 += node_overlap;
        metrics.max_label_node_px2 = metrics.max_label_node_px2.max(node_overlap);
    }
    for i in 0..labels.len() {
        for &(.., other) in &labels[i + 1..] {
            let overlap = rect_overlap(labels[i].1, other);
            metrics.label_label_px2 += overlap;
            metrics.max_label_label_px2 = metrics.max_label_label_px2.max(overlap);
        }
    }
    for &(edge_idx, rect) in &labels {
        for (other_idx, edge) in layout.edges.iter().enumerate() {
            if other_idx != edge_idx && polyline_hits_rect(&edge.points, rect) {
                metrics.foreign_path_hits += 1;
            }
        }
    }
    for &(edge_idx, rect) in &labels {
        let center = (rect.0 + rect.2 * 0.5, rect.1 + rect.3 * 0.5);
        let detachment = distance_to_polyline(center, &layout.edges[edge_idx].points);
        metrics.max_detachment_px = metrics.max_detachment_px.max(detachment);
        // Подпись шире своего полуразмера от пути — уже визуальный отрыв.
        let overreach = detachment - (rect.2 * 0.5).max(rect.3 * 0.5);
        metrics.max_detachment_overreach = metrics.max_detachment_overreach.max(overreach);
    }
    if let Some(quality) = flowchart_quality_metrics(layout) {
        metrics.quality_score = quality.quality_score;
    }

    metrics
}

/// Flowchart-блоки из golden-фикстуры — канарейки реальных документов.
fn fixture_flowcharts() -> Vec<(String, String)> {
    let markdown = include_str!("../../tests/fixtures/markdown/mermaid.md");
    let mut blocks = Vec::new();
    let mut source = String::new();
    let mut inside = false;
    let mut index = 0usize;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !inside && trimmed == "```mermaid" {
            inside = true;
            source.clear();
            continue;
        }
        if inside && trimmed == "```" {
            inside = false;
            index += 1;
            let header = source.lines().next().unwrap_or("");
            if header.starts_with("flowchart") || header.starts_with("graph ") {
                blocks.push((format!("fixture#{index}"), source.clone()));
            }
            continue;
        }
        if inside {
            source.push_str(line);
            source.push('\n');
        }
    }
    blocks
}

/// Корпус стенда: фикстура + синтетические плотные случаи.
fn corpus() -> Vec<(String, String)> {
    let mut cases = fixture_flowcharts();

    // Несколько подписей в один rank-gap (fan-in).
    cases.push((
        "fan-in-labels".to_owned(),
        "flowchart TD\n\
         \x20   A[Приём] -->|первый поток заявок| Q[Очередь]\n\
         \x20   B[Импорт] -->|второй поток заявок| Q\n\
         \x20   C[Webhook] -->|третий поток заявок| Q\n\
         \x20   Q -->|пакетная обработка| W[Worker]\n"
            .to_owned(),
    ));

    // Многострочные подписи.
    cases.push((
        "multiline-labels".to_owned(),
        "flowchart TD\n\
         \x20   A[Клиент] -->|запрос на создание<br/>с валидацией| B[Сервис]\n\
         \x20   B -->|ответ с кодом<br/>и описанием ошибки| A\n"
            .to_owned(),
    ));

    // Subgraph + длинные подписи на входе и выходе.
    cases.push((
        "subgraph-long-labels".to_owned(),
        "flowchart LR\n\
         \x20   In[Вход] -->|очень длинная подпись на входе в кластер обработки| Core\n\
         \x20   subgraph Cluster[\"Кластер\"]\n\
         \x20       Core[Ядро] --> Store[(Хранилище)]\n\
         \x20   end\n\
         \x20   Core -->|очень длинная подпись на выходе из кластера обработки| Out[Выход]\n"
            .to_owned(),
    ));

    // Контекстная схема из тестов layout_fix: обходные маршруты, подписи.
    cases.push((
        "context".to_owned(),
        "flowchart TB\n\
         \x20   Mobile[\"Mobile client\"] -->|token| Relay[Relay]\n\
         \x20   Relay -->|TLS| Edge\n\
         \x20   Admin[\"Admin UI\"] -->|cookie| Edge\n\n\
         \x20   subgraph Cluster[\"Cluster\"]\n\
         \x20       Edge[\"Edge proxy\"] --> App[\"App service\"]\n\
         \x20       App --> DB[(Database)]\n\
         \x20       App --> Files[/\"files\"/]\n\
         \x20       App --> Queue[\"Queue\"]\n\
         \x20       App --> Cache[(Cache)]\n\
         \x20   end\n"
            .to_owned(),
    ));

    cases
}

/// Печатает таблицу «базовая линия → пост-правки» по корпусу.
///
/// Таблица видна с `cargo test -- --nocapture`; сводка переносится в
/// `docs/progress.md` при изменении пост-правок.
#[test]
fn probe_prints_baseline_vs_fixup() {
    println!("\ncase | labels | node px² | label px² | path hits | detach px | score");
    for (name, source) in corpus() {
        let baseline = layout_of(&source);
        let mut fixed = baseline.clone();
        layout_fix::fixup_layout(&mut fixed);
        let base = collect(&baseline);
        let fix = collect(&fixed);
        assert_eq!(base.labels, fix.labels, "{name}: подписи потерялись");
        println!(
            "{name} | {} | {:.0} → {:.0} | {:.0} → {:.0} | {} → {} | {:.0} → {:.0} | {:.0} → {:.0}",
            base.labels,
            base.label_node_px2,
            fix.label_node_px2,
            base.label_label_px2,
            fix.label_label_px2,
            base.foreign_path_hits,
            fix.foreign_path_hits,
            base.max_detachment_px,
            fix.max_detachment_px,
            base.quality_score,
            fix.quality_score,
        );
    }
}

/// Антидеградационный сторож: пост-правки не должны ухудшать размещение
/// подписей относительно базовой линии рендерера ни на одном кейсе
/// корпуса. Площади пересечений с узлами и другими подписями не растут
/// (с допуском на `f32`).
#[test]
fn fixup_is_not_worse_than_renderer_baseline() {
    for (name, source) in corpus() {
        let baseline = layout_of(&source);
        let mut fixed = baseline.clone();
        layout_fix::fixup_layout(&mut fixed);
        let base = collect(&baseline);
        let fix = collect(&fixed);
        assert!(
            fix.label_node_px2 <= base.label_node_px2 + 1.0,
            "{name}: пересечение подписей с узлами выросло {:.0} → {:.0} px²",
            base.label_node_px2,
            fix.label_node_px2,
        );
        assert!(
            fix.label_label_px2 <= base.label_label_px2 + 1.0,
            "{name}: пересечение подписей между собой выросло {:.0} → {:.0} px²",
            base.label_label_px2,
            fix.label_label_px2,
        );
    }
}

/// Раскладка корпуса после пост-правок.
fn fixed_metrics(source: &str) -> Metrics {
    let mut layout = layout_of(source);
    layout_fix::fixup_layout(&mut layout);
    collect(&layout)
}

/// Ни одна подпись не залезает в bbox узла глубже касания pad-зоны.
///
/// Проверка на уровне `Layout`, а не готового SVG: `render_svg` читает
/// `label_anchor` verbatim, поэтому геометрия раскладки полностью
/// определяет геометрию SVG (в отличие от sequence, где якорь снимается
/// и позицию считает сам рендерер — см. тест в `render.rs`).
#[test]
fn flowchart_labels_stay_clear_of_nodes() {
    for (name, source) in corpus() {
        let metrics = fixed_metrics(&source);
        assert!(
            metrics.max_label_node_px2 <= CLEAN_OVERLAP_PX2,
            "{name}: подпись залезла в узел на {:.0} px²",
            metrics.max_label_node_px2,
        );
    }
}

/// Подписи не пересекаются между собой глубже касания pad-зоны.
#[test]
fn flowchart_labels_do_not_overlap_each_other() {
    for (name, source) in corpus() {
        let metrics = fixed_metrics(&source);
        assert!(
            metrics.max_label_label_px2 <= CLEAN_OVERLAP_PX2,
            "{name}: подписи пересеклись на {:.0} px²",
            metrics.max_label_label_px2,
        );
    }
}

/// Подпись не отрывается от собственного ребра дальше своего полуразмера
/// плюс небольшой зазор.
#[test]
fn flowchart_labels_stay_near_their_own_edge() {
    /// Зазор сверх полуразмера подписи, px.
    const DETACHMENT_ALLOWANCE: f32 = 40.0;
    for (name, source) in corpus() {
        let metrics = fixed_metrics(&source);
        assert!(
            metrics.max_detachment_overreach <= DETACHMENT_ALLOWANCE,
            "{name}: подпись оторвалась от своего ребра на {:.0} px сверх полуразмера",
            metrics.max_detachment_overreach,
        );
    }
}

/// Канарейка из golden-фикстуры: очень длинная подпись ребра.
///
/// Вторая проверка сознательно падает, если mmdr когда-нибудь починит
/// дефект у себя: это сигнал пересмотреть необходимость нашей
/// пост-правки (обновление mmdr — отдельная задача, AGENTS.md).
#[test]
fn canary_long_label_is_clear_of_nodes() {
    let Some((name, source)) = corpus().into_iter().find(|(name, _)| name == "fixture#3") else {
        panic!("в фикстуре mermaid.md нет блока #3 с длинной подписью");
    };
    assert_eq!(name, "fixture#3");

    let baseline = collect(&layout_of(&source));
    assert!(
        baseline.label_node_px2 > 200.0,
        "mmdr больше не кладёт длинную подпись на узел ({:.0} px²) — \
         пересмотреть необходимость пост-правки",
        baseline.label_node_px2,
    );

    let fixed = fixed_metrics(&source);
    assert!(
        fixed.max_label_node_px2 <= CLEAN_OVERLAP_PX2,
        "канареечная подпись в узле на {:.0} px²",
        fixed.max_label_node_px2,
    );
}
