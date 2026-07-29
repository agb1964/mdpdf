//! Генерация Typst для диаграмм Mermaid (ТЗ §10.5).
//!
//! Блок кода с языком `mermaid` превращается в вызов `mdpdf-diagram(...)`
//! с уже размещёнными примитивами. Подписи узлов и рёбер — пользовательский
//! текст и попадают в Typst только через экранированные строковые литералы
//! (ТЗ §23); числа производятся раскладкой и форматируются стабильно.

use crate::ast::block::CodeBlock;
use crate::mermaid::layout::{LineStyle, PlacedDiagram, PlacedShape};
use crate::mermaid::{MermaidError, layout, parse};
use crate::typst_gen::blocks::tuple;
use crate::typst_gen::escape::string_literal;
use crate::typst_gen::generator::RenderOptions;

/// Выражение `mdpdf-diagram(...)` для mermaid-блока.
///
/// # Errors
///
/// [`MermaidError`], если диаграмма вне подмножества (§10.5) или содержит
/// синтаксическую ошибку; вызывающий код деградирует до обычного блока кода.
pub fn diagram_expression(
    code: &CodeBlock,
    options: &RenderOptions,
) -> Result<String, MermaidError> {
    let diagram = parse(&code.code)?;
    let font_size = options.font_size.as_pt();
    let placed = layout(&diagram, font_size);
    let fit = fit_factor(&placed, text_width_pt(options), text_height_pt(options));
    Ok(expression(&placed, fit))
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

/// Масштаб вписывания: диаграмма (с текстом) не выходит за пределы
/// текстовой области ни по ширине, ни по высоте.
fn fit_factor(placed: &PlacedDiagram, max_width: f64, max_height: f64) -> f64 {
    let mut fit = 1.0;
    if placed.width > 0.0 && placed.width > max_width {
        fit = max_width / placed.width;
    }
    if placed.height > 0.0 && placed.height * fit > max_height {
        fit = max_height / placed.height;
    }
    // Квантование вниз: округление в большую сторону вернуло бы переполнение.
    (fit * 10000.0).floor() / 10000.0
}

/// Число в пунктах со стабильным представлением без экспоненты и хвостовых
/// нулей (по образцу `Length::Display`).
fn pt(value: f64) -> String {
    let formatted = format!("{value:.4}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}pt")
}

/// Масштаб в процентах со стабильным представлением.
fn percent(fit: f64) -> String {
    let formatted = format!("{:.4}", fit * 100.0);
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}%")
}

/// Кортеж из готовых полей (обёртка над `tuple` для владеющих строк).
fn fields_tuple(fields: &[String]) -> String {
    let refs: Vec<&str> = fields.iter().map(String::as_str).collect();
    tuple(&refs)
}

fn shape_literal(shape: PlacedShape) -> String {
    let name = match shape {
        PlacedShape::Rect => "rect",
        PlacedShape::Rounded => "rounded",
        PlacedShape::Diamond => "diamond",
        PlacedShape::Circle => "circle",
    };
    string_literal(name)
}

fn style_literal(style: LineStyle) -> String {
    let name = match style {
        LineStyle::Plain => "plain",
        LineStyle::Arrow => "arrow",
        LineStyle::FilledArrow => "filled-arrow",
        LineStyle::DashedPlain => "dashed",
        LineStyle::DashedArrow => "dashed-arrow",
        LineStyle::DashedFilledArrow => "dashed-filled-arrow",
    };
    string_literal(name)
}

