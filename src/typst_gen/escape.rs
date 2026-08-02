//! Экранирование пользовательского ввода (ТЗ §23).
//!
//! Для каждого контекста — отдельная функция. Одна универсальная функция на все
//! случаи запрещена, потому что правила несовместимы: то, что безопасно внутри
//! строкового литерала, небезопасно в content-контексте, и наоборот.
//!
//! Главный инвариант (ТЗ §22): ни один пользовательский фрагмент не должен
//! интерпретироваться как Typst-код.

use std::fmt::Write;

use thiserror::Error;

use crate::typst_gen::RESOURCE_PREFIX;

/// Ошибка экранирования.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EscapeError {
    /// Значение недопустимо в данном контексте.
    #[error("value cannot be escaped for {context}: {message}")]
    Invalid {
        /// Контекст экранирования.
        context: &'static str,
        /// Описание проблемы.
        message: String,
    },
}

impl EscapeError {
    fn invalid(context: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid {
            context,
            message: message.into(),
        }
    }
}

/// Строковый литерал Typst вместе с кавычками (ТЗ §23.1).
///
/// Экранируются обратная косая черта, кавычка, переводы строк, возврат каретки,
/// табуляция, нулевой байт и управляющие символы Unicode.
#[must_use]
pub fn string_literal(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // Управляющие символы и невидимые форматирующие: только через \u{...}.
            ch if is_control_like(ch) => {
                // Запись в String не может завершиться ошибкой.
                let _ = write!(result, "\\u{{{:x}}}", ch as u32);
            }
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}

/// Текст для markup-контекста Typst (ТЗ §23.2).
///
/// Исключает исполнение `#`, разметку `[`/`]`, математику `$`, обратную косую
/// черту, комментарии `//` и `/* */`, а также raw-разделители.
///
/// Генератор по умолчанию передаёт пользовательский текст строковыми литералами
/// и в markup не выводит — функция существует для контекстов, где content-форма
/// необходима, и покрыта тестами наравне с остальными.
#[must_use]
pub fn text_content(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' | '#' | '[' | ']' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '/' | '~' | '-'
            | '+' | '=' | '\'' | '"' => {
                result.push('\\');
                result.push(ch);
            }
            ch if is_control_like(ch) => result.push(' '),
            ch => result.push(ch),
        }
    }
    result
}

/// Адрес ссылки как строковый литерал (ТЗ §23).
///
/// Ссылка не загружается, поэтому допустима любая схема, но управляющие символы
/// и переводы строк в URL запрещены.
///
/// # Errors
///
/// [`EscapeError`], если адрес содержит управляющие символы или превышает
/// разумную длину.
pub fn url_literal(value: &str) -> Result<String, EscapeError> {
    const CONTEXT: &str = "url";
    if value.len() > MAX_URL_BYTES {
        return Err(EscapeError::invalid(
            CONTEXT,
            format!("url is {} bytes, limit is {MAX_URL_BYTES}", value.len()),
        ));
    }
    if let Some(ch) = value.chars().find(|ch| is_control_like(*ch)) {
        return Err(EscapeError::invalid(
            CONTEXT,
            format!("url contains a control character U+{:04X}", ch as u32),
        ));
    }
    Ok(string_literal(value))
}

/// Виртуальный путь ресурса как строковый литерал (ТЗ §23, §24.6).
///
/// Принимает **только** пути, которые построил сам генератор: начинающиеся
/// с [`RESOURCE_PREFIX`], без `..`, без обратных косых черт и управляющих
/// символов. Раньше проверялась лишь абсолютность, то есть контракт был шире
/// заявленного в этой доке: сюда прошёл бы любой абсолютный путь, включая
/// `/etc/passwd`. Реальный вызов один и передаёт сгенерированный путь, но
/// полагаться на это — значит держать защиту на честном слове.
///
/// # Errors
///
/// [`EscapeError`], если путь не удовлетворяет этим условиям.
pub fn path_literal(value: &str) -> Result<String, EscapeError> {
    const CONTEXT: &str = "path";
    if !value.starts_with(RESOURCE_PREFIX) {
        return Err(EscapeError::invalid(
            CONTEXT,
            format!("virtual resource path must start with {RESOURCE_PREFIX}"),
        ));
    }
    if value.contains("..") {
        return Err(EscapeError::invalid(
            CONTEXT,
            "virtual resource path must not contain ..",
        ));
    }
    if value.contains('\\') {
        return Err(EscapeError::invalid(
            CONTEXT,
            "virtual resource path must not contain a backslash",
        ));
    }
    if let Some(ch) = value.chars().find(|ch| is_control_like(*ch)) {
        return Err(EscapeError::invalid(
            CONTEXT,
            format!("path contains a control character U+{:04X}", ch as u32),
        ));
    }
    Ok(string_literal(value))
}

