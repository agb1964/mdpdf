//! Проверка SVG на внешние ссылки (ТЗ §33.3).
//!
//! Лист-модуль: работает с голыми байтами и не знает ни о Markdown, ни о Typst,
//! ни о файловой системе. Нужен двум слоям сразу — [`crate::mermaid`] проверяет
//! выход рендерера до регистрации ресурса, чтобы «плохая» диаграмма
//! деградировала до блока кода (ТЗ §10.5.5), а [`crate::compiler`] повторяет
//! проверку для всех ресурсов уже как фатальную (защита в глубину).

/// Схемы, запрещённые в `href`/`xlink:href` внутри SVG (ТЗ §33.3).
///
/// `data:` запрещена наравне с сетевыми и файловыми схемами: это тоже способ
/// протащить произвольный, неограниченный по размеру и неучтённый лимитами
/// (ТЗ §40) блок байт в обход проверки самого файла-изображения.
const FORBIDDEN_SCHEMES: &[&str] = &["http:", "https:", "file:", "data:"];

/// Похожи ли байты на SVG (ТЗ §33.3).
#[must_use]
pub fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let Ok(text) = std::str::from_utf8(head) else {
        return false;
    };
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg"))
}

/// Ищет в SVG ссылку на внешний ресурс.
///
/// Проверяются два места: атрибут `href`/`xlink:href` и функциональная нотация
/// `url(...)` — последняя доступна из `style="fill:url(http://...)"`, из
/// `<style>@import url("https://...")</style>` и из презентационных атрибутов,
/// куда `href`-проверка не заглядывает вовсе.
///
/// Не покрыто намеренно: значение атрибута без кавычек (`href=http://...`) —
/// XML такого не допускает, и SVG-парсер отвергнет файл раньше нас.
///
/// Полноценный XML-парсер здесь не нужен (задача прямо это исключает):
/// достаточно построчного поиска атрибута и разбора значения в кавычках —
/// SVG с легитимным локальным содержимым (например, `tests/fixtures/images/dot.svg`)
/// такого атрибута вообще не содержит. Возвращает значение атрибута для
/// сообщения об ошибке.
#[must_use]
pub fn external_reference(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let lower = text.to_lowercase();
    let bytes = lower.as_bytes();

    let mut search_from = 0usize;
    while let Some(found) = lower[search_from..].find("href") {
        let href_start = search_from + found;
        let after_href = href_start + "href".len();

        // Между `href` и `=` допускаются пробелы (`href = "..."`).
        let mut cursor = skip_ascii_whitespace(bytes, after_href);
        if bytes.get(cursor) != Some(&b'=') {
            search_from = after_href;
            continue;
        }
        cursor = skip_ascii_whitespace(bytes, cursor + 1);

        let Some(&quote) = bytes.get(cursor) else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            search_from = after_href;
            continue;
        }
        let value_start = cursor + 1;
        let Some(value_end) = lower[value_start..].find(quote as char) else {
            break;
        };
        let value = lower[value_start..value_start + value_end].trim();

        if FORBIDDEN_SCHEMES
            .iter()
            .any(|scheme| value.starts_with(scheme))
        {
            return Some(value.to_owned());
        }

        search_from = value_start + value_end;
    }

    url_reference(&lower)
}

/// Ищет запрещённую схему внутри `url(...)`.
///
/// Кавычки внутри скобок необязательны (`url(http://a)` и `url("http://a")`
/// равноправны), поэтому значение обрезается по первой закрывающей скобке или
/// кавычке. Локальные ссылки вида `url(#gradient)`, которыми полон вывод
/// Mermaid, схеме не соответствуют и проходят.
fn url_reference(lower: &str) -> Option<String> {
    lower.match_indices("url(").find_map(|(index, matched)| {
        let value = lower[index + matched.len()..]
            .trim_start()
            .trim_start_matches(['"', '\'']);
        if !FORBIDDEN_SCHEMES
            .iter()
            .any(|scheme| value.starts_with(scheme))
        {
            return None;
        }
        let end = value.find([')', '"', '\'']).unwrap_or(value.len());
        Some(value[..end].trim().to_owned())
    })
}

/// Позиция первого непробельного байта начиная с `from`; конец среза, если
/// такого байта нет.
fn skip_ascii_whitespace(bytes: &[u8], from: usize) -> usize {
    bytes
        .get(from..)
        .and_then(|rest| rest.iter().position(|byte| !byte.is_ascii_whitespace()))
        .map_or(bytes.len(), |offset| from + offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_is_detected_by_root_element() {
        assert!(looks_like_svg(b"<svg xmlns=\"\"></svg>"));
        assert!(looks_like_svg(b"<?xml version=\"1.0\"?><svg></svg>"));
        assert!(!looks_like_svg(b"not an image"));
        assert!(!looks_like_svg(b""));
    }

    #[test]
    fn svg_with_no_href_has_no_external_reference() {
        assert_eq!(
            external_reference(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><circle/></svg>"),
            None
        );
    }

    #[test]
    fn svg_with_a_local_fragment_href_is_allowed() {
        assert_eq!(
            external_reference(
                b"<svg><use href=\"#icon\"/><use xlink:href=\"local.svg#a\"/></svg>"
            ),
            None
        );
    }

    #[test]
    fn svg_referencing_http_is_rejected() {
        let value = external_reference(b"<svg><image href=\"http://example.com/a.png\"/></svg>");
        assert_eq!(value.as_deref(), Some("http://example.com/a.png"));
    }

    #[test]
    fn svg_referencing_https_via_xlink_href_is_rejected() {
        let value =
            external_reference(b"<svg><image xlink:href=\"https://example.com/a.png\"/></svg>");
        assert_eq!(value.as_deref(), Some("https://example.com/a.png"));
    }

    #[test]
    fn svg_referencing_a_local_file_scheme_is_rejected() {
        let value = external_reference(b"<svg><image href=\"file:///etc/passwd\"/></svg>");
        assert_eq!(value.as_deref(), Some("file:///etc/passwd"));
    }

    #[test]
    fn svg_referencing_a_data_uri_is_rejected() {
        let value = external_reference(b"<svg><image href=\"data:image/png;base64,AAAA\"/></svg>");
        assert_eq!(value.as_deref(), Some("data:image/png;base64,aaaa"));
    }

    #[test]
    fn a_style_attribute_with_a_network_url_is_rejected() {
        let value =
            external_reference(br#"<svg><rect style="fill:url(http://example.com/a.png)"/></svg>"#);
        assert_eq!(value.as_deref(), Some("http://example.com/a.png"));
    }

    #[test]
    fn a_style_block_with_an_import_is_rejected() {
        let value = external_reference(
            br#"<svg><style>@import url("https://example.com/x.css");</style></svg>"#,
        );
        assert_eq!(value.as_deref(), Some("https://example.com/x.css"));
    }

    #[test]
    fn a_local_fragment_url_is_allowed() {
        assert_eq!(
            external_reference(b"<svg><rect fill=\"url(#gradient)\"/></svg>"),
            None
        );
    }

    #[test]
    fn href_with_spaces_around_equals_is_still_found() {
        let value = external_reference(b"<svg><image href = 'https://example.com/a.png'/></svg>");
        assert_eq!(value.as_deref(), Some("https://example.com/a.png"));
    }
}
