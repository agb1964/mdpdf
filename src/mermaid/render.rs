//! Рендеринг диаграмм Mermaid в SVG через `mermaid-rs-renderer` (ТЗ §10.5).
//!
//! Модуль ничего не знает о Typst, `pulldown-cmark` и файловой системе:
//! на входе — текст диаграммы, на выходе — байты SVG и его размер.
//! JavaScript, Chromium и внешние процессы не используются (ТЗ §2).

use mermaid_rs_renderer::{
    DiagramKind, Layout, LayoutConfig, Theme, compute_layout, measure_svg_dimensions,
    parse_mermaid_strict, render_svg,
};

use crate::mermaid::error::MermaidError;
use crate::mermaid::layout_fix;
use crate::mermaid::limits;
use crate::svg;

/// Семейство шрифта, которым рендерер *измеряет* текст.
///
/// Имя намеренно не существует ни в одной системе. Тогда запрос в `fontdb`
/// заведомо не находит начертания, и рендерер переходит на встроенную
/// табличную метрику. Это даёт сразу два обязательных для `mdpdf` свойства:
///
/// * геометрия не зависит от набора системных шрифтов, то есть одинакова
///   на всех ОС (ТЗ §10.5.4, §32);
/// * рендерер не пишет кеш шрифта в домашний каталог пользователя —
///   запись выполняется только после успешного запроса.
///
/// Остаточное отклонение: сам вызов `load_system_fonts()` внутри рендерера
/// происходит один раз за процесс. Это чтение каталогов, результат которого
/// не используется; убрать его можно только через upstream-API.
pub(crate) const MEASUREMENT_FAMILY: &str = "mdpdf-diagram-sans";

/// Семейство, которым текст внутри SVG рисует уже Typst (ТЗ §34).
const RENDER_FAMILY: &str = "Noto Sans";

/// Отрендеренная диаграмма.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedDiagram {
    /// Байты SVG. Единственный носитель пользовательского текста: в Typst
    /// source подписи не попадают вообще (ТЗ §10.5.3).
    pub svg: Vec<u8>,
    /// Собственная ширина диаграммы в px.
    pub width_px: f64,
    /// Собственная высота диаграммы в px.
    pub height_px: f64,
}

/// Рендерит исходник диаграммы Mermaid в SVG.
///
/// # Errors
///
/// [`MermaidError`], если исходник или результат выходят за лимиты §15,
/// диаграмма не разобралась, в SVG нашлась внешняя ссылка либо рендерер
/// запаниковал. Во всех случаях вызывающий код деградирует до обычного
/// блока кода с предупреждением, сборка не рвётся (ТЗ §10.5.5).
pub fn render(source: &str) -> Result<RenderedDiagram, MermaidError> {
    if source.len() > limits::MAX_SOURCE_BYTES {
        return Err(MermaidError::SourceTooLarge {
            size: source.len(),
            limit: limits::MAX_SOURCE_BYTES,
        });
    }

    // Раскладка рендерера — сторонний код на `f32`; паника не должна
    // выходить наружу. `Theme` и `LayoutConfig` строятся внутри замыкания,
    // поэтому `AssertUnwindSafe` не требуется.
    //
    // Глобальный измеритель рендерера защищён мьютексом, и паника его
    // отравляет. Здесь это безвредно: после отравления измерение возвращает
    // `None`, а это ровно тот же путь, на который уводит `MEASUREMENT_FAMILY`.
    let rendered =
        std::panic::catch_unwind(|| render_inner(source)).map_err(|_| MermaidError::Panicked)??;

    if rendered.svg.len() > limits::MAX_SVG_BYTES {
        return Err(MermaidError::SvgTooLarge {
            size: rendered.svg.len(),
            limit: limits::MAX_SVG_BYTES,
        });
    }

    // Политика ресурсов одна и та же для картинок пользователя и для выхода
    // рендерера: `click A "https://…"` даёт в SVG внешний `href` (ТЗ §33.3).
    if let Some(reference) = svg::external_reference(&rendered.svg) {
        return Err(MermaidError::ExternalReference { reference });
    }

    Ok(rendered)
}

/// Тема раскладки: шрифт — служебное несуществующее семейство
/// [`MEASUREMENT_FAMILY`], чтобы метрика текста была табличной и не
/// зависела от системных шрифтов (ТЗ §10.5.4).
pub(crate) fn measurement_theme() -> Theme {
    let mut theme = Theme::modern();
    theme.font_family = MEASUREMENT_FAMILY.to_owned();
    theme
}

