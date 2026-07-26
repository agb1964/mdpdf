//! Встроенные шрифты (ТЗ §34).
//!
//! Байты подключаются через `include_bytes!`, системные шрифты не используются,
//! порядок регистрации детерминирован, разбор выполняется один раз на процесс
//! (`OnceLock`). Подключение к Typst-`World` — на Milestone 3.

/// Noto Sans Regular.
pub const NOTO_SANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
/// Noto Sans Bold.
pub const NOTO_SANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Bold.ttf");
/// Noto Sans Italic.
pub const NOTO_SANS_ITALIC: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Italic.ttf");
/// Noto Sans Bold Italic.
pub const NOTO_SANS_BOLD_ITALIC: &[u8] =
    include_bytes!("../../assets/fonts/NotoSans-BoldItalic.ttf");
/// Noto Sans Mono Regular.
pub const NOTO_SANS_MONO_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/NotoSansMono-Regular.ttf");

/// Все встроенные шрифты в детерминированном порядке регистрации.
pub const EMBEDDED_FONTS: [&[u8]; 5] = [
    NOTO_SANS_REGULAR,
    NOTO_SANS_BOLD,
    NOTO_SANS_ITALIC,
    NOTO_SANS_BOLD_ITALIC,
    NOTO_SANS_MONO_REGULAR,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fonts_are_embedded_truetype() {
        for font in EMBEDDED_FONTS {
            assert!(!font.is_empty(), "embedded font is empty");
            // TrueType-контейнер начинается с 0x00010000, OpenType/CFF — с "OTTO".
            let magic = &font[..4];
            assert!(
                magic == [0x00, 0x01, 0x00, 0x00] || magic == b"OTTO",
                "unexpected font magic: {magic:?}"
            );
        }
    }
}
