//! Преобразование внутренних диагностик Typst в собственную модель (ТЗ §37).
//!
//! Пользователю не показываются Rust type names, `FileId(...)`, debug dump Typst
//! и backtrace. Наполняется на Milestone 3.

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
