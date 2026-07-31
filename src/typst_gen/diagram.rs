//! Генерация Typst для диаграмм Mermaid (ТЗ §10.5).
//!
//! Блок кода с языком `mermaid` рендерится в SVG, SVG регистрируется как
//! ресурс в памяти, а в Typst уходит только виртуальный путь и ширина.
//! Подписи узлов и рёбер — пользовательский текст — живут исключительно
//! внутри байтов SVG и в Typst source не попадают вообще (ТЗ §10.5.3, §23).

use crate::ast::SourceSpan;
use crate::ast::block::CodeBlock;
use crate::mermaid::{self, MermaidError, RenderedDiagram};
use crate::typst_gen::error::TypstGenerationError;
use crate::typst_gen::escape::{path_literal, string_literal};
use crate::typst_gen::generator::{RenderOptions, ResourceKind, ResourceReference, ResourceSource};
use crate::typst_gen::next_logical_path;

/// Текст `alt` для диаграммы.
///
/// Фиксированная строка, а не подпись из диаграммы: детерминировано,
/// не зависит от языка документа и не даёт пользовательскому тексту
/// повода оказаться в Typst source.
const DIAGRAM_ALT: &str = "Mermaid diagram";

/// Ошибка генерации диаграммы.
#[derive(Debug)]
pub enum DiagramError {
    /// Диаграмма не отрендерилась. Вызывающий код деградирует до блока кода
    /// с предупреждением, сборка продолжается (ТЗ §10.5.5).
    Mermaid(MermaidError),
    /// Нарушен внутренний инвариант генератора — сборка обязана упасть.
    Generation(TypstGenerationError),
}

/// Выражение `mdpdf-diagram(...)` для mermaid-блока.
///
/// Регистрирует SVG как ресурс в памяти и возвращает вызов шаблонной
/// функции с виртуальным путём и уже вписанной в страницу шириной.
///
/// # Errors
///
/// [`DiagramError::Mermaid`], если диаграмма не отрендерилась;
/// [`DiagramError::Generation`], если виртуальный путь оказался
/// недопустимым (внутренний инвариант).
pub fn diagram_expression(
    code: &CodeBlock,
    span: Option<SourceSpan>,
    resources: &mut Vec<ResourceReference>,
    options: &RenderOptions,
) -> Result<String, DiagramError> {
    let rendered = mermaid::render(&code.code).map_err(DiagramError::Mermaid)?;

    let width = fit_width(
        rendered.width_px,
        rendered.height_px,
        text_width_pt(options),
        text_height_pt(options),
    );

    let logical_path = next_logical_path(resources, "mermaid-", "svg");
    let path = path_literal(&logical_path).map_err(|error| {
        DiagramError::Generation(TypstGenerationError::InvalidImagePath {
            value: logical_path.clone(),
            span,
            message: error.to_string(),
        })
    })?;

    register(rendered, logical_path, span, resources);

    Ok(format!(
        "mdpdf-diagram(path: {path}, alt: {}, width: {})",
        string_literal(DIAGRAM_ALT),
        pt(width)
    ))
}

/// Кладёт байты SVG в список ресурсов документа.
fn register(
    rendered: RenderedDiagram,
    logical_path: String,
    span: Option<SourceSpan>,
    resources: &mut Vec<ResourceReference>,
) {
    resources.push(ResourceReference {
        logical_path,
        source: ResourceSource::Embedded {
            bytes: rendered.svg,
        },
        kind: ResourceKind::Image,
        span,
    });
}

/// Ширина текстовой области страницы в пунктах (для portrait-форматов
/// ширина — меньшая сторона).
fn text_width_pt(options: &RenderOptions) -> f64 {
    let width_mm = options.paper.shorter_side_mm() - 2.0 * options.margin.as_mm();
    width_mm.max(0.0) * 72.0 / 25.4
}

/// Бюджет высоты диаграммы в пунктах: высота текстовой области минус
/// вертикальные отступы блока `mdpdf-diagram` (0.8em сверху и снизу) и небольшой
/// запас на округления внутри раскладки Typst — иначе блок, встающий впритык,
/// выталкивает за собой пустую страницу.
fn text_height_pt(options: &RenderOptions) -> f64 {
    let height_mm = options.paper.longer_side_mm() - 2.0 * options.margin.as_mm();
    (height_mm.max(0.0) * 72.0 / 25.4 - 1.6 * options.font_size.as_pt() - 4.0).max(0.0)
}

/// Ширина, с которой диаграмма ставится на страницу.
///
/// Один px SVG считается одним пунктом. Диаграмма не выходит за текстовую
/// область ни по ширине, ни по высоте и **никогда не увеличивается**:
/// маленькая схема не должна растягиваться на всю полосу набора.
fn fit_width(width_px: f64, height_px: f64, max_width: f64, max_height: f64) -> f64 {
    if !width_px.is_finite() || !height_px.is_finite() || width_px <= 0.0 {
        return 0.0;
    }

    let mut width = width_px.min(max_width);
    if height_px > 0.0 {
        // Высота масштабируется пропорционально ширине.
        let height = height_px * width / width_px;
        if height > max_height {
            width *= max_height / height;
        }
    }
    // Квантование вниз: округление в большую сторону вернуло бы переполнение.
    (width * 10000.0).floor() / 10000.0
}

