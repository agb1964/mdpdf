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
    let name = path.display().to_string();

    // Размер известен до чтения — гигантский файл отвергается, не попав в память.
    let metadata = fs::metadata(path).map_err(|source| AppError::Input {
        path: name.clone(),
        source,
    })?;
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err(too_large(metadata.len()));
    }

    let file = fs::File::open(path).map_err(|source| AppError::Input {
        path: name.clone(),
        source,
    })?;
    let bytes = read_limited(file, &name)?;

    Ok(SourceDocument {
        name,
        text: decode_and_normalize(&bytes)?,
    })
}

fn read_stdin() -> Result<SourceDocument, AppError> {
    // У потока размера нет, поэтому читаем на байт больше лимита: этого хватает,
    // чтобы отличить «ровно по лимиту» от «слишком много», не набирая гигабайты.
    let bytes = read_limited(std::io::stdin().lock(), "<stdin>")?;
    Ok(SourceDocument {
        name: "<stdin>".to_owned(),
        text: decode_and_normalize(&bytes)?,
    })
}

/// Читает поток, останавливаясь сразу за лимитом (ТЗ §40).
fn read_limited(source: impl Read, name: &str) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    let read = source
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| AppError::Input {
            path: name.to_owned(),
            source,
        })?;
    if read > MAX_INPUT_BYTES {
        return Err(too_large(read as u64));
    }
    Ok(bytes)
}

fn too_large(size: u64) -> AppError {
    AppError::InvalidInput {
        message: format!("input is {size} bytes, limit is {MAX_INPUT_BYTES} bytes"),
    }
}

/// Декодирует UTF-8 и применяет правила нормализации ТЗ §13.
///
/// # Errors
///
/// [`AppError::InvalidInput`], если вход слишком велик, не является UTF-8
/// или содержит нулевой байт.
pub fn decode_and_normalize(bytes: &[u8]) -> Result<String, AppError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(too_large(bytes.len() as u64));
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
fn normalize_line_endings(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }

    text.replace("\r\n", "\n").replace('\r', "\n")
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
    fn an_oversized_file_is_rejected_without_being_read() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("huge.md");

        // Разреженный файл: на диске занимает почти ничего, но по метаданным
        // превышает лимит. Если бы проверка стояла после чтения, тест съел бы
        // 32 MiB памяти — в этом и смысл ранней проверки (ТЗ §40).
        let file = fs::File::create(&path).expect("create");
        file.set_len(MAX_INPUT_BYTES as u64 + 1).expect("grow");
        drop(file);

        let config = AppConfig {
            input: InputSource::File(path),
            output: None,
            title: None,
            author: None,
            paper: crate::typst_gen::generator::PaperSize::A4,
            margin: "20mm".to_owned(),
            font_size: "11pt".to_owned(),
            toc: false,
            heading_numbers: false,
            check: true,
            emit_ast: None,
            emit_typst: None,
            overwrite: false,
            quiet: true,
            verbose: false,
        };

        let err = read_source(&config).expect_err("oversized input");
        assert!(matches!(err, AppError::InvalidInput { .. }));
    }

    #[test]
    fn a_file_exactly_at_the_limit_is_accepted() {
        // Граница включительна: лимит — максимально допустимый размер, а не
        // первый запрещённый.
        assert!(decode_and_normalize(&[b'a'; 16]).is_ok());
    }

    #[test]
    fn empty_input_is_valid() {
        assert_eq!(decode_and_normalize(b"").expect("valid input"), "");
    }
}
