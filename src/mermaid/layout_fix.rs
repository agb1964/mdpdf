//! Пост-обработка раскладки `mermaid-rs-renderer` до сериализации в SVG.
//!
//! Движок 0.3.1 иногда отдаёт геометрию, которую нельзя оставить как есть:
//!
//! 1. Ребро к id subgraph (или от него) маршрутизируется «пилкой» по
//!    границе подграфа — в SVG это выглядит как лишняя скруглённая рамка
//!    на всю ширину кластера (типичная схема `A --> SubgraphId`).
//!    Концы полилинии при этом **корректны**; промежуточные точки —
//!    мусор. Лечение: оставить first→last **только** для таких рёбер.
//!    Обходные маршруты (внешний узел → цель внутри subgraph, мимо
//!    чужих узлов) схлопывать нельзя: хорда first→last режет середину.
//! 2. Подписи flowchart наезжают на узлы. Проход размещения подписей
//!    самого mmdr (`label_placement`, beam search с учётом полилиний
//!    рёбер) на чистых раскладках результат лучше не трогать: здесь
//!    переставляются только подписи, чей якорь реально конфликтует с
//!    препятствиями (bbox узлов, полоса заголовка subgraph, уже
//!    размещённые подписи). Конфликтные ищут свободное место вдоль или
//!    рядом с путём.
//!
//! Sequence-подписи здесь не трогаем: для них якоря снимаются отдельно
//! в [`mod@crate::mermaid::render`].

use std::collections::HashSet;

use mermaid_rs_renderer::{DiagramKind, Layout};

/// Исправляет известные дефекты раскладки mmdr 0.3.1 на месте.
pub(crate) fn fixup_layout(layout: &mut Layout) {
    simplify_subgraph_anchor_edges(layout);
    place_flowchart_edge_labels(layout);
}

/// Упрощает рёбра к/от **якоря subgraph id** до отрезка first→last.
///
/// mmdr для `A --> SubgraphId` строит path вида:
/// `низ(A) → влево → челноки по всей ширине subgraph → точка_на_рамке`.
/// Первая и последняя точки верны.
///
/// **Не** трогаем остальные длинные пути: обход вокруг чужого узла
/// (см. тест `side_entry_keeps_detour_around_middle_node`) first→last
/// превратил бы в хорду сквозь середину.
fn simplify_subgraph_anchor_edges(layout: &mut Layout) {
    let anchor_ids: HashSet<&str> = layout
        .nodes
        .iter()
        .filter(|(_, node)| node.hidden || node.anchor_subgraph.is_some())
        .map(|(id, _)| id.as_str())
        .collect();

    if anchor_ids.is_empty() {
        return;
    }

    for edge in &mut layout.edges {
        if edge.points.len() <= 2 {
            continue;
        }
        let touches_anchor =
            anchor_ids.contains(edge.from.as_str()) || anchor_ids.contains(edge.to.as_str());
        if !touches_anchor {
            continue;
        }
        // Дополнительно: «пилка» действительно раздувает длину. Короткий
        // ортогональный обход (4 точки) не трогаем.
        let [first, .., last] = edge.points[..] else {
            continue;
        };
        let direct = distance(first, last).max(1.0);
        let along = polyline_length(&edge.points);
        if along > direct * 2.0 || edge.points.len() > 4 {
            edge.points = vec![first, last];
        }
    }
}

/// Пересечение подписи с препятствием до этой площади считается касанием
/// pad-зоны, а не конфликтом (полное залезание в узел — сотни px²).
pub(crate) const CLEAN_OVERLAP_PX2: f32 = 80.0;

/// Штраф за пересечение подписи с чужой полилинией. Мягкий: в плотной
/// схеме полностью свободного места может не быть, но свободное
/// предпочтительнее.
const CROSSING_PENALTY: f32 = 60.0;

