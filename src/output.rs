//! Запись результата (ТЗ §6.4).
//!
//! На Milestone 0 реализовано только вычисление пути выходного файла.
//! Атомарная запись PDF (временный файл → flush → rename) появится на Milestone 4.

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Путь PDF по умолчанию: `input.md` → `input.pdf` (ТЗ §5.1).
#[must_use]
pub fn default_output_path(input: &Path) -> PathBuf {
    input.with_extension("pdf")
}

/// Атомарно записывает PDF рядом с целевым файлом.
///
/// # Errors
///
/// На этапе каркаса всегда возвращает [`AppError::NotImplemented`].
pub fn write_pdf_atomically(_path: &Path, _bytes: &[u8], _overwrite: bool) -> Result<(), AppError> {
    Err(AppError::NotImplemented {
        feature: "atomic PDF write",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_replaced() {
        assert_eq!(
            default_output_path(Path::new("docs/input.md")),
            PathBuf::from("docs/input.pdf")
        );
    }

    #[test]
    fn missing_extension_gets_pdf() {
        assert_eq!(
            default_output_path(Path::new("README")),
            PathBuf::from("README.pdf")
        );
    }

    #[test]
    fn dots_in_name_are_preserved() {
        assert_eq!(
            default_output_path(Path::new("v1.2.notes.md")),
            PathBuf::from("v1.2.notes.pdf")
        );
    }
}
