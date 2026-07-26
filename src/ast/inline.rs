//! Inline-элементы документа (ТЗ §10.10–10.12).

use serde::{Deserialize, Serialize};

use crate::ast::Spanned;

/// Inline-элемент.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Inline {
    /// Обычный текст.
    Text(String),
    /// Мягкий перенос строки.
    SoftBreak,
    /// Жёсткий перенос строки.
    HardBreak,
    /// Курсив.
    Emphasis(Vec<Inline>),
    /// Полужирный.
    Strong(Vec<Inline>),
    /// Зачёркнутый.
    Strikethrough(Vec<Inline>),
    /// Inline-код.
    Code(String),
    /// Ссылка.
    Link(Spanned<Link>),
    /// Изображение.
    Image(Spanned<Image>),
}

/// Ссылка (ТЗ §10.11). Не загружается ни на одном этапе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    /// Адрес. Пустое значение допустимо.
    pub destination: String,
    /// Заголовок ссылки.
    pub title: Option<String>,
    /// Текст ссылки, может содержать форматирование.
    pub content: Vec<Inline>,
}

/// Изображение (ТЗ §10.12).
///
/// `source` сохраняется без чтения файла — чтение выполняется только на этапе
/// компиляции.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// Путь к изображению в исходном виде.
    pub source: String,
    /// Заголовок изображения.
    pub title: Option<String>,
    /// Alt-текст, хранится структурированно.
    pub alt: Vec<Inline>,
}
