//! Метаданные документа (ТЗ §10.2).
//!
//! В первой версии заполняются только из CLI: извлечение YAML front matter
//! не реализуется, сам блок отбрасывается парсером.

use serde::{Deserialize, Serialize};

/// Метаданные документа.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Заголовок документа.
    pub title: Option<String>,
    /// Автор.
    pub author: Option<String>,
    /// Язык документа.
    pub language: Option<String>,
}
