//! Чтение входного документа и нормализация текста (ТЗ §13).
//!
//! Правила нормализации:
//! * UTF-8 BOM в начале файла удаляется;
//! * CRLF и одиночный CR превращаются в LF;
//! * остальные Unicode-символы сохраняются как есть;
//! * Unicode normalization (NFC/NFKC) не выполняется;
//! * trailing spaces сохраняются;
//! * нулевой байт — ошибка входа.

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::config::{AppConfig, InputSource};
use crate::error::AppError;

/// Прочитанный и нормализованный исходный документ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    /// Имя для диагностических сообщений: путь файла или `<stdin>`.
    pub name: String,
    /// Нормализованный текст Markdown.
    pub text: String,
}

/// Максимальный размер входного Markdown (ТЗ §40).
pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

/// Читает документ согласно конфигурации.
///
/// # Errors
///
/// [`AppError::Input`] при ошибке ввода-вывода, [`AppError::InvalidInput`] —
/// если вход не UTF-8, содержит нулевой байт или превышает [`MAX_INPUT_BYTES`].
pub fn read_source(config: &AppConfig) -> Result<SourceDocument, AppError> {
    match &config.input {
        InputSource::File(path) => read_file(path),
        InputSource::Stdin => read_stdin(),
    }
}

fn read_file(path: &Path) -> Result<SourceDocument, AppError> {
    let bytes = fs::read(path).map_err(|source| AppError::Input {
        path: path.display().to_string(),
        source,
    })?;
    Ok(SourceDocument {
        name: path.display().to_string(),
        text: decode_and_normalize(&bytes)?,
    })
}

fn read_stdin() -> Result<SourceDocument, AppError> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|source| AppError::Input {
            path: "<stdin>".to_owned(),
            source,
        })?;
    Ok(SourceDocument {
        name: "<stdin>".to_owned(),
        text: decode_and_normalize(&bytes)?,
    })
}

/// Декодирует UTF-8 и применяет правила нормализации ТЗ §13.
///
/// # Errors
///
/// [`AppError::InvalidInput`], если вход слишком велик, не является UTF-8
/// или содержит нулевой байт.
pub fn decode_and_normalize(bytes: &[u8]) -> Result<String, AppError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(AppError::InvalidInput {
            message: format!(
                "input is {} bytes, limit is {MAX_INPUT_BYTES} bytes",
                bytes.len()
            ),
        });
    }

    let without_bom = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);

    let text = std::str::from_utf8(without_bom).map_err(|err| AppError::InvalidInput {
        message: format!("input is not valid UTF-8 at byte {}", err.valid_up_to()),
    })?;

    if let Some(position) = text.find('\0') {
        return Err(AppError::InvalidInput {
            message: format!("input contains a NUL byte at offset {position}"),
        });
    }

    Ok(normalize_line_endings(text))
}

/// CRLF и одиночный CR → LF.
#[must_use]
pub fn normalize_line_endings(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }

    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_is_stripped() {
        let text =
            decode_and_normalize(b"\xEF\xBB\xBF# \xD0\x9f\xD1\x80\xD0\xb8\xD0\xb2\xD0\xb5\xD1\x82")
                .expect("valid input");
        assert_eq!(text, "# Привет");
    }

    #[test]
    fn crlf_and_lone_cr_become_lf() {
        let text = decode_and_normalize(b"a\r\nb\rc\nd").expect("valid input");
        assert_eq!(text, "a\nb\nc\nd");
    }

    #[test]
    fn trailing_spaces_are_preserved() {
        let text = decode_and_normalize(b"line  \nnext").expect("valid input");
        assert_eq!(text, "line  \nnext");
    }

    #[test]
    fn nul_byte_is_rejected() {
        let err = decode_and_normalize(b"a\0b").expect_err("NUL byte");
        assert!(matches!(err, AppError::InvalidInput { .. }));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let err = decode_and_normalize(&[0xFF, 0xFE]).expect_err("invalid UTF-8");
        assert!(matches!(err, AppError::InvalidInput { .. }));
    }

    #[test]
    fn empty_input_is_valid() {
        assert_eq!(decode_and_normalize(b"").expect("valid input"), "");
    }
}
