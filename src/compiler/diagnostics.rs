//! Преобразование внутренних диагностик Typst в собственную модель (ТЗ §37).
//!
//! Пользователю не показываются Rust type names, `FileId(...)`, debug dump Typst
//! и backtrace: наружу уходит только текст сообщения, позиция и подсказки.

use typst::WorldExt;
use typst::diag::{Severity as TypstSeverity, SourceDiagnostic};

use crate::compiler::world::MdpdfWorld;

/// Уровень диагностического сообщения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Предупреждение: PDF всё равно создаётся (ТЗ §38).
    Warning,
    /// Ошибка: обработка прекращается.
    Error,
}

/// Место, к которому относится диагностика.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSource {
    /// Имя файла в терминах, понятных пользователю.
    pub file: String,
    /// Номер строки, если известен.
    pub line: Option<usize>,
    /// Номер столбца, если известен.
    pub column: Option<usize>,
}

/// Диагностическое сообщение компилятора.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Уровень.
    pub severity: Severity,
    /// Текст сообщения.
    pub message: String,
    /// Место возникновения.
    pub source: Option<DiagnosticSource>,
    /// Подсказки по исправлению.
    pub hints: Vec<String>,
}

impl Diagnostic {
    /// Однострочное представление для вывода в stderr.
    ///
    /// Тонкая обёртка над [`Display`](std::fmt::Display); в новом коде диагностику
    /// достаточно подставить в форматную строку.
    #[must_use]
    pub fn render(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for Diagnostic {
    /// Однострочное представление для вывода в stderr.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        match &self.source {
            Some(source) => {
                let line = source.line.unwrap_or(1);
                let column = source.column.unwrap_or(1);
                write!(
                    formatter,
                    "{}:{line}:{column}: {level}: {}",
                    source.file, self.message
                )
            }
            None => write!(formatter, "{level}: {}", self.message),
        }
    }
}

/// Переводит диагностики Typst в собственную модель.
#[must_use]
pub fn convert(world: &MdpdfWorld, diagnostics: &[SourceDiagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| convert_one(world, diagnostic))
        .collect()
}

fn convert_one(world: &MdpdfWorld, diagnostic: &SourceDiagnostic) -> Diagnostic {
    Diagnostic {
        severity: match diagnostic.severity {
            TypstSeverity::Error => Severity::Error,
            TypstSeverity::Warning => Severity::Warning,
        },
        message: diagnostic.message.to_string(),
        source: locate(world, diagnostic),
        hints: diagnostic
            .hints
            .iter()
            .map(|hint| hint.v.to_string())
            .collect(),
    }
}

/// Имя, под которым пользователю показывается сгенерированный Typst.
///
/// ТЗ §37: если ошибку нельзя точно сопоставить с Markdown, указывается
/// `generated Typst:42:17` — имя исходного `.md` здесь было бы враньём.
pub const GENERATED_TYPST: &str = "generated Typst";

/// Позиция в сгенерированном Typst. Сопоставление с Markdown-исходником
/// выполняется уровнем выше (ТЗ §37).
fn locate(world: &MdpdfWorld, diagnostic: &SourceDiagnostic) -> Option<DiagnosticSource> {
    let range = world.range(diagnostic.span)?;
    let (line, column) = world.line_column(range.start)?;
    Some(DiagnosticSource {
        file: GENERATED_TYPST.to_owned(),
        line: Some(line),
        column: Some(column),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_includes_position_when_known() {
        let diagnostic = Diagnostic {
            severity: Severity::Error,
            message: "unknown variable".to_owned(),
            source: Some(DiagnosticSource {
                file: GENERATED_TYPST.to_owned(),
                line: Some(42),
                column: Some(17),
            }),
            hints: vec![],
        };
        assert_eq!(
            diagnostic.render(),
            "generated Typst:42:17: error: unknown variable"
        );
    }

    #[test]
    fn rendering_without_position_still_names_the_level() {
        let diagnostic = Diagnostic {
            severity: Severity::Warning,
            message: "unused".to_owned(),
            source: None,
            hints: vec![],
        };
        assert_eq!(diagnostic.render(), "warning: unused");
    }

    #[test]
    fn missing_line_and_column_fall_back_to_one() {
        let diagnostic = Diagnostic {
            severity: Severity::Error,
            message: "broken".to_owned(),
            source: Some(DiagnosticSource {
                file: GENERATED_TYPST.to_owned(),
                line: None,
                column: None,
            }),
            hints: vec![],
        };
        assert_eq!(diagnostic.render(), "generated Typst:1:1: error: broken");
    }
}
