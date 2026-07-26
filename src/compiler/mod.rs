//! Этап 3: Typst source → PDF (ТЗ §30–§41).
//!
//! Это **единственный** модуль проекта, которому разрешено импортировать
//! `typst::*`, `typst_layout::*` и `typst_pdf::*` (ТЗ §31). Внешний Typst CLI
//! не вызывается, сеть и системные шрифты не используются.

pub mod diagnostics;
pub mod error;
pub mod files;
pub mod fonts;
pub mod pdf;
pub mod world;

use std::path::Path;

use crate::compiler::error::CompileError;
use crate::compiler::pdf::CompiledPdf;
use crate::compiler::world::MdpdfWorld;
use crate::typst_gen::generator::ResourceReference;

/// Вход компилятора (ТЗ §31).
#[derive(Debug, Clone, Copy)]
pub struct CompileInput<'a> {
    /// Сгенерированный Typst-исходник.
    pub typst_source: &'a str,
    /// Имя исходного документа для сообщений, которые невозможно привязать
    /// к позиции: например, недоступный каталог ресурсов.
    ///
    /// В диагностики Typst не попадает намеренно. Позиции внутри
    /// сгенерированного кода помечаются как
    /// [`diagnostics::GENERATED_TYPST`], потому что подставить туда имя `.md`
    /// значило бы указать пользователю на строку, которой в его файле нет
    /// (ТЗ §37). Ошибки, привязываемые к Markdown, несут
    /// [`CompileError::markdown_span`], а префикс `файл:строка:столбец`
    /// строит вызывающая сторона, у которой есть исходный текст.
    pub source_name: &'a str,
    /// Каталог, относительно которого разрешаются ресурсы.
    pub base_dir: &'a Path,
    /// Ресурсы, которые разрешено предоставить документу.
    pub resources: &'a [ResourceReference],
}

/// Компилятор PDF (ТЗ §31).
pub trait PdfCompiler {
    /// Компилирует документ в байты PDF.
    ///
    /// # Errors
    ///
    /// [`CompileError`] при недоступном ресурсе, ошибке Typst или некорректном
    /// выводе экспортера.
    fn compile(&self, input: CompileInput<'_>) -> Result<Vec<u8>, CompileError>;
}

/// Реализация на встроенных библиотеках Typst.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddedTypstCompiler;

impl EmbeddedTypstCompiler {
    /// Создаёт компилятор.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Компилирует документ, сохраняя предупреждения (ТЗ §38).
    ///
    /// Предупреждения нельзя терять, а сигнатура [`PdfCompiler::compile`]
    /// закреплена ТЗ и возвращает только байты, поэтому полный результат
    /// доступен через этот метод.
    ///
    /// # Errors
    ///
    /// [`CompileError`] при недоступном ресурсе, ошибке Typst или некорректном
    /// выводе экспортера.
    pub fn compile_document(&self, input: CompileInput<'_>) -> Result<CompiledPdf, CompileError> {
        // Последовательность ТЗ §35: разрешить ресурсы, собрать ограниченный
        // World, скомпилировать, экспортировать, проверить сигнатуру.
        let resources =
            files::resolve_resources(input.resources, input.base_dir, input.source_name)?;
        let world = MdpdfWorld::new(input.typst_source, &resources)?;
        pdf::compile(&world)
    }
}

impl PdfCompiler for EmbeddedTypstCompiler {
    fn compile(&self, input: CompileInput<'_>) -> Result<Vec<u8>, CompileError> {
        self.compile_document(input).map(|compiled| compiled.bytes)
    }
}

/// Лимиты ресурсов первой версии (ТЗ §40).
pub mod limits {
    /// Максимальное число узлов AST.
    ///
    /// Источник истины — `ast::limits`: лимит соблюдает парсер, компилятору
    /// он нужен только для справки.
    pub const MAX_AST_NODES: usize = crate::ast::limits::MAX_AST_NODES;
    /// Максимальная глубина вложенности.
    pub const MAX_NESTING_DEPTH: usize = crate::ast::limits::MAX_NESTING_DEPTH;
    /// Максимальное число изображений в документе.
    pub const MAX_IMAGES: usize = 1_000;
    /// Максимальный размер одного изображения, байты.
    pub const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
    /// Максимальный суммарный размер изображений, байты.
    pub const MAX_TOTAL_IMAGE_BYTES: usize = 256 * 1024 * 1024;
    /// Максимальная длина URL, байты.
    ///
    /// Переиспользует константу из `typst_gen::escape`, а не дублирует
    /// значение: `compiler` уже импортирует `typst_gen` (ТЗ §31), а обратной
    /// зависимости быть не должно, поэтому источник истины — там.
    pub const MAX_URL_BYTES: usize = crate::typst_gen::escape::MAX_URL_BYTES;
    /// Максимальная длина одного текстового узла, байты.
    pub const MAX_TEXT_NODE_BYTES: usize = 16 * 1024 * 1024;
}
