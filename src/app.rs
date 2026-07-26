//! Конвейер приложения (ТЗ §42).
//!
//! Порядок шагов зафиксирован: чтение → парсинг → валидация → генерация Typst →
//! (диагностический вывод) → компиляция → атомарная запись PDF.
//! Реализованы шаги до построения AST включительно; генерация Typst и
//! компиляция появятся на Milestone 2–3.

use std::fs;
use std::path::Path;

use crate::ast::document::Document;
use crate::ast::metadata::DocumentMetadata;
use crate::cli::Cli;
use crate::config::AppConfig;
use crate::error::{AppError, ExitStatus};
use crate::markdown::parser::MarkdownParser;
use crate::source::{self, SourceDocument};

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

    let ast = parse(&document, &config)?;

    if config.verbose {
        eprintln!("mdpdf: parsed {} top-level blocks", ast.blocks.len());
    }

    if let Some(path) = &config.emit_ast {
        write_ast(path, &ast)?;
        if !config.quiet {
            println!("Created {}", path.display());
        }
    }

    // Milestone 2: typst_gen::generator::TypstGenerator::generate.
    Err(AppError::NotImplemented {
        feature: "Typst generation (Milestone 2)",
    })
}

/// Строит AST из прочитанного документа.
///
/// Диагностика дополняется позицией `файл:строка:столбец` (ТЗ §16).
fn parse(document: &SourceDocument, config: &AppConfig) -> Result<Document, AppError> {
    let metadata = DocumentMetadata {
        title: config.title.clone(),
        author: config.author.clone(),
        language: None,
    };
    let parser = MarkdownParser::default().with_metadata(metadata);

    parser
        .parse(&document.text)
        .map_err(|error| match error.span() {
            Some(span) => {
                let (line, column) = span.line_column(&document.text);
                AppError::MarkdownAt {
                    location: format!("{}:{line}:{column}", document.name),
                    source: error,
                }
            }
            None => AppError::Markdown(error),
        })
}

/// Записывает AST в JSON (ТЗ §5.6).
///
/// Формат не является стабильным публичным интерфейсом первой версии.
fn write_ast(path: &Path, document: &Document) -> Result<(), AppError> {
    let json = serde_json::to_string_pretty(document).map_err(|error| AppError::Output {
        path: path.to_path_buf(),
        source: std::io::Error::other(error),
    })?;
    fs::write(path, json + "\n").map_err(|source| AppError::Output {
        path: path.to_path_buf(),
        source,
    })
}
