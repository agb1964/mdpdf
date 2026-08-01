//! Генерация inline-узлов (ТЗ §24.4–24.6).
//!
//! Каждый узел превращается в вызов функции Typst, а пользовательский текст —
//! в строковый литерал. Ни один фрагмент не попадает в markup как есть (ТЗ §22).

use crate::ast::inline::Inline;
use crate::typst_gen::error::TypstGenerationError;
use crate::typst_gen::escape::{path_literal, string_literal, url_literal};
use crate::typst_gen::generator::{ResourceKind, ResourceReference, ResourceSource};
use crate::typst_gen::next_logical_path;

/// Пустое содержимое Typst.
pub const EMPTY_CONTENT: &str = "[]";

/// Собирает выражение Typst для последовательности inline-элементов.
///
/// # Errors
///
/// [`TypstGenerationError`], если значение не удалось безопасно экранировать
/// или путь изображения недопустим.
pub fn inline_expression(
    inlines: &[Inline],
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    if inlines.is_empty() {
        return Ok(EMPTY_CONTENT.to_owned());
    }

    let mut parts = Vec::with_capacity(inlines.len());
    for inline in inlines {
        parts.push(single_inline(inline, resources)?);
    }
    Ok(parts.join(" + "))
}

fn single_inline(
    inline: &Inline,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let expression = match inline {
        Inline::Text(text) => format!("text({})", string_literal(text)),
        Inline::Code(code) => format!("mdpdf-inline-code({})", string_literal(code)),
        // Мягкий перенос — пробельный, жёсткий — принудительный (ТЗ §24.3).
        Inline::SoftBreak => "text(\" \")".to_owned(),
        Inline::HardBreak => "linebreak()".to_owned(),
        Inline::Emphasis(children) => format!("emph({})", inline_expression(children, resources)?),
        Inline::Strong(children) => format!("strong({})", inline_expression(children, resources)?),
        Inline::Strikethrough(children) => {
            format!("strike({})", inline_expression(children, resources)?)
        }
        Inline::Link(link) => {
            // Сетевой адрес допустим: ссылка не загружается (ТЗ §24.5).
            let url = url_literal(&link.value.destination).map_err(|error| {
                TypstGenerationError::InvalidUrl {
                    value: link.value.destination.clone(),
                    span: Some(link.span),
                    message: error.to_string(),
                }
            })?;
            let body = inline_expression(&link.value.content, resources)?;
            format!("link({url}, {body})")
        }
        Inline::Image(image) => {
            let logical_path = register_image(image, resources)?;
            let path = path_literal(&logical_path).map_err(|error| {
                TypstGenerationError::InvalidImagePath {
                    value: logical_path.clone(),
                    span: Some(image.span),
                    message: error.to_string(),
                }
            })?;
            // Alt — это описание, а не подпись: Typst принимает его строкой,
            // и изображение остаётся inline-элементом, как в Markdown.
            let alt = if image.value.alt.is_empty() {
                "none".to_owned()
            } else {
                string_literal(&plain_text(&image.value.alt))
            };
            format!("mdpdf-image(path: {path}, alt: {alt})")
        }
    };
    Ok(expression)
}

/// Плоский текст inline-последовательности: только содержимое, без разметки.
fn plain_text(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(value) | Inline::Code(value) => text.push_str(value),
            Inline::SoftBreak | Inline::HardBreak => text.push(' '),
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strikethrough(children) => text.push_str(&plain_text(children)),
            Inline::Link(link) => text.push_str(&plain_text(&link.value.content)),
            Inline::Image(image) => text.push_str(&plain_text(&image.value.alt)),
        }
    }
    text
}

/// Присваивает изображению виртуальный путь и запоминает соответствие (ТЗ §24.6).
///
/// Номера выдаются последовательно в порядке обхода AST, поэтому вывод
/// детерминирован (ТЗ §25).
fn register_image(
    image: &crate::ast::Spanned<crate::ast::inline::Image>,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let source = image.value.source.trim();
    if source.is_empty() {
        return Err(TypstGenerationError::InvalidImagePath {
            value: image.value.source.clone(),
            span: Some(image.span),
            message: "image path is empty".to_owned(),
        });
    }

    let logical_path = next_logical_path(resources, "", &extension_of(source));

    resources.push(ResourceReference {
        logical_path: logical_path.clone(),
        source: ResourceSource::File {
            path: source.to_owned(),
        },
        kind: ResourceKind::Image,
        span: Some(image.span),
    });
    Ok(logical_path)
}

