//! Форматирование выводимого Typst (ТЗ §26).
//!
//! Отступ — 2 пробела, перевод строки — LF, хвостовых пробелов нет,
//! завершающий LF обязателен.

use crate::typst_gen::error::TypstGenerationError;

/// Накопитель Typst-исходника с учётом отступов.
#[derive(Debug, Default)]
pub struct TypstWriter {
    buffer: String,
    depth: usize,
}

impl TypstWriter {
    /// Ширина одного уровня отступа (ТЗ §26).
    pub const INDENT: &'static str = "  ";

    /// Создаёт пустой накопитель.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавляет строку с текущим отступом.
    pub fn line(&mut self, text: &str) {
        if !text.is_empty() {
            for _ in 0..self.depth {
                self.buffer.push_str(Self::INDENT);
            }
            self.buffer.push_str(text.trim_end());
        }
        self.buffer.push('\n');
    }

    /// Добавляет готовый фрагмент как есть, без отступа.
    pub fn push_raw(&mut self, text: &str) {
        self.buffer.push_str(text);
        if !text.ends_with('\n') {
            self.buffer.push('\n');
        }
    }

    /// Пустая строка между верхнеуровневыми блоками (ТЗ §26).
    pub fn blank_line(&mut self) {
        self.buffer.push('\n');
    }

    /// Выполняет замыкание на один уровень глубже.
    ///
    /// # Errors
    ///
    /// Пробрасывает ошибку замыкания.
    pub fn indented<F>(&mut self, body: F) -> Result<(), TypstGenerationError>
    where
        F: FnOnce(&mut Self) -> Result<(), TypstGenerationError>,
    {
        self.depth += 1;
        let result = body(self);
        self.depth -= 1;
        result
    }

    /// То же для замыканий, которые не могут завершиться ошибкой.
    pub fn indented_infallible<F>(&mut self, body: F)
    where
        F: FnOnce(&mut Self),
    {
        self.depth += 1;
        body(self);
        self.depth -= 1;
    }

    /// Завершает вывод, гарантируя ровно один завершающий LF.
    #[must_use]
    pub fn finish(mut self) -> String {
        while self.buffer.ends_with("\n\n") {
            self.buffer.pop();
        }
        if !self.buffer.ends_with('\n') {
            self.buffer.push('\n');
        }
        self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indentation_is_two_spaces_per_level() {
        let mut writer = TypstWriter::new();
        writer.line("a");
        writer.indented_infallible(|writer| {
            writer.line("b");
            writer.indented_infallible(|writer| writer.line("c"));
        });
        assert_eq!(writer.finish(), "a\n  b\n    c\n");
    }

    #[test]
    fn trailing_whitespace_is_stripped() {
        let mut writer = TypstWriter::new();
        writer.line("a   ");
        assert_eq!(writer.finish(), "a\n");
    }

    #[test]
    fn output_ends_with_exactly_one_newline() {
        let mut writer = TypstWriter::new();
        writer.line("a");
        writer.blank_line();
        writer.blank_line();
        assert_eq!(writer.finish(), "a\n");
    }

    #[test]
    fn empty_line_carries_no_indentation() {
        let mut writer = TypstWriter::new();
        writer.indented_infallible(|writer| {
            writer.line("");
            writer.line("a");
        });
        assert_eq!(writer.finish(), "\n  a\n");
    }
}
