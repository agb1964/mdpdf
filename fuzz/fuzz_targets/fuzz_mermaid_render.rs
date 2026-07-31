//! Fuzz-таргет: рендеринг Mermaid не должен ронять процесс ни на каком входе
//! (ТЗ §19.4, §10.5.5).
//!
//! Раскладка `mermaid-rs-renderer` — сторонний код на `f32`, помеченный
//! автором как early development, поэтому паника здесь считается ожидаемым
//! исходом и ловится `catch_unwind` внутри `mdpdf::mermaid::render`. Инвариант
//! таргета: вызов всегда возвращает `Ok` или `Err`, но никогда не завершает
//! процесс — контракт «диаграмма не рвёт сборку».
//!
//! Произвольные байты интерпретируются как UTF-8 (невалидные последовательности
//! отбрасываются `String::from_utf8_lossy`, чтобы фаззер тратил бюджет на сам
//! рендерер, а не на перекодировку). Вход обрезается по лимиту §15: более
//! длинные исходники отсекаются проверкой размера ещё до рендерера, и фаззить
//! их бессмысленно.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdpdf::mermaid::{limits, render};

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let source = match source.char_indices().nth(limits::MAX_SOURCE_BYTES) {
        Some((index, _)) => &source[..index],
        None => &source,
    };

    if let Ok(rendered) = render(source) {
        // Инвариант: размеры конечны — иначе вписывание в страницу даст NaN.
        debug_assert!(rendered.width_px.is_finite() && rendered.height_px.is_finite());
        // Инвариант: успешный рендер всегда даёт непустой SVG.
        debug_assert!(!rendered.svg.is_empty());
    }
});