/// Число в пунктах со стабильным представлением без экспоненты и хвостовых
/// нулей (по образцу `Length::Display`).
fn pt(value: f64) -> String {
    let formatted = format!("{value:.4}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}pt")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst_gen::generator::PaperSize;

    fn code(source: &str) -> CodeBlock {
        CodeBlock {
            language: Some("mermaid".to_owned()),
            code: source.to_owned(),
        }
    }

    fn options() -> RenderOptions {
        RenderOptions::default()
    }

    fn render_one(source: &str) -> (String, Vec<ResourceReference>) {
        let mut resources = Vec::new();
        let expression = diagram_expression(&code(source), None, &mut resources, &options())
            .expect("diagram renders");
        (expression, resources)
    }

    #[test]
    fn a_diagram_becomes_a_template_call_with_a_virtual_path() {
        let (expression, resources) = render_one("flowchart TD\n    A[Начало] --> B[Конец]\n");

        assert!(
            expression.starts_with("mdpdf-diagram(path: \"/mdpdf-resources/mermaid-000001.svg\""),
            "{expression}"
        );
        assert!(
            expression.contains("alt: \"Mermaid diagram\""),
            "{expression}"
        );
        assert_eq!(resources.len(), 1);
        assert!(matches!(
            resources[0].source,
            ResourceSource::Embedded { .. }
        ));
    }

    #[test]
    fn user_text_never_reaches_the_typst_source() {
        let payload = "#panic(\"pwned\")";
        let (expression, resources) =
            render_one(&format!("flowchart TD\n    A[{payload}] --> B[x]\n"));

        assert!(
            !expression.contains(payload),
            "подпись утекла в Typst source: {expression}"
        );
        // Текст обязан быть внутри SVG, а не потерян.
        let ResourceSource::Embedded { bytes } = &resources[0].source else {
            panic!("ожидались встроенные байты");
        };
        let svg = String::from_utf8_lossy(bytes);
        assert!(svg.contains("panic"), "подпись потерялась вместе с SVG");
    }

    #[test]
    fn resources_share_one_counter_in_document_order() {
        let mut resources = Vec::new();
        let options = options();
        for _ in 0..3 {
            diagram_expression(
                &code("flowchart TD\n    A --> B\n"),
                None,
                &mut resources,
                &options,
            )
            .expect("diagram renders");
        }

        let paths: Vec<&str> = resources
            .iter()
            .map(|resource| resource.logical_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec![
                "/mdpdf-resources/mermaid-000001.svg",
                "/mdpdf-resources/mermaid-000002.svg",
                "/mdpdf-resources/mermaid-000003.svg",
            ]
        );
    }

    #[test]
    fn an_unsupported_diagram_is_an_error_and_registers_nothing() {
        let mut resources = Vec::new();
        let error = diagram_expression(
            &code("totallyNotADiagram\n    A --> B\n"),
            None,
            &mut resources,
            &options(),
        )
        .expect_err("unsupported diagram");

        assert!(matches!(error, DiagramError::Mermaid(_)), "{error:?}");
        assert!(resources.is_empty(), "остался висячий виртуальный путь");
    }

    #[test]
    fn a_small_diagram_is_not_upscaled() {
        let max_width = text_width_pt(&options());
        assert_eq!(fit_width(100.0, 50.0, max_width, 600.0), 100.0);
    }

    #[test]
    fn a_wide_diagram_is_scaled_down_to_the_text_width() {
        let width = fit_width(4000.0, 100.0, 400.0, 600.0);
        assert!(width <= 400.0, "{width}");
        assert!((width - 400.0).abs() < 0.01, "{width}");
    }

    #[test]
    fn a_tall_diagram_is_scaled_down_to_the_height_budget() {
        // 200x2000 при бюджете высоты 500 → ширина обязана сжаться до 50.
        let width = fit_width(200.0, 2000.0, 400.0, 500.0);
        assert!(width <= 50.0, "{width}");
        assert!(width > 49.9, "{width}");
    }

    #[test]
    fn degenerate_dimensions_do_not_produce_a_non_finite_width() {
        assert_eq!(fit_width(0.0, 0.0, 400.0, 500.0), 0.0);
        assert_eq!(fit_width(f64::NAN, 10.0, 400.0, 500.0), 0.0);
        assert_eq!(fit_width(f64::INFINITY, 10.0, 400.0, 500.0), 0.0);
    }

    #[test]
    fn the_page_budget_follows_paper_size() {
        // Letter шире A4 по короткой стороне (215.9 мм против 210 мм).
        let letter = RenderOptions {
            paper: PaperSize::Letter,
            ..RenderOptions::default()
        };
        assert!(text_width_pt(&letter) > text_width_pt(&RenderOptions::default()));
    }

    #[test]
    fn lengths_are_formatted_without_trailing_zeros() {
        assert_eq!(pt(12.0), "12pt");
        assert_eq!(pt(12.5), "12.5pt");
        assert_eq!(pt(0.0), "0pt");
    }
}
