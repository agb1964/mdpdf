//! Рендеринг диаграмм Mermaid в SVG через `mermaid-rs-renderer` (ТЗ §10.5).
//!
//! Модуль ничего не знает о Typst, `pulldown-cmark` и файловой системе:
//! на входе — текст диаграммы, на выходе — байты SVG и его размер.
//! JavaScript, Chromium и внешние процессы не используются (ТЗ §2).

use mermaid_rs_renderer::{
    LayoutConfig, Theme, compute_layout, measure_svg_dimensions, parse_mermaid_strict, render_svg,
};

use crate::mermaid::error::MermaidError;
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
const MEASUREMENT_FAMILY: &str = "mdpdf-diagram-sans";

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

/// Собственно конвейер рендерера. Вызывается только под `catch_unwind`.
fn render_inner(source: &str) -> Result<RenderedDiagram, MermaidError> {
    let mut theme = Theme::modern();
    theme.font_family = MEASUREMENT_FAMILY.to_owned();
    let layout_config = LayoutConfig {
        fast_text_metrics: true,
        ..LayoutConfig::default()
    };

    let parsed = parse_mermaid_strict(source).map_err(|error| MermaidError::Render {
        message: error.to_string(),
    })?;
    let layout = compute_layout(&parsed.graph, &theme, &layout_config);
    let dimensions = measure_svg_dimensions(&layout, &layout_config, None);
    let svg = pin_font_family(render_svg(&layout, &theme, &layout_config));

    Ok(RenderedDiagram {
        svg: svg.into_bytes(),
        width_px: f64::from(dimensions.width),
        height_px: f64::from(dimensions.height),
    })
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
fn pin_font_family(svg: String) -> String {
    svg.replace(MEASUREMENT_FAMILY, RENDER_FAMILY)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOWCHART: &str = "flowchart TD\n    A[Начало] --> B[Конец]\n";

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
}