/// Конфиг раскладки: табличные (быстрые) метрики текста, остальное —
/// по умолчанию рендерера.
pub(crate) fn measurement_layout_config() -> LayoutConfig {
    LayoutConfig {
        fast_text_metrics: true,
        ..LayoutConfig::default()
    }
}

/// Разбор и раскладка без пост-правок — базовая линия рендерера,
/// с которой тесты сравнивают [`layout_fix`](crate::mermaid::layout_fix).
#[cfg(test)]
pub(crate) fn layout_of(source: &str) -> Layout {
    let parsed = parse_mermaid_strict(source).expect("parse");
    compute_layout(
        &parsed.graph,
        &measurement_theme(),
        &measurement_layout_config(),
    )
}

/// Собственно конвейер рендерера. Вызывается только под `catch_unwind`.
fn render_inner(source: &str) -> Result<RenderedDiagram, MermaidError> {
    let theme = measurement_theme();
    let layout_config = measurement_layout_config();

    let parsed = parse_mermaid_strict(source).map_err(|error| MermaidError::Render {
        message: error.to_string(),
    })?;
    let mut layout = compute_layout(&parsed.graph, &theme, &layout_config);
    // Пост-правки поверх mmdr 0.3.1: «пилящие» рёбра к subgraph id и
    // подписи flowchart. Sequence-якоря снимаются отдельно ниже.
    layout_fix::fixup_layout(&mut layout);
    drop_sequence_label_anchors(&mut layout);
    let dimensions = measure_svg_dimensions(&layout, &layout_config, None);
    let svg = pin_font_family(&render_svg(&layout, &theme, &layout_config));

    Ok(RenderedDiagram {
        svg: svg.into_bytes(),
        width_px: f64::from(dimensions.width),
        height_px: f64::from(dimensions.height),
    })
}

/// Снимает у сообщений sequence-диаграммы позицию подписи, вычисленную
/// оптимизатором рендерера.
///
/// `mermaid-rs-renderer` 0.3.1 подбирает `label_anchor` минимизацией штрафов
/// (`sequence_label_penalty`): за пересечение с занятой областью штраф
/// 10 000+, а за удаление от собственной стрелки — квадратичный, но заметно
/// меньший. На разреженной диаграмме результат приемлем, но на плотной
/// (самосообщения, `alt`, многострочные подписи) подписи разъезжаются:
/// в регрессионном тесте блок подписи заходит на собственную стрелку на
/// 26 px, а визуально подписи читаются как относящиеся к соседним
/// сообщениям.
///
/// Без якоря рендерер применяет собственную геометрическую формулу
/// «блок подписи стоит на небольшом зазоре над линией сообщения» — ровно то
/// поведение, которого ждут от sequence-диаграммы. Поэтому здесь якорь
/// снимается, а не пересчитывается: своей арифметики мы не добавляем.
///
/// Для остальных типов диаграмм якорь сохраняется: там формула по умолчанию
/// привязана к первой точке ребра, а не к середине, и снятие якоря сделало бы
/// хуже. Наложение подписей flowchart на узлы лечится отдельно — избирательной
/// перестановкой конфликтующих подписей в [`layout_fix`].
fn drop_sequence_label_anchors(layout: &mut Layout) {
    if layout.kind != DiagramKind::Sequence {
        return;
    }
    for edge in &mut layout.edges {
        edge.label_anchor = None;
    }
}

