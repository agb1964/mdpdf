//! Fuzz-таргет: парсер Markdown не должен паниковать и обязан завершаться
//! на любом входе (ТЗ §50).
//!
//! Произвольные байты интерпретируются как UTF-8 (невалидные последовательности
//! отбрасываются `String::from_utf8_lossy`, чтобы фаззер тратил бюджет на сам
//! парсер, а не на перекодировку). Результат парсинга не важен — важно, что
//! `parse` либо возвращает `Ok`, либо `Err`, но никогда не паникует и не виснет.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdpdf::markdown::parser::MarkdownParser;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let parser = MarkdownParser::default();
    // Инвариант: любой вход даёт Ok(Document) или Err(MarkdownError), без паники.
    let _ = parser.parse(&source);
});
