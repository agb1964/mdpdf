//! `mdpdf` — автономный конвертер Markdown → PDF.
//!
//! Конвейер строго разделён на три независимых слоя (ТЗ §7, §61):
//!
//! ```text
//! Markdown  --[markdown]-->  Document AST  --[typst_gen]-->  Typst source  --[compiler]-->  PDF
//! ```
//!
//! * [`markdown`] не знает о Typst;
//! * [`ast`] не содержит Typst-синтаксиса;
//! * [`typst_gen`] не зависит от `pulldown-cmark` и не читает файловую систему;
//! * [`compiler`] не знает о Markdown и является единственным местом, где допустимы
//!   импорты `typst::*` и `typst_pdf::*`.
//!
//! Программа не обращается к сети и не запускает внешние процессы.

#![forbid(unsafe_code)]

pub mod app;
pub mod ast;
pub mod cli;
pub mod compiler;
pub mod config;
pub mod error;
pub mod markdown;
pub mod mermaid;
pub mod output;
pub mod source;
pub mod typst_gen;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::error::ExitStatus;

/// Точка входа бинарного файла.
///
/// Разбирает аргументы, запускает конвейер и превращает результат в код завершения
/// (ТЗ §43). Ошибки печатаются в stderr, сообщение об успехе — в stdout (ТЗ §44).
#[must_use]
pub fn run_cli() -> ExitCode {
    let args = match Cli::try_parse() {
        Ok(args) => args,
        Err(err) => {
            // clap сам решает, куда писать: --help/--version идут в stdout,
            // ошибки разбора аргументов — в stderr.
            let _ = err.print();
            return match err.exit_code() {
                0 => ExitCode::from(ExitStatus::Success.code()),
                _ => ExitCode::from(ExitStatus::CliError.code()),
            };
        }
    };

    let verbose = args.verbose;

    match app::run(args) {
        Ok(status) => ExitCode::from(status.code()),
        Err(err) => {
            // Формат сообщения задан ТЗ §16: `input.md:12:5: описание`.
            eprintln!("{err}");
            if verbose {
                let mut source = std::error::Error::source(&err);
                while let Some(cause) = source {
                    eprintln!("  caused by: {cause}");
                    source = cause.source();
                }
            }
            ExitCode::from(err.exit_status().code())
        }
    }
}