/// Расширение исходного файла в нижнем регистре. Настоящее определение формата
/// выполняется компилятором по содержимому (ТЗ §33.3).
fn extension_of(source: &str) -> String {
    let tail = source.rsplit(['/', '\\']).next().unwrap_or(source);
    tail.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 8
                && extension.chars().all(|ch| ch.is_ascii_alphanumeric())
        })
        .unwrap_or_else(|| "img".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::inline::{Image, Link};
    use crate::ast::{SourceSpan, Spanned};

    fn span() -> SourceSpan {
        SourceSpan::new(0, 1)
    }

    #[test]
    fn empty_sequence_becomes_empty_content() {
        let mut resources = Vec::new();
        assert_eq!(
            inline_expression(&[], &mut resources).expect("empty"),
            EMPTY_CONTENT
        );
    }

    #[test]
    fn text_becomes_a_string_literal_inside_text() {
        let mut resources = Vec::new();
        let expression =
            inline_expression(&[Inline::Text("Привет".to_owned())], &mut resources).expect("text");
        assert_eq!(expression, "text(\"Привет\")");
    }

    #[test]
    fn injection_payload_stays_inside_the_literal() {
        let mut resources = Vec::new();
        let expression = inline_expression(
            &[Inline::Text("#panic(\"injected\")".to_owned())],
            &mut resources,
        )
        .expect("text");
        assert_eq!(expression, "text(\"#panic(\\\"injected\\\")\")");
    }

    #[test]
    fn formatting_nests() {
        let mut resources = Vec::new();
        let expression = inline_expression(
            &[Inline::Strong(vec![Inline::Emphasis(vec![Inline::Text(
                "a".to_owned(),
            )])])],
            &mut resources,
        )
        .expect("nested");
        assert_eq!(expression, "strong(emph(text(\"a\")))");
    }

    #[test]
    fn breaks_are_distinguished() {
        let mut resources = Vec::new();
        let expression = inline_expression(&[Inline::SoftBreak, Inline::HardBreak], &mut resources)
            .expect("breaks");
        assert_eq!(expression, "text(\" \") + linebreak()");
    }

    #[test]
    fn images_get_sequential_virtual_paths() {
        let mut resources = Vec::new();
        let image = |source: &str| {
            Inline::Image(Spanned::new(
                Image {
                    source: source.to_owned(),
                    title: None,
                    alt: vec![],
                },
                span(),
            ))
        };
        let expression = inline_expression(
            &[image("a/one.PNG"), image("two.jpeg"), image("three")],
            &mut resources,
        )
        .expect("images");

        assert!(expression.contains("/mdpdf-resources/000001.png"));
        assert!(expression.contains("/mdpdf-resources/000002.jpeg"));
        assert!(expression.contains("/mdpdf-resources/000003.img"));
        assert_eq!(resources.len(), 3);
        assert_eq!(resources[0].display_path(), "a/one.PNG");
    }

    #[test]
    fn empty_image_path_is_rejected() {
        let mut resources = Vec::new();
        let image = Inline::Image(Spanned::new(
            Image {
                source: "   ".to_owned(),
                title: None,
                alt: vec![],
            },
            span(),
        ));
        let err = inline_expression(&[image], &mut resources).expect_err("empty path");
        assert!(matches!(err, TypstGenerationError::InvalidImagePath { .. }));
    }

    #[test]
    fn link_url_is_a_literal_and_network_addresses_are_allowed() {
        let mut resources = Vec::new();
        let link = Inline::Link(Spanned::new(
            Link {
                destination: "https://example.com".to_owned(),
                title: None,
                content: vec![Inline::Text("текст".to_owned())],
            },
            span(),
        ));
        let expression = inline_expression(&[link], &mut resources).expect("link");
        assert_eq!(expression, "link(\"https://example.com\", text(\"текст\"))");
    }

    #[test]
    fn link_url_with_control_characters_is_rejected() {
        let mut resources = Vec::new();
        let link = Inline::Link(Spanned::new(
            Link {
                destination: "https://a\nb".to_owned(),
                title: None,
                content: vec![],
            },
            span(),
        ));
        let err = inline_expression(&[link], &mut resources).expect_err("bad url");
        assert!(matches!(err, TypstGenerationError::InvalidUrl { .. }));
    }
}