fn expression(placed: &PlacedDiagram, fit: f64) -> String {
    // Геометрия масштабируется здесь, чтобы шаблону оставалось сжать только
    // текст, штрихи и стрелки — так вписывание не зависит от поведения
    // scale/reflow при пагинации.
    let boxes: Vec<String> = placed
        .boxes
        .iter()
        .map(|placed_box| {
            fields_tuple(&[
                pt(placed_box.x * fit),
                pt(placed_box.y * fit),
                pt(placed_box.w * fit),
                pt(placed_box.h * fit),
                string_literal(&placed_box.label),
                shape_literal(placed_box.shape),
            ])
        })
        .collect();
    let lines: Vec<String> = placed
        .lines
        .iter()
        .map(|line| {
            fields_tuple(&[
                pt(line.x1 * fit),
                pt(line.y1 * fit),
                pt(line.x2 * fit),
                pt(line.y2 * fit),
                style_literal(line.style),
            ])
        })
        .collect();
    let labels: Vec<String> = placed
        .labels
        .iter()
        .map(|label| {
            fields_tuple(&[
                pt(label.x * fit),
                pt(label.y * fit),
                pt(label.w * fit),
                string_literal(&label.text),
            ])
        })
        .collect();
    format!(
        "mdpdf-diagram(width: {}, height: {}, fit: {}, boxes: {}, lines: {}, labels: {})",
        pt(placed.width * fit),
        pt(placed.height * fit),
        percent(fit),
        fields_tuple(&boxes),
        fields_tuple(&lines),
        fields_tuple(&labels),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_block(code: &str) -> CodeBlock {
        CodeBlock {
            language: Some("mermaid".to_owned()),
            code: code.to_owned(),
        }
    }

    #[test]
    fn generates_typed_diagram_call() {
        let expression = diagram_expression(
            &code_block("graph TD\nA[Start] --> B"),
            &RenderOptions::default(),
        )
        .expect("diagram generates");
        assert!(expression.starts_with("mdpdf-diagram(width: "));
        assert!(expression.contains("boxes: ("));
        assert!(expression.contains("lines: ("));
        assert!(expression.contains("\"Start\""));
        assert!(expression.contains("\"rect\""));
        assert!(expression.contains("\"arrow\""));
        // Координаты — числа в пунктах.
        assert!(expression.contains("pt, "));
    }

    #[test]
    fn labels_stay_inside_string_literals() {
        // `]` — терминатор подписи узла, поэтому в payload его нет; покрытие
        // произвольных payload'ов со скобками — в tests/typst_generator.rs.
        let payload = "x\" #place( rect(";
        let source = format!("graph TD\nA[{payload}] --> B");
        let expression =
            diagram_expression(&code_block(&source), &RenderOptions::default()).expect("generates");
        assert!(
            expression.contains("x\\\" #place( rect("),
            "quote must be escaped: {expression}"
        );
        assert!(
            !expression.contains("\n#place"),
            "payload must not break into markup: {expression}"
        );
    }

    #[test]
    fn small_diagram_keeps_full_scale() {
        let expression = diagram_expression(
            &code_block("graph TD\nA[Start] --> B"),
            &RenderOptions::default(),
        )
        .expect("diagram generates");
        assert!(expression.contains("fit: 100%"), "{expression}");
    }

    #[test]
    fn wide_diagram_is_scaled_down_with_text() {
        let edges: Vec<String> = (0..20).map(|i| format!("root --> child{i}")).collect();
        let source = format!("graph TD\n{}", edges.join("\n"));
        let expression =
            diagram_expression(&code_block(&source), &RenderOptions::default()).expect("generates");
        let fit = extract_fit(&expression);
        assert!(fit < 1.0, "wide diagram must shrink: {expression}");
        // Масштаб совпадает с отношением ширины области к естественной ширине.
        let natural_width = layout(&parse(&source).expect("parses"), 11.0).width;
        let expected = text_width_pt(&RenderOptions::default()) / natural_width;
        assert!(
            (fit - expected).abs() < 0.001,
            "fit {fit} must track width ratio {expected}"
        );
    }

    #[test]
    fn tall_diagram_is_scaled_down_to_page_height() {
        let messages: Vec<String> = (0..40).map(|i| format!("A->>B: сообщение {i}")).collect();
        let source = format!("sequenceDiagram\n{}", messages.join("\n"));
        let expression =
            diagram_expression(&code_block(&source), &RenderOptions::default()).expect("generates");
        let fit = extract_fit(&expression);
        assert!(fit < 1.0, "tall diagram must shrink: {expression}");
        let natural_height = layout(&parse(&source).expect("parses"), 11.0).height;
        let budget = text_height_pt(&RenderOptions::default());
        assert!(
            natural_height * fit <= budget + 0.01,
            "scaled height {} must fit budget {budget}",
            natural_height * fit
        );
    }

    /// Значение `fit: N%` из сгенерированного выражения (доля, не проценты).
    fn extract_fit(expression: &str) -> f64 {
        let marker = "fit: ";
        let start = expression.find(marker).expect("fit present") + marker.len();
        let end = expression[start..].find('%').expect("percent") + start;
        expression[start..end]
            .parse::<f64>()
            .expect("fit is a number")
            / 100.0
    }

    #[test]
    fn long_edge_label_is_wrapped_into_diagram_bounds() {
        let source = format!("graph TD\nA -->|{}| B", "очень длинная подпись ".repeat(20));
        let expression =
            diagram_expression(&code_block(&source), &RenderOptions::default()).expect("generates");
        // Подпись переносится по 180pt и входит в границы: диаграмма остаётся
        // в масштабе 100%, но её ширина и высота покрывают подпись.
        assert!(expression.contains("fit: 100%"), "{expression}");
        let placed = layout(&parse(&source).expect("parses"), 11.0);
        assert!(
            placed.width >= 180.0,
            "label must expand bounds: {placed:?}"
        );
    }

    #[test]
    fn huge_edge_label_scales_diagram_down() {
        let source = format!("graph TD\nA -->|{}| B", "x".repeat(5000));
        let expression =
            diagram_expression(&code_block(&source), &RenderOptions::default()).expect("generates");
        let fit = extract_fit(&expression);
        assert!(fit < 1.0, "huge label must shrink diagram: {expression}");
        let placed = layout(&parse(&source).expect("parses"), 11.0);
        let budget = text_height_pt(&RenderOptions::default());
        assert!(
            placed.height * fit <= budget + 0.01,
            "scaled height {} must fit budget {budget}",
            placed.height * fit
        );
    }

    #[test]
    fn unsupported_diagram_is_an_error_for_fallback() {
        let result =
            diagram_expression(&code_block("gantt\ntitle Plan"), &RenderOptions::default());
        assert!(matches!(
            result,
            Err(MermaidError::UnsupportedDiagramType { .. })
        ));
    }

    #[test]
    fn pt_formatting_is_stable() {
        assert_eq!(pt(0.0), "0pt");
        assert_eq!(pt(11.5), "11.5pt");
        assert_eq!(pt(1.0 / 3.0), "0.3333pt");
    }
}