/// Переставляет только конфликтующие подписи flowchart.
///
/// `label_anchor` в mmdr — **центр** подписи (`LabelRect::from_center`).
///
/// Финальный проход размещения подписей самого mmdr (`label_placement`)
/// учитывает полилинии рёбер и текст узлов, поэтому на чистых раскладках
/// его якорь сохраняем: безусловная перезапись более простой эвристикой
/// могла бы ухудшить результат. Перестановка запускается, только если
/// якорь mmdr пересекается с препятствием сверх [`CLEAN_OVERLAP_PX2`].
///
/// Препятствия — только видимые узлы (+ pad), **полоса заголовка**
/// subgraph (не весь кластер: иначе любая подпись на ребре внутрь Server
/// считается «внутри препятствия» и уезжает в случайное место) и уже
/// размещённые подписи, чтобы не слипались.
fn place_flowchart_edge_labels(layout: &mut Layout) {
    if layout.kind != DiagramKind::Flowchart {
        return;
    }

    // Только видимые узлы (+ pad). Весь bbox subgraph не кладём: иначе
    // подпись на ребре внутрь кластера уезжает на внешние узлы.
    // Уже размещённые подписи добавляем по ходу — чтобы не слипались.
    const PAD: f32 = 4.0;
    let mut obstacles: Vec<(f32, f32, f32, f32)> = layout
        .nodes
        .values()
        .filter(|node| !node.hidden)
        .map(|node| {
            (
                node.x - PAD,
                node.y - PAD,
                node.width + PAD * 2.0,
                node.height + PAD * 2.0,
            )
        })
        .collect();

    // Центральная полоса заголовка subgraph (не на всю ширину) — мягкая
    // зона, куда многострочные подписи лучше не сажать.
    for sub in &layout.subgraphs {
        let band_h = 32.0_f32.min(sub.height * 0.3);
        let band_w = (sub.width * 0.45).clamp(120.0, 280.0);
        let band_x = sub.x + (sub.width - band_w) * 0.5;
        obstacles.push((band_x, sub.y, band_w, band_h));
    }

    // Индексы рёбер с подписями — размещаем по одному.
    let labeled: Vec<usize> = layout
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.label
                .as_ref()
                .is_some_and(|l| l.width > 0.0 && l.height > 0.0)
        })
        .map(|(i, _)| i)
        .collect();

    for edge_idx in labeled {
        let (points, label_w, label_h, current) = {
            let edge = &layout.edges[edge_idx];
            let Some(label) = edge.label.as_ref() else {
                continue;
            };
            (
                edge.points.clone(),
                label.width,
                label.height,
                edge.label_anchor,
            )
        };
        if points.len() < 2 {
            continue;
        }

        let half_w = label_w * 0.5;
        let half_h = label_h * 0.5;

        // Чистый якорь mmdr не трогаем, только занимаем место, чтобы
        // конфликтующие подписи не сели на него.
        if let Some((cx, cy)) = current {
            let overlap = overlap_area(cx - half_w, cy - half_h, label_w, label_h, &obstacles);
            if overlap <= CLEAN_OVERLAP_PX2 {
                obstacles.push((
                    cx - half_w - 2.0,
                    cy - half_h - 2.0,
                    label_w + 4.0,
                    label_h + 4.0,
                ));
                continue;
            }
        }

        let tall = label_h > 30.0;

        let fractions = [0.5, 0.4, 0.6, 0.3, 0.7, 0.25, 0.75];
        let side = (half_w + 12.0).max(20.0);
        let up = half_h + 10.0;
        // Для высоких подписей (2+ строки) сразу предпочитаем бок пути:
        // в узком коридоре между узлами центр часто занят заголовком subgraph.
        let offsets: &[(f32, f32)] = if tall {
            &[
                (side, 0.0),
                (-side, 0.0),
                (side * 1.4, 0.0),
                (-side * 1.4, 0.0),
                (side, -up * 0.5),
                (-side, -up * 0.5),
                (0.0, 0.0),
                (0.0, up),
                (0.0, -up),
            ]
        } else {
            &[
                (0.0, 0.0),
                (side, 0.0),
                (-side, 0.0),
                (side * 1.4, 0.0),
                (-side * 1.4, 0.0),
                (0.0, -up),
                (0.0, up),
                (side, -up),
                (-side, -up),
            ]
        };

        let mut best = (
            point_at_fraction(&points, 0.5).unwrap_or(points[0]),
            f32::INFINITY,
        );

        for &frac in &fractions {
            let Some(base) = point_at_fraction(&points, frac) else {
                continue;
            };
            for &(dx, dy) in offsets {
                let center = (base.0 + dx, base.1 + dy);
                let rect = (center.0 - half_w, center.1 - half_h, label_w, label_h);
                let total = candidate_cost(layout, edge_idx, &obstacles, rect)
                    + (dx * dx + dy * dy).sqrt()
                    + (frac - 0.5).abs() * 8.0;
                if total < best.1 {
                    best = (center, total);
                }
            }
        }

        // Якорь mmdr — тоже кандидат: в патологически плотной раскладке
        // все свободные места могут оказаться хуже него.
        if let Some(center) = current {
            let rect = (center.0 - half_w, center.1 - half_h, label_w, label_h);
            let current_cost = candidate_cost(layout, edge_idx, &obstacles, rect);
            if current_cost < best.1 {
                best = (center, current_cost);
            }
        }

        let best = best.0;
        layout.edges[edge_idx].label_anchor = Some(best);
        // Занимаем место, чтобы следующая подпись не села сверху.
        obstacles.push((
            best.0 - half_w - 2.0,
            best.1 - half_h - 2.0,
            label_w + 4.0,
            label_h + 4.0,
        ));
    }
}

