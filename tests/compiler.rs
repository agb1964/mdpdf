//! Тесты этапа 3 (ТЗ §41). Наполняются на Milestone 3: ограниченный World,
//! виртуальная ФС, path traversal, диагностики Typst, экспорт PDF.

use mdpdf::compiler::{fonts, limits, pdf};

#[test]
fn all_five_fonts_are_embedded() {
    assert_eq!(fonts::EMBEDDED_FONTS.len(), 5);
    for font in fonts::EMBEDDED_FONTS {
        assert!(font.len() > 1024, "embedded font looks truncated");
    }
}

#[test]
fn font_registration_order_is_deterministic() {
    // ТЗ §34: порядок регистрации шрифтов фиксирован.
    assert_eq!(fonts::EMBEDDED_FONTS[0], fonts::NOTO_SANS_REGULAR);
    assert_eq!(fonts::EMBEDDED_FONTS[4], fonts::NOTO_SANS_MONO_REGULAR);
}

#[test]
fn pdf_signature_is_checked() {
    assert!(!pdf::looks_like_pdf(b"not a pdf"));
}

#[test]
fn resource_limits_match_specification() {
    assert_eq!(limits::MAX_AST_NODES, 1_000_000);
    assert_eq!(limits::MAX_NESTING_DEPTH, 128);
    assert_eq!(limits::MAX_IMAGES, 1_000);
    assert_eq!(limits::MAX_IMAGE_BYTES, 64 * 1024 * 1024);
    assert_eq!(limits::MAX_TOTAL_IMAGE_BYTES, 256 * 1024 * 1024);
    assert_eq!(limits::MAX_URL_BYTES, 16 * 1024);
    assert_eq!(limits::MAX_TEXT_NODE_BYTES, 16 * 1024 * 1024);
}
