//! Разбор аргументов командной строки (ТЗ §5).
//!
//! Модуль описывает только поверхность CLI. Преобразование в типизированную
//! конфигурацию конвейера выполняется в [`crate::config`].

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Markdown → PDF, один бинарный файл, без сети и внешних процессов.
#[derive(Debug, Clone, Parser)]
#[command(name = "mdpdf", version, about, long_about = None)]
pub struct Cli {
    /// Markdown-файл или "-" для stdin
    pub input: String,

    /// Выходной PDF
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Переопределить заголовок документа
    #[arg(long, value_name = "TEXT")]
    pub title: Option<String>,

    /// Указать автора
    #[arg(long, value_name = "TEXT")]
    pub author: Option<String>,

    /// Размер страницы
    #[arg(long, value_name = "PAPER", default_value = "a4")]
    pub paper: PaperArg,

    /// Поля страницы
    #[arg(long, value_name = "LENGTH", default_value = "20mm")]
    pub margin: String,

    /// Основной размер текста
    #[arg(long = "font-size", value_name = "LENGTH", default_value = "11pt")]
    pub font_size: String,

    /// Создать оглавление
    #[arg(long)]
    pub toc: bool,

    /// Нумеровать заголовки
    #[arg(long = "heading-numbers")]
    pub heading_numbers: bool,

    /// Проверить документ без записи PDF
    #[arg(long)]
    pub check: bool,

    /// Записать AST в JSON
    #[arg(long = "emit-ast", value_name = "FILE")]
    pub emit_ast: Option<PathBuf>,

    /// Записать сгенерированный Typst
    #[arg(long = "emit-typst", value_name = "FILE")]
    pub emit_typst: Option<PathBuf>,

    /// Разрешить замену выходного файла
    #[arg(long)]
    pub overwrite: bool,

    /// Не выводить сообщения об успехе
    #[arg(long)]
    pub quiet: bool,

    /// Расширенная диагностика
    #[arg(long)]
    pub verbose: bool,
}

/// Размер страницы, принимаемый на вход CLI (ТЗ §20.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PaperArg {
    /// 210 × 297 мм.
    A4,
    /// 8.5 × 11 дюймов.
    Letter,
}

impl Cli {
    /// Читается ли документ из stdin.
    #[must_use]
    pub fn reads_stdin(&self) -> bool {
        self.input == "-"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_specification() {
        let cli = Cli::parse_from(["mdpdf", "input.md"]);
        assert_eq!(cli.paper, PaperArg::A4);
        assert_eq!(cli.margin, "20mm");
        assert_eq!(cli.font_size, "11pt");
        assert!(!cli.toc);
        assert!(!cli.heading_numbers);
        assert!(!cli.check);
        assert!(!cli.overwrite);
        assert!(!cli.quiet);
        assert!(!cli.verbose);
        assert!(cli.output.is_none());
    }

    #[test]
    fn dash_input_means_stdin() {
        let cli = Cli::parse_from(["mdpdf", "-", "--output", "out.pdf"]);
        assert!(cli.reads_stdin());
        assert_eq!(cli.output, Some(PathBuf::from("out.pdf")));
    }

    #[test]
    fn short_output_flag_is_accepted() {
        let cli = Cli::parse_from(["mdpdf", "input.md", "-o", "out.pdf"]);
        assert_eq!(cli.output, Some(PathBuf::from("out.pdf")));
    }

    #[test]
    fn missing_input_is_an_error() {
        assert!(Cli::try_parse_from(["mdpdf"]).is_err());
    }

    #[test]
    fn unknown_paper_is_rejected() {
        assert!(Cli::try_parse_from(["mdpdf", "in.md", "--paper", "a3"]).is_err());
    }
}
