//! Экспорт документа Typst в PDF и проверка результата (ТЗ §35, §39).
//!
//! Проверяется, что байты непусты, начинаются с `%PDF-` и имеют разумный
//! минимальный размер. Полный повторный парсинг PDF не выполняется.
//! Наполняется на Milestone 3.

/// Сигнатура, с которой обязан начинаться корректный PDF.
pub const PDF_MAGIC: &[u8] = b"%PDF-";

/// Минимальный правдоподобный размер PDF, байты.
pub const MIN_PDF_BYTES: usize = 512;

/// Быстрая проверка байтов PDF (ТЗ §39).
#[must_use]
pub fn looks_like_pdf(bytes: &[u8]) -> bool {
    bytes.len() >= MIN_PDF_BYTES && bytes.starts_with(PDF_MAGIC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bytes_are_rejected() {
        assert!(!looks_like_pdf(b""));
    }

    #[test]
    fn wrong_magic_is_rejected() {
        let bytes = [b'X'; MIN_PDF_BYTES];
        assert!(!looks_like_pdf(&bytes));
    }

    #[test]
    fn too_small_is_rejected() {
        assert!(!looks_like_pdf(b"%PDF-1.7"));
    }

    #[test]
    fn plausible_pdf_is_accepted() {
        let mut bytes = PDF_MAGIC.to_vec();
        bytes.resize(MIN_PDF_BYTES, b'\n');
        assert!(looks_like_pdf(&bytes));
    }
}
