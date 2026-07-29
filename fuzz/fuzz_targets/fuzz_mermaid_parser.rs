//! Fuzz-таргет: парсер и раскладка Mermaid не должны паниковать и обязаны
//! завершаться на любом входе (ТЗ §19.4).
//!
//! Произвольные байты интерпретируются как UTF-8 (невалидные последовательности
//! отбрасываются `String::from_utf8_lossy`, чтобы фаззер тратил бюджет на сам
//! парсер, а не на перекодировку). Раскладка тоже фаззится: она обязана
//! переварить любую модель, которую способен выдать парсер.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdpdf::mermaid::{layout, parse};

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    // Инвариант: любой вход даёт Ok(Diagram) или Err(MermaidError), без паники.
    if let Ok(diagram) = parse(&source) {
        // Инвариант: раскладка не паникует и даёт конечные координаты.
        let placed = layout(&diagram, 11.0);
        debug_assert!(placed.width.is_finite() && placed.height.is_finite());
    }
});
