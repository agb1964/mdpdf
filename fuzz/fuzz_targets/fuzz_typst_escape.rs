//! Fuzz-таргет: функции экранирования Typst не паникуют и не позволяют
//! вырваться из контекста, в который подставлена строка (ТЗ §10.1, §10.2, §19.4).
//!
//! Прогоняет произвольную строку через все четыре функции экранирования.
//! Для `string_literal` дополнительно проверяется главный инвариант модуля:
//! результат — это ровно один строковый литерал (начинается и заканчивается
//! кавычкой), и внутри него нет НЕэкранированной кавычки, которой можно было
//! бы досрочно закрыть литерал и начать писать произвольный Typst-код.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdpdf::typst_gen::escape::{path_literal, string_literal, text_content, url_literal};

/// Проверяет, что `literal` — это один string-литерал Typst без возможности
/// вырваться из него раньше срока: снаружи кавычки, а внутри каждая `"`
/// обязательно предварена нечётным числом обработанных подряд `\`.
fn is_closed_string_literal(literal: &str) -> bool {
    let chars: Vec<char> = literal.chars().collect();
    if chars.len() < 2 {
        return false;
    }
    if chars[0] != '"' || *chars.last().expect("длина >= 2 проверена выше") != '"' {
        return false;
    }

    let end = chars.len() - 1;
    let mut i = 1;
    while i < end {
        match chars[i] {
            // Экранированная последовательность: `\` и следующий за ним символ
            // (каким бы он ни был) не могут закрыть литерал.
            '\\' => i += 2,
            // Любая другая кавычка внутри литерала — это выход из строки
            // раньше финальной кавычки: инвариант нарушен.
            '"' => return false,
            _ => i += 1,
        }
    }
    // Дошли строго до закрывающей кавычки без незакрытого "хвоста" escape-пары.
    i == end
}

fuzz_target!(|data: &[u8]| {
    let value = String::from_utf8_lossy(data);

    let literal = string_literal(&value);
    assert!(
        is_closed_string_literal(&literal),
        "string_literal произвела литерал, из которого можно вырваться: {literal:?} \
         (вход: {value:?})"
    );

    // Остальные функции: инвариант — отсутствие паники. `url_literal` и
    // `path_literal` вправе вернуть Err на невалидном входе, это ожидаемо.
    let _ = text_content(&value);
    let _ = url_literal(&value);
    let _ = path_literal(&value);
});