/// Стоимость кандидата: пересечение прямоугольника подписи с препятствиями
/// и с чужими полилиниями рёбер. Близость к пути и середине ребра считает
/// вызывающий код.
fn candidate_cost(
    layout: &Layout,
    edge_idx: usize,
    obstacles: &[(f32, f32, f32, f32)],
    rect: (f32, f32, f32, f32),
) -> f32 {
    let overlap = overlap_area(rect.0, rect.1, rect.2, rect.3, obstacles);
    let crossings = layout
        .edges
        .iter()
        .enumerate()
        .filter(|(idx, edge)| *idx != edge_idx && polyline_intersects_rect(&edge.points, rect))
        .count() as f32;
    overlap * 30.0 + crossings * CROSSING_PENALTY
}

/// Хотя бы один сегмент полилинии пересекает прямоугольник.
fn polyline_intersects_rect(points: &[(f32, f32)], rect: (f32, f32, f32, f32)) -> bool {
    points
        .windows(2)
        .any(|w| segment_intersects_rect(w[0], w[1], rect))
}

/// Отрезок пересекает прямоугольник (конец внутри или пересечение сторон).
fn segment_intersects_rect(a: (f32, f32), b: (f32, f32), rect: (f32, f32, f32, f32)) -> bool {
    let inside = |p: (f32, f32)| {
        p.0 >= rect.0 && p.0 <= rect.0 + rect.2 && p.1 >= rect.1 && p.1 <= rect.1 + rect.3
    };
    if inside(a) || inside(b) {
        return true;
    }
    let corners = [
        (rect.0, rect.1),
        (rect.0 + rect.2, rect.1),
        (rect.0 + rect.2, rect.1 + rect.3),
        (rect.0, rect.1 + rect.3),
    ];
    (0..4).any(|i| segments_intersect(a, b, corners[i], corners[(i + 1) % 4]))
}

