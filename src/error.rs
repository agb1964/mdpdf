//! Коды завершения и корневой тип ошибки приложения (ТЗ §43, §53).
//!
//! В доменных модулях используется `thiserror`, `anyhow` запрещён.
//! `unwrap()`, `expect()` и `panic!` в production-коде запрещены.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

use crate::ast::validate::AstValidationError;
use crate::compiler::error::CompileError;
use crate::markdown::error::MarkdownError;
use crate::typst_gen::error::TypstGenerationError;

/// Код завершения процесса. Значения стабильны и документированы (ТЗ §43).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    /// 0 — успех.
    Success,
    /// 1 — общая ошибка выполнения.
    GeneralError,
    /// 2 — ошибка аргументов CLI.
    CliError,
    /// 3 — ошибка чтения входа.
    InputError,
    /// 4 — ошибка Markdown.
    MarkdownError,
    /// 5 — ошибка AST validation.
    AstValidationError,
    /// 6 — ошибка генерации Typst.
    TypstGenerationError,
    /// 7 — ошибка компиляции Typst.
    CompileError,
    /// 8 — ошибка записи результата.
    OutputError,
    /// 9 — нарушение политики доступа к ресурсу.
    ResourcePolicyError,
}

impl ExitStatus {
    /// Числовое значение кода завершения.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::GeneralError => 1,
            Self::CliError => 2,
            Self::InputError => 3,
            Self::MarkdownError => 4,
            Self::AstValidationError => 5,
            Self::TypstGenerationError => 6,
            Self::CompileError => 7,
            Self::OutputError => 8,
            Self::ResourcePolicyError => 9,
        }
    }
}

/// Корневая ошибка приложения. Каждый вариант однозначно отображается в [`ExitStatus`].
#[derive(Debug, Error)]
pub enum AppError {
    /// Некорректная комбинация аргументов, не отлавливаемая `clap`.
    #[error("invalid arguments: {message}")]
    Cli {
        /// Человекочитаемое описание проблемы.
        message: String,
    },

    /// Ошибка чтения входного документа.
    #[error("cannot read {path}")]
    Input {
        /// Путь входа или `-` для stdin.
        path: String,
        /// Причина.
        #[source]
        source: io::Error,
    },

    /// Вход не является корректным UTF-8 или содержит нулевой байт (ТЗ §13).
    #[error("invalid input: {message}")]
    InvalidInput {
        /// Описание проблемы.
        message: String,
    },

    /// Ошибка этапа 1 — парсинг Markdown.
    #[error(transparent)]
    Markdown(#[from] MarkdownError),

    /// Та же ошибка, но с позицией в исходном файле (ТЗ §16):
    /// `input.md:12:5: unsupported Markdown construct: inline HTML`.
    #[error("{location}: {source}")]
    MarkdownAt {
        /// Префикс `файл:строка:столбец`.
        location: String,
        /// Исходная ошибка.
        #[source]
        source: MarkdownError,
    },

    /// Ошибка валидации AST.
    #[error(transparent)]
    AstValidation(#[from] AstValidationError),

    /// Та же ошибка с позицией в исходном файле (ТЗ §16):
    /// `input.md:3:1: network image source is not allowed`.
    #[error("{location}: {source}")]
    AstValidationAt {
        /// Префикс `файл:строка:столбец`.
        location: String,
        /// Исходная ошибка.
        #[source]
        source: AstValidationError,
    },

    /// Ошибка этапа 2 — генерация Typst.
    #[error(transparent)]
    TypstGeneration(#[from] TypstGenerationError),

    /// Ошибка этапа 3 — компиляция Typst в PDF.
    ///
    /// В боксе: вариант заметно крупнее остальных, а `AppError` возвращается
    /// из каждой функции конвейера.
    #[error(transparent)]
    Compile(Box<CompileError>),

    /// Та же ошибка, но привязанная к позиции в Markdown (ТЗ §37):
    /// `input.md:18:1: image "images/schema.png" could not be loaded`.
    #[error("{location}: {source}")]
    CompileAt {
        /// Префикс `файл:строка:столбец`.
        location: String,
        /// Исходная ошибка.
        #[source]
        source: Box<CompileError>,
    },

    /// Ошибка записи результата.
    #[error("cannot write {path}")]
    Output {
        /// Путь выходного файла.
        path: PathBuf,
        /// Причина.
        #[source]
        source: io::Error,
    },

    /// Выходной файл уже существует, а `--overwrite` не задан (ТЗ §6.2).
    #[error("{path} already exists (use --overwrite to replace it)")]
    OutputExists {
        /// Путь выходного файла.
        path: PathBuf,
    },
}

impl From<CompileError> for AppError {
    fn from(error: CompileError) -> Self {
        Self::Compile(Box::new(error))
    }
}

impl AppError {
    /// Код завершения, соответствующий ошибке (ТЗ §43).
    #[must_use]
    pub const fn exit_status(&self) -> ExitStatus {
        match self {
            Self::Cli { .. } => ExitStatus::CliError,
            Self::Input { .. } | Self::InvalidInput { .. } => ExitStatus::InputError,
            Self::Markdown(_) | Self::MarkdownAt { .. } => ExitStatus::MarkdownError,
            Self::AstValidation(_) | Self::AstValidationAt { .. } => ExitStatus::AstValidationError,
            Self::TypstGeneration(_) => ExitStatus::TypstGenerationError,
            // Нарушение политики доступа к ресурсу имеет собственный код (ТЗ §43).
            Self::Compile(error) | Self::CompileAt { source: error, .. } => match **error {
                CompileError::ResourceAccess { .. } => ExitStatus::ResourcePolicyError,
                _ => ExitStatus::CompileError,
            },
            Self::Output { .. } | Self::OutputExists { .. } => ExitStatus::OutputError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_specification() {
        assert_eq!(ExitStatus::Success.code(), 0);
        assert_eq!(ExitStatus::GeneralError.code(), 1);
        assert_eq!(ExitStatus::CliError.code(), 2);
        assert_eq!(ExitStatus::InputError.code(), 3);
        assert_eq!(ExitStatus::MarkdownError.code(), 4);
        assert_eq!(ExitStatus::AstValidationError.code(), 5);
        assert_eq!(ExitStatus::TypstGenerationError.code(), 6);
        assert_eq!(ExitStatus::CompileError.code(), 7);
        assert_eq!(ExitStatus::OutputError.code(), 8);
        assert_eq!(ExitStatus::ResourcePolicyError.code(), 9);
    }

    #[test]
    fn resource_policy_violations_get_their_own_code() {
        // Код 9 приходит из компилятора: политика доступа к ресурсам живёт там,
        // и отдельного варианта на уровне приложения для этого не нужно.
        let err = AppError::Compile(Box::new(CompileError::ResourceAccess {
            path: "../secret.png".to_owned(),
            span: None,
            message: "outside the document directory".to_owned(),
        }));
        assert_eq!(err.exit_status(), ExitStatus::ResourcePolicyError);
    }
}
