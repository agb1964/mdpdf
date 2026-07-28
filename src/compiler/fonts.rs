//! Встроенные шрифты (ТЗ §34).
//!
//! Байты подключаются через `include_bytes!`, системные шрифты не используются,
//! порядок регистрации детерминирован, разбор выполняется один раз на процесс.
//!
//! Состав шире перечня §34: к пяти начертаниям Noto Sans добавлен
//! Noto Color Emoji. Без него эмодзи молча исчезали из PDF — глифов нет ни
//! в одном текстовом начертании, а подставить системный шрифт запрещает §32.
//! Расширение согласовано с заказчиком; цена — рост бинарника на ~10 МБ.

use std::collections::BTreeMap;
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

/// Noto Color Emoji. Цветной растровый шрифт (CBDT/CBLC): Typst кладёт глифы
/// в PDF как ICC-цветные изображения с альфа-каналом, поэтому цвет сохраняется.
pub const NOTO_COLOR_EMOJI: &[u8] = include_bytes!("../../assets/fonts/NotoColorEmoji.ttf");

/// Начертания основного текста. Именно они обязаны покрывать кириллицу (§34).
pub const TEXT_FONTS: [&[u8]; 5] = [
    NOTO_SANS_REGULAR,
    NOTO_SANS_BOLD,
    NOTO_SANS_ITALIC,
    NOTO_SANS_BOLD_ITALIC,
    NOTO_SANS_MONO_REGULAR,
];

/// Все встроенные шрифты в детерминированном порядке регистрации (ТЗ §34).
pub const EMBEDDED_FONTS: [&[u8]; 6] = [
    NOTO_SANS_REGULAR,
    NOTO_SANS_BOLD,
    NOTO_SANS_ITALIC,
    NOTO_SANS_BOLD_ITALIC,
    NOTO_SANS_MONO_REGULAR,
    NOTO_COLOR_EMOJI,
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

    /// Есть ли глиф символа хотя бы в одном из встроенных шрифтов.
    #[must_use]
    pub fn covers(&self, character: char) -> bool {
        self.fonts
            .iter()
            .any(|font| font.info().coverage.contains(character as u32))
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

/// Символы текста, для которых ни в одном встроенном шрифте нет глифа.
///
/// Такой символ не выводится ни как замещающий прямоугольник, ни как ошибка —
/// он просто исчезает из PDF, а утилита рапортует об успехе. Подставить
/// системный шрифт нельзя (§32), поэтому единственное, что можно сделать
/// честно, — предупредить (§38).
///
/// Возвращает пары «символ, сколько раз встретился», отсортированные по
/// убыванию частоты; при равной частоте — по коду символа, чтобы вывод был
/// детерминирован (§25).
///
/// # Errors
///
/// [`CompileError::Font`], если встроенные шрифты не разбираются.
pub fn uncovered_characters(text: &str) -> Result<Vec<(char, usize)>, CompileError> {
    let set = embedded_fonts()?;

    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    for character in text.chars().filter(|character| needs_glyph(*character)) {
        *counts.entry(character).or_default() += 1;
    }
    // Покрытие спрашивается один раз на различающийся символ, а не на каждое
    // вхождение: у большого документа символов миллионы, а различных — сотни.
    counts.retain(|character, _| !set.covers(*character));

    let mut uncovered: Vec<(char, usize)> = counts.into_iter().collect();
    uncovered.sort_by(|(left_char, left_count), (right_char, right_count)| {
        right_count.cmp(left_count).then(left_char.cmp(right_char))
    });
    Ok(uncovered)
}

/// Нужен ли символу глиф вообще.
///
/// Пробелы и переводы строк рисовать нечем и незачем; управляющие символы
/// и невидимые модификаторы (селекторы начертания, ZWJ, метки направления)
/// глифов не имеют по определению и попадали бы в предупреждение шумом.
fn needs_glyph(character: char) -> bool {
    !character.is_whitespace()
        && !character.is_control()
        && !matches!(character,
            '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
        )
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
    fn every_text_face_covers_cyrillic() {
        let set = embedded_fonts().expect("fonts parse");
        // Проверяются только текстовые начертания: эмодзи-шрифт кириллицу
        // не покрывает и не должен — он подключён ради символов, которых нет
        // в Noto Sans.
        for index in 0..TEXT_FONTS.len() {
            let font = set.font(index).expect("font exists");
            // U+0410 «А» — базовая кириллическая буква.
            assert!(
                font.info().coverage.contains(0x0410),
                "text font #{index} has no Cyrillic coverage"
            );
        }
    }

    #[test]
    fn characters_without_glyphs_are_reported() {
        // U+E000 — область частного использования: глифа нет ни в одном
        // из встроенных шрифтов, и подставить его неоткуда.
        let uncovered =
            uncovered_characters("текст \u{E000}\u{E000} и \u{E001}").expect("fonts parse");
        assert_eq!(uncovered, vec![('\u{E000}', 2), ('\u{E001}', 1)]);
    }

    #[test]
    fn covered_text_reports_nothing() {
        let uncovered =
            uncovered_characters("Съешь ещё этих булок — 🔴 100% ok").expect("fonts parse");
        assert!(uncovered.is_empty(), "unexpected report: {uncovered:?}");
    }

    #[test]
    fn invisible_characters_are_not_reported() {
        // Пробелы, переводы строк и невидимые модификаторы глифов не имеют
        // по определению — в предупреждении они были бы шумом.
        let uncovered =
            uncovered_characters(" \t\n\u{200B}\u{FE0F}\u{200D}\u{FEFF}").expect("fonts parse");
        assert!(uncovered.is_empty(), "unexpected report: {uncovered:?}");
    }

    #[test]
    fn the_report_is_ordered_by_frequency_then_code_point() {
        let uncovered =
            uncovered_characters("\u{E001}\u{E000}\u{E000}\u{E002}").expect("fonts parse");
        assert_eq!(
            uncovered,
            vec![('\u{E000}', 2), ('\u{E001}', 1), ('\u{E002}', 1)]
        );
    }

    #[test]
    fn the_emoji_face_covers_what_text_faces_do_not() {
        let set = embedded_fonts().expect("fonts parse");
        let emoji = set
            .font(TEXT_FONTS.len())
            .expect("emoji font is registered last");
        // 🔴 U+1F534 — из-за него эмодзи и понадобились: в Noto Sans его нет.
        assert!(emoji.info().coverage.contains(0x0001_F534));
        for index in 0..TEXT_FONTS.len() {
            let text = set.font(index).expect("font exists");
            assert!(
                !text.info().coverage.contains(0x0001_F534),
                "text font #{index} unexpectedly covers an emoji"
            );
        }
    }
}
