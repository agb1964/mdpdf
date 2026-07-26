//! Преобразование аргументов CLI в конфигурацию конвейера.
//!
//! Здесь выполняются проверки, которые `clap` выразить не может (ТЗ §5.3),
//! и вычисляется путь выходного файла (ТЗ §5.1).

use std::path::PathBuf;

use crate::cli::{Cli, PaperArg};
use crate::error::AppError;
use crate::output;
use crate::typst_gen::generator::{Length, RenderOptions};

/// Источник входного документа.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSource {
    /// Локальный Markdown-файл.
    File(PathBuf),
    /// Стандартный ввод.
    Stdin,
}

/// Конфигурация одного запуска конвейера.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Откуда читается Markdown.
    pub input: InputSource,
    /// Куда записывается PDF. `None` в режимах `--check` и «только `--emit-*`».
    pub output: Option<PathBuf>,
    /// Заголовок документа из CLI (ТЗ §10.2).
    pub title: Option<String>,
    /// Автор документа из CLI.
    pub author: Option<String>,
    /// Размер страницы.
    pub paper: String,
    /// Поля страницы.
    pub margin: String,
    /// Основной размер текста.
    pub font_size: String,
    /// Строить оглавление.
    pub toc: bool,
    /// Нумеровать заголовки.
    pub heading_numbers: bool,
    /// Проверка без записи PDF.
    pub check: bool,
    /// Куда записать JSON с AST.
    pub emit_ast: Option<PathBuf>,
    /// Куда записать сгенерированный Typst.
    pub emit_typst: Option<PathBuf>,
    /// Разрешена ли перезапись выходного файла.
    pub overwrite: bool,
    /// Подавить сообщение об успехе.
    pub quiet: bool,
    /// Расширенная диагностика.
    pub verbose: bool,
}

/// Строковое представление размера страницы для [`PaperSize::from_str`].
///
/// Явный `match` вместо `format!("{:?}", ..)` не зависит от имени варианта
/// enum `Debug`-представления и не ломается при его переименовании.
fn paper_arg_to_str(paper: PaperArg) -> &'static str {
    match paper {
        PaperArg::A4 => "a4",
        PaperArg::Letter => "letter",
    }
}

impl AppConfig {
    /// Собирает конфигурацию из аргументов CLI.
    ///
    /// # Errors
    ///
    /// Возвращает [`AppError::Cli`], если при чтении из stdin не задан `--output`
    /// (ТЗ §5.3).
    pub fn from_cli(args: Cli) -> Result<Self, AppError> {
        let input = if args.reads_stdin() {
            InputSource::Stdin
        } else {
            InputSource::File(PathBuf::from(&args.input))
        };

        // PDF не записывается в --check и в режиме, где запрошен только --emit-*
        // без явного --output (ТЗ §5.4, §5.5).
        let emit_only =
            args.output.is_none() && (args.emit_typst.is_some() || args.emit_ast.is_some());

        let output = if args.check || emit_only {
            None
        } else {
            match (&args.output, &input) {
                (Some(path), _) => Some(path.clone()),
                (None, InputSource::File(path)) => Some(output::default_output_path(path)),
                (None, InputSource::Stdin) => {
                    return Err(AppError::Cli {
                        message: "--output is required when reading from stdin".to_owned(),
                    });
                }
            }
        };

        Ok(Self {
            input,
            output,
            title: args.title,
            author: args.author,
            paper: paper_arg_to_str(args.paper).to_owned(),
            margin: args.margin,
            font_size: args.font_size,
            toc: args.toc,
            heading_numbers: args.heading_numbers,
            check: args.check,
            emit_ast: args.emit_ast,
            emit_typst: args.emit_typst,
            overwrite: args.overwrite,
            quiet: args.quiet,
            verbose: args.verbose,
        })
    }

    /// Собирает типизированные параметры рендеринга (ТЗ §20).
    ///
    /// # Errors
    ///
    /// [`AppError::TypstGeneration`], если размер страницы, поля или размер
    /// текста заданы недопустимо.
    pub fn render_options(&self) -> Result<RenderOptions, AppError> {
        let options = RenderOptions {
            paper: self.paper.parse()?,
            margin: Length::parse(&self.margin, "margin")?,
            font_size: Length::parse(&self.font_size, "font-size")?,
            title: self.title.clone(),
            author: self.author.clone(),
            toc: self.toc,
            heading_numbers: self.heading_numbers,
        };
        options.validate()?;
        Ok(options)
    }

    /// Каталог, относительно которого разрешаются пути изображений (ТЗ §6.3).
    #[must_use]
    pub fn base_dir(&self) -> PathBuf {
        match &self.input {
            InputSource::File(path) => path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), PathBuf::from),
            InputSource::Stdin => PathBuf::from("."),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn config_from(args: &[&str]) -> Result<AppConfig, AppError> {
        AppConfig::from_cli(Cli::parse_from(args))
    }

    #[test]
    fn output_defaults_next_to_input() {
        let config = config_from(&["mdpdf", "docs/input.md"]).expect("config");
        assert_eq!(config.output, Some(PathBuf::from("docs/input.pdf")));
        assert_eq!(config.base_dir(), PathBuf::from("docs"));
    }

    #[test]
    fn stdin_requires_explicit_output() {
        let err = config_from(&["mdpdf", "-"]).expect_err("stdin without --output");
        assert!(matches!(err, AppError::Cli { .. }));
    }

    #[test]
    fn stdin_resolves_paths_against_cwd() {
        let config = config_from(&["mdpdf", "-", "-o", "out.pdf"]).expect("config");
        assert_eq!(config.input, InputSource::Stdin);
        assert_eq!(config.base_dir(), PathBuf::from("."));
    }

    #[test]
    fn check_mode_writes_no_pdf() {
        let config = config_from(&["mdpdf", "input.md", "--check"]).expect("config");
        assert!(config.output.is_none());
    }

    #[test]
    fn emit_typst_alone_writes_no_pdf() {
        let config =
            config_from(&["mdpdf", "input.md", "--emit-typst", "out.typ"]).expect("config");
        assert!(config.output.is_none());
    }

    #[test]
    fn emit_typst_with_output_still_writes_pdf() {
        let config = config_from(&["mdpdf", "in.md", "--emit-typst", "o.typ", "-o", "o.pdf"])
            .expect("config");
        assert_eq!(config.output, Some(PathBuf::from("o.pdf")));
    }
}
