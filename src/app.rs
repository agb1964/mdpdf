//! Конвейер приложения (ТЗ §42).
//!
//! Порядок шагов зафиксирован: чтение → парсинг → валидация → генерация Typst →
//! (диагностический вывод) → компиляция → атомарная запись PDF.

use std::fs;
use std::path::Path;

use crate::ast::SourceSpan;
use crate::ast::document::Document;
use crate::ast::metadata::DocumentMetadata;
use crate::cli::Cli;
use crate::compiler::error::CompileError;
use crate::compiler::fonts;
use crate::compiler::{CompileInput, EmbeddedTypstCompiler};
use crate::config::AppConfig;
use crate::error::{AppError, ExitStatus};
use crate::markdown::error::MarkdownError;
use crate::markdown::parser::MarkdownParser;
use crate::output;
use crate::source::{self, SourceDocument};
use crate::typst_gen::generator::TypstGenerator;

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

    warn_about_missing_glyphs(&document, &config)?;

    let generated = TypstGenerator::new(config.render_options()?).generate(&ast)?;

    // Нефатальные предупреждения генерации (деградировавшие mermaid-диаграммы,
    // ТЗ §10.5) не теряются; PDF всё равно создаётся.
    for warning in &generated.warnings {
        eprintln!("{}: warning: {warning}", document.name);
    }

    if config.verbose {
        eprintln!(
            "mdpdf: generated {} bytes of Typst, {} local resources",
            generated.source.len(),
            generated.resources.len()
        );
    }

    if let Some(path) = &config.emit_typst {
        write_text(path, &generated.source)?;
        if !config.quiet {
            println!("Created {}", path.display());
        }
    }

    // Диагностический режим: запрошен только --emit-*, выходной PDF не задан,
    // поэтому конвейер на этом заканчивается (ТЗ §5.5, §5.6).
    // `--check`, наоборот, обязан пройти весь конвейер вплоть до компиляции
    // и лишь не записывать PDF (ТЗ §5.4), поэтому сюда не попадает.
    if !config.check && config.output.is_none() {
        return Ok(ExitStatus::Success);
    }

    let compiled = EmbeddedTypstCompiler::new()
        .compile_document(CompileInput {
            typst_source: &generated.source,
            source_name: &document.name,
            base_dir: &config.base_dir(),
            resources: &generated.resources,
        })
        .map_err(|error| {
            // Диагностики Typst показываются пользователю построчно (ТЗ §37),
            // а не прячутся за общим «PDF compilation failed».
            if let CompileError::Typst { diagnostics } = &error {
                for diagnostic in diagnostics {
                    eprintln!("{}", diagnostic.render());
                }
            }
            // Если ошибку удалось привязать к месту в Markdown, показывается
            // именно оно, а не позиция в сгенерированном Typst (ТЗ §37).
            match error.markdown_span() {
                Some(span) => {
                    let (line, column) = span.line_column(&document.text);
                    AppError::CompileAt {
                        location: format!("{}:{line}:{column}", document.name),
                        source: Box::new(error),
                    }
                }
                None => AppError::Compile(Box::new(error)),
            }
        })?;

    // Предупреждения Typst не теряются и уходят в stderr; PDF всё равно
    // создаётся (ТЗ §38).
    for warning in &compiled.warnings {
        eprintln!("{}", warning.render());
    }

    if config.verbose {
        eprintln!("mdpdf: produced {} bytes of PDF", compiled.bytes.len());
    }

    // Отсутствие пути здесь означает `--check`: режим «только --emit-*» вышел
    // раньше, а во всех остальных случаях путь вычислен в AppConfig (ТЗ §5.1).
    match &config.output {
        Some(path) => {
            output::write_pdf_atomically(path, &compiled.bytes, config.overwrite)?;
            if !config.quiet {
                println!("Created {}", path.display());
            }
        }
        None => {
            if !config.quiet {
                println!("Checked {}", document.name);
            }
        }
    }

    Ok(ExitStatus::Success)
}

/// Предупреждает о символах, для которых нет глифа ни в одном встроенном
/// шрифте (ТЗ §38).
///
/// Такой символ не превращается в замещающий прямоугольник — он просто
/// исчезает, и без предупреждения пользователь узнаёт о потере, только сверив
/// PDF с исходником. PDF при этом создаётся: решение, приемлема ли потеря,
/// принимает человек.
fn warn_about_missing_glyphs(
    document: &SourceDocument,
    config: &AppConfig,
) -> Result<(), AppError> {
    let uncovered = fonts::uncovered_characters(&document.text)?;
    if uncovered.is_empty() {
        return Ok(());
    }

    let total: usize = uncovered.iter().map(|(_, count)| count).sum();
    // В строку попадает не более десяти видов символов: при поломанной
    // кодировке их могут быть сотни, и вывод стал бы нечитаемым.
    let shown: Vec<String> = uncovered
        .iter()
        .take(10)
        .map(|(character, count)| format!("{character} (U+{:04X}) — {count}", *character as u32))
        .collect();
    let tail = if uncovered.len() > shown.len() {
        format!(", ещё {} видов", uncovered.len() - shown.len())
    } else {
        String::new()
    };

    eprintln!(
        "{}: warning: {total} character(s) have no glyph in the embedded fonts \
         and will be missing from the PDF: {}{tail}",
        document.name,
        shown.join(", ")
    );

    if config.verbose {
        eprintln!(
            "mdpdf: подставить системный шрифт нельзя — окружение компиляции \
             намеренно закрыто (ТЗ §32)"
        );
    }
    Ok(())
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
        .map_err(|error| markdown_error(error, document))
}

/// Переводит ошибку парсера в ошибку приложения, сохраняя позицию и правильный
/// код завершения.
///
/// Ошибка валидации AST разворачивается из [`MarkdownError::AstValidation`]:
/// у неё собственный код 5, а не общий код Markdown 4 (ТЗ §43).
fn markdown_error(error: MarkdownError, document: &SourceDocument) -> AppError {
    let location = |span: SourceSpan| {
        let (line, column) = span.line_column(&document.text);
        format!("{}:{line}:{column}", document.name)
    };

    match error {
        MarkdownError::AstValidation(error) => AppError::AstValidationAt {
            location: location(error.span()),
            source: error,
        },
        error => match error.span() {
            Some(span) => AppError::MarkdownAt {
                location: location(span),
                source: error,
            },
            None => AppError::Markdown(error),
        },
    }
}

/// Записывает AST в JSON (ТЗ §5.6).
///
/// Формат не является стабильным публичным интерфейсом первой версии.
fn write_ast(path: &Path, document: &Document) -> Result<(), AppError> {
    let json = serde_json::to_string_pretty(document).map_err(|error| AppError::Output {
        path: path.to_path_buf(),
        source: std::io::Error::other(error),
    })?;
    write_text(path, &(json + "\n"))
}

/// Записывает диагностический текстовый файл (ТЗ §5.5, §5.6).
fn write_text(path: &Path, contents: &str) -> Result<(), AppError> {
    fs::write(path, contents).map_err(|source| AppError::Output {
        path: path.to_path_buf(),
        source,
    })
}
