//! Тесты этапа 1 (ТЗ §17, §46). Наполняются на Milestone 1:
//! по одному тесту на тип узла, вложенные списки и цитаты, таблицы,
//! Unicode и кириллица, неправильное вложение, golden-тесты AST.

use mdpdf::source;

#[test]
fn input_normalization_matches_specification() {
    let text = source::decode_and_normalize("\u{feff}# Заголовок\r\nтекст\rконец".as_bytes())
        .expect("valid input");
    assert_eq!(text, "# Заголовок\nтекст\nконец");
}

#[test]
fn empty_markdown_is_a_valid_document() {
    assert_eq!(source::decode_and_normalize(b"").expect("valid input"), "");
}
