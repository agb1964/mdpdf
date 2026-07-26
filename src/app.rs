//! Конвейер приложения (ТЗ §42).
//!
//! Порядок шагов зафиксирован: чтение → парсинг → валидация → генерация Typst →
//! (диагностический вывод) → компиляция → атомарная запись PDF.
//! На Milestone 0 реализованы только чтение входа и разбор конфигурации;
//! остальные шаги возвращают [`AppError::NotImplemented`].

use crate::cli::Cli;
use crate::config::AppConfig;
use crate::error::{AppError, ExitStatus};
use crate::source;

/// Выполняет конвейер целиком.
///
/// # Errors
///
/// Любая ошибка этапа конвейера возвращается как [`AppError`] и отображается
/// в код завершения по ТЗ §43.
pub fn run(args: Cli) -> Result<ExitStatus, AppError> {
    let config = AppConfig::from_cli(args)?;
    let document = source::read_source(&config)?;

    if config.verbose {
        eprintln!(
            "mdpdf: read {} ({} bytes)",
            document.name,
            document.text.len()
        );
    }

    // Milestone 1: markdown::parser::MarkdownParser::parse + ast::validate.
    Err(AppError::NotImplemented {
        feature: "Markdown parsing (Milestone 1)",
    })
}
