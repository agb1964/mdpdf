//! Ограниченное окружение Typst `World` (ТЗ §32).
//!
//! Предоставляет ровно четыре вещи: основной исходник `/main.typ`, встроенные
//! шрифты и заранее разрешённые изображения. Всё остальное — файловая система,
//! домашний каталог, переменные окружения, сеть, пакеты Typst, системные шрифты
//! и текущее время — недоступно.

use std::collections::HashMap;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::FontBook;
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};

use crate::compiler::error::CompileError;
use crate::compiler::files::ResolvedResources;
use crate::compiler::fonts::{self, EmbeddedFontSet};

/// Виртуальный путь основного исходника (ТЗ §33).
pub const MAIN_PATH: &str = "/main.typ";

/// Ограниченное окружение компиляции.
pub struct MdpdfWorld {
    library: LazyHash<Library>,
    fonts: &'static EmbeddedFontSet,
    main_id: FileId,
    main: Source,
    files: HashMap<FileId, Bytes>,
}

impl std::fmt::Debug for MdpdfWorld {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MdpdfWorld")
            .field("files", &self.files.len())
            .finish_non_exhaustive()
    }
}

impl MdpdfWorld {
    /// Собирает окружение из готового исходника и разрешённых ресурсов.
    ///
    /// # Errors
    ///
    /// [`CompileError`], если встроенные шрифты не разбираются или виртуальный
    /// путь недопустим.
    pub fn new(source: &str, resources: &ResolvedResources) -> Result<Self, CompileError> {
        let main_id = virtual_file_id(MAIN_PATH)?;

        // Байты переводятся в `Bytes` один раз: Typst обращается к файлу
        // многократно, а `Bytes` клонируется дёшево.
        let mut files = HashMap::new();
        for (logical_path, bytes) in resources.iter() {
            files.insert(virtual_file_id(logical_path)?, Bytes::new(bytes.to_vec()));
        }

        Ok(Self {
            library: LazyHash::new(Library::default()),
            fonts: fonts::embedded_fonts()?,
            main_id,
            main: Source::new(main_id, source.to_owned()),
            files,
        })
    }

    /// Виртуальный путь запрошенного файла в человекочитаемом виде.
    fn logical_path(id: FileId) -> String {
        id.vpath().get_with_slash().to_string()
    }

    /// Строка и столбец байтового смещения в `/main.typ`, считая с единицы.
    ///
    /// Нужна для диагностик: позиция в сгенерированном Typst (ТЗ §37).
    #[must_use]
    pub fn line_column(&self, offset: usize) -> Option<(usize, usize)> {
        let (line, column) = self.main.lines().byte_to_line_column(offset)?;
        Some((line + 1, column + 1))
    }
}

/// Строит `FileId` для виртуального пути внутри проекта.
fn virtual_file_id(path: &str) -> Result<FileId, CompileError> {
    let vpath = VirtualPath::new(path).map_err(|error| CompileError::InternalInvariant {
        message: format!("invalid virtual path {path}: {error}"),
    })?;
    Ok(RootedPath::new(VirtualRoot::Project, vpath).intern())
}

impl typst::World for MdpdfWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            return Ok(self.main.clone());
        }
        // Никаких `import`/`include` из документа: единственный исходник — main.
        Err(FileError::NotFound(Self::logical_path(id).into()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(Self::logical_path(id).into()))
    }

    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.fonts.font(index)
    }

    /// Время недоступно: вывод обязан быть детерминированным (ТЗ §25, §32).
    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use typst::World;

    use super::*;

    fn world(source: &str) -> MdpdfWorld {
        MdpdfWorld::new(source, &ResolvedResources::default()).expect("world builds")
    }

    #[test]
    fn main_source_is_available() {
        let world = world("#let a = 1\n");
        let main = world.main();
        assert_eq!(
            world.source(main).expect("main source").text(),
            "#let a = 1\n"
        );
    }

    #[test]
    fn no_other_source_file_exists() {
        let world = world("");
        let other = virtual_file_id("/template.typ").expect("valid path");
        assert!(world.source(other).is_err());
    }

    #[test]
    fn unknown_files_are_not_served() {
        let world = world("");
        let unknown = virtual_file_id("/mdpdf-resources/000001.png").expect("valid path");
        assert!(world.file(unknown).is_err());
    }

    #[test]
    fn registered_resources_are_served() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("a.png"), b"\x89PNG\r\n\x1a\n1234").expect("write");

        let resources = crate::compiler::files::resolve_resources(
            &[crate::typst_gen::generator::ResourceReference {
                logical_path: "/mdpdf-resources/000001.png".to_owned(),
                source_path: "a.png".to_owned(),
                kind: crate::typst_gen::generator::ResourceKind::Image,
                span: None,
            }],
            dir.path(),
            "doc.md",
        )
        .expect("resources resolve");

        let world = MdpdfWorld::new("", &resources).expect("world builds");
        let id = virtual_file_id("/mdpdf-resources/000001.png").expect("valid path");
        assert!(world.file(id).is_ok());
    }

    #[test]
    fn time_is_not_available() {
        let world = world("");
        assert!(world.today(None).is_none());
        // Смещение игнорируется: источника времени в окружении нет вовсе.
    }

    #[test]
    fn every_embedded_font_is_reachable() {
        let world = world("");
        assert!(world.font(0).is_some());
        assert!(world.font(4).is_some());
        assert!(world.font(5).is_none());
        assert!(world.book().families().count() > 0);
    }
}
