//! Fuzz-таргет: валидация AST не паникует ни на каком документе (ТЗ §14, §50).
//!
//! `MarkdownParser::parse` уже вызывает `validate_document` внутри себя, но
//! таргет всё равно прогоняет валидатор повторно на построенном дереве —
//! именно инварианты `validate_document` (а не парсера) здесь и проверяются:
//! на любом AST, которое парсер счёл достаточным, чтобы вернуть `Ok`,
//! валидатор обязан либо согласиться, либо вернуть `Err`, но не паниковать.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdpdf::ast::validate::validate_document;
use mdpdf::markdown::parser::MarkdownParser;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let parser = MarkdownParser::default();

    if let Ok(document) = parser.parse(&source) {
        // Документ уже прошёл валидацию внутри parse(), но инвариант
        // «validate_document никогда не паникует» проверяем отдельно и явно.
        let _ = validate_document(&document);
    }
});
