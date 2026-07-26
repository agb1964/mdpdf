//! Корневой узел документа (ТЗ §10.1).

use serde::{Deserialize, Serialize};

use crate::ast::Spanned;
use crate::ast::block::Block;
use crate::ast::metadata::DocumentMetadata;

/// Документ целиком.
///
/// Блоки хранятся обёрнутыми в [`Spanned`], чтобы у каждого узла верхнего
/// уровня был диагностический диапазон (ТЗ §11).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// Метаданные документа.
    pub metadata: DocumentMetadata,
    /// Блоки верхнего уровня.
    pub blocks: Vec<Spanned<Block>>,
}

impl Document {
    /// Пустой документ. Пустой Markdown — корректный вход (ТЗ §14).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}