/// Ограничение длины URL (ТЗ §40).
///
/// `pub(crate)`, потому что то же значение переиспользуется в
/// `compiler::limits` (единый источник истины, чтобы лимиты не разъезжались
/// при правке одной из констант). `typst_gen` ничего не импортирует из
/// `compiler`, так что зависимость идёт в разрешённую сторону: `compiler` →
/// `typst_gen`.
pub(crate) const MAX_URL_BYTES: usize = 16 * 1024;

/// Управляющие и невидимые форматирующие символы.
fn is_control_like(ch: char) -> bool {
    ch.is_control()
        || matches!(ch,
            '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_wraps_in_quotes() {
        assert_eq!(string_literal("abc"), "\"abc\"");
    }

    #[test]
    fn string_literal_escapes_backslash_and_quote() {
        assert_eq!(string_literal(r#"a\b"c"#), r#""a\\b\"c""#);
    }

    #[test]
    fn string_literal_escapes_whitespace_controls() {
        assert_eq!(string_literal("a\nb\rc\td"), "\"a\\nb\\rc\\td\"");
    }

    #[test]
    fn string_literal_escapes_nul_and_control_characters() {
        assert_eq!(string_literal("a\0b"), "\"a\\u{0}b\"");
        assert_eq!(string_literal("a\u{7}b"), "\"a\\u{7}b\"");
    }

    #[test]
    fn string_literal_escapes_invisible_formatting() {
        assert_eq!(string_literal("a\u{202E}b"), "\"a\\u{202e}b\"");
        assert_eq!(string_literal("\u{feff}"), "\"\\u{feff}\"");
    }

    #[test]
    fn string_literal_keeps_cyrillic_as_is() {
        assert_eq!(string_literal("Привет, мир"), "\"Привет, мир\"");
    }

    #[test]
    fn string_literal_neutralises_injection_payloads() {
        for payload in [
            "#panic(\"injected\")",
            "#include \"secret\"",
            "#import \"@preview/package\"",
            "$ x + y $",
            "[content]",
            "\"quoted\"",
            "\\escaped",
            "// comment",
            "/* comment */",
            "```rust\ncode\n```",
        ] {
            let escaped = string_literal(payload);
            assert!(escaped.starts_with('"') && escaped.ends_with('"'));
            // Внутри литерала не может оказаться неэкранированной кавычки,
            // то есть выйти из него payload не может.
            let inner = &escaped[1..escaped.len() - 1];
            let mut chars = inner.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\\' {
                    chars.next();
                } else {
                    assert_ne!(ch, '"', "payload escaped the literal: {payload}");
                }
            }
        }
    }

    #[test]
    fn text_content_escapes_markup_characters() {
        assert_eq!(text_content("#panic()"), "\\#panic()");
        assert_eq!(text_content("[a]"), "\\[a\\]");
        assert_eq!(text_content("$x$"), "\\$x\\$");
        assert_eq!(text_content("a`b`"), "a\\`b\\`");
        assert_eq!(text_content("// comment"), "\\/\\/ comment");
    }

    #[test]
    fn text_content_keeps_cyrillic_as_is() {
        assert_eq!(text_content("Привет"), "Привет");
    }

    #[test]
    fn url_literal_accepts_ordinary_addresses() {
        assert_eq!(
            url_literal("https://example.com/a?b=1").expect("valid url"),
            "\"https://example.com/a?b=1\""
        );
        assert_eq!(url_literal("").expect("empty url"), "\"\"");
        assert_eq!(
            url_literal("mailto:a@b.c").expect("mailto"),
            "\"mailto:a@b.c\""
        );
    }

    #[test]
    fn url_literal_rejects_control_characters() {
        let err = url_literal("https://a\nb").expect_err("newline in url");
        assert!(matches!(err, EscapeError::Invalid { context: "url", .. }));
    }

    #[test]
    fn url_literal_rejects_overlong_values() {
        let long = "h".repeat(MAX_URL_BYTES + 1);
        assert!(url_literal(&long).is_err());
    }

    #[test]
    fn path_literal_accepts_generated_virtual_paths() {
        assert_eq!(
            path_literal("/mdpdf-resources/000001.png").expect("valid path"),
            "\"/mdpdf-resources/000001.png\""
        );
    }

    #[test]
    fn path_literal_rejects_traversal_and_relative_paths() {
        assert!(path_literal("images/a.png").is_err());
        assert!(path_literal("/mdpdf-resources/../../etc/passwd").is_err());
        assert!(path_literal("/mdpdf-resources\\a.png").is_err());
        assert!(path_literal("/mdpdf-resources/a\0.png").is_err());
    }

    #[test]
    fn path_literal_accepts_nothing_outside_the_resource_prefix() {
        // Контракт узкий намеренно: функция обслуживает ровно один вызов,
        // который передаёт путь, построенный генератором (ТЗ §24.6).
        for outside in [
            "/etc/passwd",
            "/main.typ",
            "/template.typ",
            "/mdpdf-resource/000001.png",
            "/",
            "",
        ] {
            assert!(
                path_literal(outside).is_err(),
                "path outside the resource prefix must be rejected: {outside}"
            );
        }
    }
}
