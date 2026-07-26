//! Встроенные шрифты (ТЗ §34).
//!
//! Байты подключаются через `include_bytes!`, системные шрифты не используются,
//! порядок регистрации детерминирован, разбор выполняется один раз на процесс.

use std::sync::OnceLock;

use typst::text::{Font, FontBook};
use typst::utils::LazyHash;

use crate::compiler::error::CompileError;

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

/// Все встроенные шрифты в детерминированном порядке регистрации (ТЗ §34).
pub const EMBEDDED_FONTS: [&[u8]; 5] = [
    NOTO_SANS_REGULAR,
    NOTO_SANS_BOLD,
    NOTO_SANS_ITALIC,
    NOTO_SANS_BOLD_ITALIC,
    NOTO_SANS_MONO_REGULAR,
];

/// Разобранные шрифты вместе с каталогом для Typst.
#[derive(Debug)]
pub struct EmbeddedFontSet {
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl EmbeddedFontSet {
    /// Каталог шрифтов для `World::book`.
    #[must_use]
    pub const fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    /// Шрифт по индексу для `World::font`.
    #[must_use]
    pub fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    /// Количество разобранных начертаний.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    /// Нет ли ни одного начертания.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
}

/// Разбирает встроенные шрифты один раз на процесс (ТЗ §34, §58).
///
/// # Errors
///
/// [`CompileError::Font`], если встроенный шрифт не удалось разобрать. Это
/// внутренняя ошибка сборки, а не пользовательская: паниковать нельзя (ТЗ §53).
pub fn embedded_fonts() -> Result<&'static EmbeddedFontSet, CompileError> {
    static FONTS: OnceLock<Result<EmbeddedFontSet, String>> = OnceLock::new();

    FONTS
        .get_or_init(parse_embedded_fonts)
        .as_ref()
        .map_err(|message| CompileError::Font {
            message: message.clone(),
        })
}

fn parse_embedded_fonts() -> Result<EmbeddedFontSet, String> {
    let mut fonts = Vec::with_capacity(EMBEDDED_FONTS.len());
    for (index, data) in EMBEDDED_FONTS.iter().enumerate() {
        let bytes = typst::foundations::Bytes::new(*data);
        let font = Font::new(bytes, 0)
            .ok_or_else(|| format!("embedded font #{index} could not be parsed"))?;
        fonts.push(font);
    }
    let book = FontBook::from_fonts(&fonts);
    Ok(EmbeddedFontSet {
        book: LazyHash::new(book),
        fonts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fonts_are_embedded_truetype() {
        for font in EMBEDDED_FONTS {
            assert!(!font.is_empty(), "embedded font is empty");
            let magic = &font[..4];
            assert!(
                magic == [0x00, 0x01, 0x00, 0x00] || magic == b"OTTO",
                "unexpected font magic: {magic:?}"
            );
        }
    }

    #[test]
    fn embedded_fonts_parse() {
        let set = embedded_fonts().expect("fonts parse");
        assert_eq!(set.len(), EMBEDDED_FONTS.len());
        assert!(!set.is_empty());
        assert!(set.font(0).is_some());
        assert!(set.font(EMBEDDED_FONTS.len()).is_none());
    }

    #[test]
    fn fonts_are_parsed_once_per_process() {
        let first = embedded_fonts().expect("fonts parse");
        let second = embedded_fonts().expect("fonts parse");
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn every_face_covers_cyrillic() {
        let set = embedded_fonts().expect("fonts parse");
        for index in 0..set.len() {
            let font = set.font(index).expect("font exists");
            // U+0410 «А» — базовая кириллическая буква.
            assert!(
                font.info().coverage.contains(0x0410),
                "font #{index} has no Cyrillic coverage"
            );
        }
    }
}