/// Отрезки пересекаются (без коллинеарных касаний).
fn segments_intersect(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> bool {
    let cross = |o: (f32, f32), p: (f32, f32), q: (f32, f32)| {
        (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0)
    };
    (cross(a, b, c) > 0.0) != (cross(a, b, d) > 0.0)
        && (cross(c, d, a) > 0.0) != (cross(c, d, b) > 0.0)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    (dx * dx + dy * dy).sqrt()
}

fn polyline_length(points: &[(f32, f32)]) -> f32 {
    points.windows(2).map(|w| distance(w[0], w[1])).sum()
}

/// Точка на полилинии на доле `t` ∈ [0, 1] длины.
fn point_at_fraction(points: &[(f32, f32)], t: f32) -> Option<(f32, f32)> {
    let &first = points.first()?;
    if points.len() == 1 {
        return Some(first);
    }
    let total = polyline_length(points);
    if total <= f32::EPSILON {
        return Some(first);
    }
    let mut remain = t.clamp(0.0, 1.0) * total;
    for window in points.windows(2) {
        let a = window[0];
        let b = window[1];
        let len = distance(a, b);
        if remain <= len {
            let ratio = if len > f32::EPSILON {
                (remain / len).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return Some((a.0 + (b.0 - a.0) * ratio, a.1 + (b.1 - a.1) * ratio));
        }
        remain -= len;
    }
    points.last().copied()
}

fn overlap_area(x: f32, y: f32, w: f32, h: f32, obstacles: &[(f32, f32, f32, f32)]) -> f32 {
    obstacles
        .iter()
        .filter_map(|&(ox, oy, ow, oh)| {
            let iw = (x + w).min(ox + ow) - x.max(ox);
            let ih = (y + h).min(oy + oh) - y.max(oy);
            (iw > 0.0 && ih > 0.0).then_some(iw * ih)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::render::layout_of;

    /// Широкий subgraph + рёбра к/от его id (проверка «пилки»).
    const ARCHITECTURE_DOMAIN: &str = r#"flowchart TB
    BOT["Worker"] --> Domain
    subgraph Domain["Services"]
        auth[auth]
        catalog[catalog]
        booking[booking]
        notifications[notifications]
        admin[admin]
    end
    Domain --> Infra["Infra"]
"#;

    /// Два внешних входа в узел внутри subgraph; один путь идёт через
    /// промежуточный узел, второй — обходом (нельзя схлопнуть в хорду).
    const CONTEXT: &str = r#"flowchart TB
    Mobile["Mobile client"] -->|token| Relay[Relay]
    Relay -->|TLS| Edge
    Admin["Admin UI"] -->|cookie| Edge

    subgraph Cluster["Cluster"]
        Edge["Edge proxy"] --> App["App service"]
        App --> DB[(Database)]
        App --> Files[/"files"/]
        App --> Queue["Queue"]
        App --> Cache[(Cache)]
    end
"#;

    #[test]
    fn edge_to_subgraph_id_is_simplified_to_two_points() {
        let source = "flowchart TD\n    A[Start] --> Server\n    subgraph Server[\"S\"]\n        B[API]\n    end\n";
        let mut layout = layout_of(source);
        let before = layout
            .edges
            .iter()
            .find(|e| e.from == "A" && e.to == "Server")
            .map(|e| e.points.len())
            .expect("edge A->Server");
        assert!(before > 4, "pts={before}");

        fixup_layout(&mut layout);

        let edge = layout
            .edges
            .iter()
            .find(|e| e.from == "A" && e.to == "Server")
            .expect("edge");
        assert_eq!(edge.points.len(), 2);
    }

    #[test]
    fn architecture_domain_edges_are_not_full_width_scribbles() {
        let mut layout = layout_of(ARCHITECTURE_DOMAIN);
        fixup_layout(&mut layout);
        for edge in &layout.edges {
            if edge.from == "BOT" || edge.to == "Infra" || edge.from == "Domain" {
                let min_x = edge
                    .points
                    .iter()
                    .map(|p| p.0)
                    .fold(f32::INFINITY, f32::min);
                let max_x = edge
                    .points
                    .iter()
                    .map(|p| p.0)
                    .fold(f32::NEG_INFINITY, f32::max);
                assert!(
                    max_x - min_x < 40.0,
                    "{}->{} span {:.0}",
                    edge.from,
                    edge.to,
                    max_x - min_x
                );
            }
        }
    }

    /// Обход Admin→Edge не должен схлопываться в хорду через Relay.
    #[test]
    fn side_entry_keeps_detour_around_middle_node() {
        let mut layout = layout_of(CONTEXT);
        let before = layout
            .edges
            .iter()
            .find(|e| e.from == "Admin" && e.to == "Edge")
            .map(|e| e.points.len())
            .expect("Admin->Edge");
        assert!(before > 2, "ожидали обходной маршрут mmdr, pts={before}");

        fixup_layout(&mut layout);

        let edge = layout
            .edges
            .iter()
            .find(|e| e.from == "Admin" && e.to == "Edge")
            .expect("Admin->Edge");
        // Обход сохранён (не 2 точки first→last).
        assert!(
            edge.points.len() > 2,
            "обход схлопнули в хорду: {:?}",
            edge.points
        );

        // Хорда first→last не должна резать bbox промежуточного Relay.
        let mid = layout.nodes.get("Relay").expect("Relay");
        let mid_rect = (mid.x, mid.y, mid.width, mid.height);
        let cuts_center = segment_hits_rect(edge.points[0], *edge.points.last().unwrap(), mid_rect);
        if edge.points.len() == 2 {
            assert!(
                !cuts_center,
                "хорда Admin→Edge режет Relay: {:?}",
                edge.points
            );
        }
    }

    /// Подписи рёбер не должны сидеть внутри bbox узлов.
    #[test]
    fn context_edge_labels_avoid_node_boxes() {
        let mut layout = layout_of(CONTEXT);
        fixup_layout(&mut layout);

        let nodes: Vec<(f32, f32, f32, f32)> = layout
            .nodes
            .values()
            .filter(|n| !n.hidden)
            .map(|n| (n.x, n.y, n.width, n.height))
            .collect();

        for edge in &layout.edges {
            let Some(label) = edge.label.as_ref() else {
                continue;
            };
            let Some((cx, cy)) = edge.label_anchor else {
                panic!("{}->{}: нет якоря подписи", edge.from, edge.to);
            };
            let area = overlap_area(
                cx - label.width * 0.5,
                cy - label.height * 0.5,
                label.width,
                label.height,
                &nodes,
            );
            // Допуск на касание pad-зоны: полное залезание в узел — сотни px².
            assert!(
                area < 80.0,
                "{}->{}: подпись пересекает узлы на {area:.0}px² (center={cx:.0},{cy:.0})",
                edge.from,
                edge.to
            );
        }
    }

    #[test]
    fn normal_edges_are_not_collapsed() {
        let source = "flowchart TD\n    A[Start] --> B[End]\n";
        let mut layout = layout_of(source);
        let before: Vec<usize> = layout.edges.iter().map(|e| e.points.len()).collect();
        fixup_layout(&mut layout);
        let after: Vec<usize> = layout.edges.iter().map(|e| e.points.len()).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn flowchart_edge_labels_get_an_anchor_on_the_path() {
        let source = "flowchart LR\n    A[Клиент] -->|HTTPS| B[API]\n";
        let mut layout = layout_of(source);
        fixup_layout(&mut layout);
        let edge = layout
            .edges
            .iter()
            .find(|e| e.label.is_some())
            .expect("label");
        assert!(edge.label_anchor.is_some());
    }

    #[test]
    fn point_at_fraction_ends() {
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        assert_eq!(point_at_fraction(&pts, 0.0), Some((0.0, 0.0)));
        assert_eq!(point_at_fraction(&pts, 1.0), Some((10.0, 10.0)));
        let mid = point_at_fraction(&pts, 0.5).unwrap();
        assert!((mid.0 - 10.0).abs() < 0.01 && mid.1.abs() < 0.01, "{mid:?}");
    }

    /// Грубая проверка: отрезок пересекает внутренность прямоугольника.
    fn segment_hits_rect(a: (f32, f32), b: (f32, f32), rect: (f32, f32, f32, f32)) -> bool {
        let (rx, ry, rw, rh) = rect;
        // Несколько проб вдоль отрезка.
        for i in 1..8 {
            let t = i as f32 / 8.0;
            let x = a.0 + (b.0 - a.0) * t;
            let y = a.1 + (b.1 - a.1) * t;
            if x > rx + 2.0 && x < rx + rw - 2.0 && y > ry + 2.0 && y < ry + rh - 2.0 {
                return true;
            }
        }
        false
    }
}