/// Заменяет служебное семейство на встроенный шрифт.
///
/// Измеряет текст рендерер (табличной метрикой), а рисует его Typst —
/// и только встроенным Noto Sans, потому что системные шрифты запрещены
/// (ТЗ §32, §34). Без этой замены Typst подставил бы Noto Sans и сам,
/// через цепочку семейств окружающего текста, но тогда результат зависел бы
/// от внутренностей Typst, а не от явного решения `mdpdf`.
///
/// Побочный эффект: подпись узла, буквально содержащая
/// `mdpdf-diagram-sans`, будет переписана. Цена признана приемлемой.
fn pin_font_family(svg: &str) -> String {
    svg.replace(MEASUREMENT_FAMILY, RENDER_FAMILY)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOWCHART: &str = "flowchart TD\n    A[Начало] --> B[Конец]\n";

    /// Плотная sequence-диаграмма: самосообщение, alt-блок и многострочные
    /// подписи. Именно на такой раскладке оптимизатор рендерера промахивается.
    const SEQUENCE_DENSE: &str = "sequenceDiagram\n\
         \x20   participant C as Client\n\
         \x20   participant S as Service\n\
         \x20   participant DB as Database\n\
         \x20   C->>S: POST /items\n\
         \x20   S->>S: validate rules R1, R2, R3\n\
         \x20   S->>DB: BEGIN\n\
         \x20   S->>DB: allocate resource (R4)\n\
         \x20   S->>DB: INSERT INTO items\n\
         \x20   alt constraint ok\n\
         \x20       S->>DB: INSERT INTO outbox\n\
         \x20       S->>DB: COMMIT\n\
         \x20       S-->>C: 201 { item }\n\
         \x20   else unique_violation\n\
         \x20       S->>DB: ROLLBACK\n\
         \x20       S-->>C: 409 CONFLICT\n\
         \x20   end\n";

    #[test]
    fn a_flowchart_renders_to_svg() {
        let rendered = render(FLOWCHART).expect("flowchart renders");
        assert!(rendered.svg.starts_with(b"<svg"));
        assert!(rendered.width_px > 0.0);
        assert!(rendered.height_px > 0.0);
    }

    #[test]
    fn rendering_is_deterministic() {
        let first = render(FLOWCHART).expect("first render");
        let second = render(FLOWCHART).expect("second render");
        assert_eq!(first, second);
    }

    #[test]
    fn the_measurement_family_never_reaches_the_output() {
        let rendered = render(FLOWCHART).expect("flowchart renders");
        let svg = String::from_utf8(rendered.svg).expect("svg is utf-8");
        assert!(
            !svg.contains(MEASUREMENT_FAMILY),
            "служебное семейство осталось в SVG"
        );
        assert!(
            svg.contains(RENDER_FAMILY),
            "встроенный шрифт не проставлен"
        );
    }

    #[test]
    fn labels_are_rendered_as_text_so_typst_can_shape_them() {
        let rendered = render(FLOWCHART).expect("flowchart renders");
        let svg = String::from_utf8(rendered.svg).expect("svg is utf-8");
        assert!(svg.contains("<text"), "подписи не попали в SVG");
        assert!(svg.contains("Начало"), "кириллица не попала в SVG");
    }

    #[test]
    fn a_sequence_diagram_renders() {
        let source = "sequenceDiagram\n    participant C as Клиент\n    \
             participant S as Сервер\n    C->>S: запрос\n";
        assert!(render(source).is_ok());
    }

    #[test]
    fn a_sequence_diagram_without_declarations_auto_creates_participants() {
        assert!(render("sequenceDiagram\n    Alice->>Bob: привет\n").is_ok());
    }

    /// Расхождение с mermaid.js, зафиксированное осознанно: если объявлен
    /// хотя бы один участник, ссылка на необъявленное имя считается
    /// опечаткой и диаграмма деградирует до блока кода с предупреждением.
    /// Авто-создание работает только когда не объявлено ни одного.
    #[test]
    fn a_partially_declared_sequence_diagram_is_reported_as_a_typo() {
        let source = "sequenceDiagram\n    participant C as Клиент\n    C->>S: запрос\n";
        let error = render(source).expect_err("undeclared participant");
        assert!(
            matches!(&error, MermaidError::Render { message } if message.contains("participant")),
            "{error:?}"
        );
    }

    #[test]
    fn an_oversized_source_is_rejected_before_rendering() {
        let source = "x".repeat(limits::MAX_SOURCE_BYTES + 1);
        assert_eq!(
            render(&source),
            Err(MermaidError::SourceTooLarge {
                size: limits::MAX_SOURCE_BYTES + 1,
                limit: limits::MAX_SOURCE_BYTES,
            })
        );
    }

    #[test]
    fn an_unknown_diagram_type_is_a_render_error() {
        let error = render("totallyNotADiagram\n    A --> B\n").expect_err("unknown type");
        assert!(matches!(error, MermaidError::Render { .. }), "{error:?}");
    }

    #[test]
    fn an_empty_source_is_a_render_error() {
        let error = render("").expect_err("empty source");
        assert!(matches!(error, MermaidError::Render { .. }), "{error:?}");
    }

    /// Регрессия на дефект раскладки `mermaid-rs-renderer` 0.3.1: подпись
    /// многострочного сообщения уезжала далеко вверх и читалась как подпись
    /// предыдущего сообщения (см. [`drop_sequence_label_anchors`]).
    ///
    /// Проверяется геометрия готового SVG: у каждой подписи низ её блока
    /// обязан лежать выше собственной стрелки и не дальше высоты подписи
    /// плюс небольшой зазор. До исправления расхождение доходило до 88 px
    /// при высоте подписи 58 px.
    #[test]
    fn sequence_labels_stay_next_to_their_own_arrow() {
        let source = SEQUENCE_DENSE;
        let rendered = render(source).expect("sequence renders");
        let svg = String::from_utf8(rendered.svg).expect("svg is utf-8");

        let mut checked = 0usize;
        for (edge_id, arrow_y) in message_arrows(&svg) {
            let (top, bottom) = label_extent(&svg, &edge_id).unwrap_or_else(|| {
                panic!("у {edge_id} нет подписи");
            });
            let height = bottom - top;
            assert!(
                bottom < arrow_y,
                "{edge_id}: подпись заходит на свою стрелку (низ {bottom}, стрелка {arrow_y})"
            );
            assert!(
                arrow_y - bottom <= height + 20.0,
                "{edge_id}: подпись оторвалась от своей стрелки на {:.1} px при высоте {height:.1}",
                arrow_y - bottom
            );
            checked += 1;
        }
        assert_eq!(checked, 10, "проверены не все сообщения");
    }

    /// `(id ребра, y горизонтальной стрелки)` для каждого сообщения.
    fn message_arrows(svg: &str) -> Vec<(String, f64)> {
        let mut arrows = Vec::new();
        for chunk in svg.split("id=\"edge-").skip(1) {
            let Some((index, rest)) = chunk.split_once('"') else {
                continue;
            };
            let Some(start) = rest.find("d=\"M ") else {
                continue;
            };
            let path = &rest[start + 5..];
            let Some(end) = path.find('"') else { continue };
            // `M x,y L x,y` — берём y первой точки.
            let Some(y) = path[..end]
                .split_once(',')
                .and_then(|(_, tail)| tail.split_whitespace().next())
                .and_then(|value| value.parse::<f64>().ok())
            else {
                continue;
            };
            arrows.push((format!("edge-{index}"), y));
        }
        arrows
    }

    /// Baseline первой и последней строки подписи данного ребра.
    ///
    /// Строки внутри `<text>` смещаются относительными `dy`, поэтому низ
    /// блока — это `y` плюс сумма всех сдвигов.
    fn label_extent(svg: &str, edge_id: &str) -> Option<(f64, f64)> {
        let marker = format!("<g class=\"edgeLabel\" data-edge-id=\"{edge_id}\"");
        let start = svg.find(&marker)?;
        let group = &svg[start..];
        let group = &group[..group.find("</g>")?];

        let attribute = |haystack: &str, name: &str| -> Option<f64> {
            let at = haystack.find(name)?;
            haystack[at + name.len()..]
                .split('"')
                .next()?
                .parse::<f64>()
                .ok()
        };

        let top = attribute(group, " y=\"")?;
        let shifts: f64 = group
            .match_indices(" dy=\"")
            .filter_map(|(at, _)| attribute(&group[at..], " dy=\""))
            .sum();
        Some((top, top + shifts))
    }

    #[test]
    fn a_click_directive_with_an_external_url_is_rejected() {
        let source =
            "flowchart TD\n    A[Ссылка] --> B[Конец]\n    click A \"https://example.com\"\n";
        let error = render(source).expect_err("external reference");
        assert!(
            matches!(&error, MermaidError::ExternalReference { reference } if reference.contains("example.com")),
            "{error:?}"
        );
    }

    /// Регрессия: `A --> SubgraphId` в mmdr 0.3.1 даёт path, который
    /// несколько раз пробегает по ширине рамки подграфа (визуально —
    /// «лишняя скруглённая рамка»). После fixup path — почти прямая.
    #[test]
    fn edge_to_subgraph_id_is_not_a_scribbled_frame() {
        let source = "flowchart TD\n    A[Start] --> Server\n    subgraph Server[\"Сервер\"]\n        B[API]\n    end\n";
        let rendered = render(source).expect("renders");
        let svg = String::from_utf8(rendered.svg).expect("utf-8");
        let path = edge_path_d(&svg, "edge-0").expect("edge-0 path");
        // «Пилка»: много горизонтальных сегментов на одной y с размахом
        // почти на всю ширину. После fixup — один-два отрезка.
        let segment_count = path.matches('L').count() + path.matches('Q').count();
        assert!(
            segment_count <= 3,
            "path слишком изломан (сегментов {segment_count}): {path}"
        );
        // Не должно быть челнока left↔right на одной координате y.
        assert!(
            !path.contains("L 8.000") || path.matches("L 8.000").count() <= 1,
            "осталась пилка по x: {path}"
        );
    }

    fn edge_path_d<'a>(svg: &'a str, edge_id: &str) -> Option<&'a str> {
        let marker = format!("id=\"{edge_id}\"");
        let start = svg.find(&marker)?;
        let rest = &svg[start..];
        let d_at = rest.find("d=\"")?;
        let path = &rest[d_at + 3..];
        let end = path.find('"')?;
        Some(&path[..end])
    }
}
