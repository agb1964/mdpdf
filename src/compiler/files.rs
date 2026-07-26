//! Разрешение локальных ресурсов до запуска Typst (ТЗ §33).
//!
//! Обслуживаются только явно зарегистрированные файлы. Выход за каталог
//! исходного Markdown, абсолютные пути и симлинки наружу запрещены; формат
//! определяется по содержимому, а не по расширению.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::compiler::error::CompileError;
use crate::compiler::limits;
use crate::typst_gen::generator::ResourceReference;

/// Формат изображения, определённый по содержимому (ТЗ §33.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// GIF.
    Gif,
    /// SVG без внешних ресурсов.
    Svg,
}

impl ImageFormat {
    /// Имя формата для сообщений.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::Svg => "SVG",
        }
    }
}

/// Набор ресурсов, разрешённых компилятору.
#[derive(Debug, Default)]
pub struct ResolvedResources {
    /// Виртуальный путь → байты файла. `BTreeMap` ради детерминированного обхода.
    files: BTreeMap<String, Vec<u8>>,
}

impl ResolvedResources {
    /// Байты по виртуальному пути.
    #[must_use]
    pub fn get(&self, logical_path: &str) -> Option<&[u8]> {
        self.files.get(logical_path).map(Vec::as_slice)
    }

    /// Пары «виртуальный путь → байты» в детерминированном порядке.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
    }

    /// Количество зарегистрированных файлов.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Нет ли ни одного ресурса.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Читает и проверяет все ресурсы документа (ТЗ §33.1).
///
/// # Errors
///
/// [`CompileError`], если файл не найден, лежит за пределами каталога
/// документа, слишком велик, не является обычным файлом или имеет
/// неподдерживаемый формат.
pub fn resolve_resources(
    resources: &[ResourceReference],
    base_dir: &Path,
    document: &str,
) -> Result<ResolvedResources, CompileError> {
    if resources.len() > limits::MAX_IMAGES {
        return Err(CompileError::LimitExceeded {
            message: format!(
                "document has {} images, limit is {}",
                resources.len(),
                limits::MAX_IMAGES
            ),
        });
    }

    let root = canonical_root(base_dir, document)?;
    let mut resolved = ResolvedResources::default();
    let mut total_bytes: usize = 0;

    for resource in resources {
        let bytes = read_resource(resource, &root)?;
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > limits::MAX_TOTAL_IMAGE_BYTES {
            return Err(CompileError::LimitExceeded {
                message: format!(
                    "images total more than {} bytes",
                    limits::MAX_TOTAL_IMAGE_BYTES
                ),
            });
        }

        // Формат проверяется до вызова Typst, чтобы ошибка была понятной (ТЗ §33.3).
        detect_format(&bytes).ok_or_else(|| CompileError::Image {
            path: resource.source_path.clone(),
            span: resource.span,
            message: "unsupported image format: expected PNG, JPEG, GIF or SVG".to_owned(),
        })?;

        resolved.files.insert(resource.logical_path.clone(), bytes);
    }

    Ok(resolved)
}

/// Ошибка каталога не привязана к позиции в Markdown, поэтому имя документа
/// попадает прямо в сообщение — иначе пользователь не поймёт, что не собралось
/// (ТЗ §31: `source_name` существует ровно для этого).
fn canonical_root(base_dir: &Path, document: &str) -> Result<PathBuf, CompileError> {
    fs::canonicalize(base_dir).map_err(|error| CompileError::ResourceAccess {
        path: base_dir.display().to_string(),
        span: None,
        message: format!("base directory of {document} is not accessible: {error}"),
    })
}

/// Читает один файл с проверками ТЗ §33.1 и §33.2.
fn read_resource(resource: &ResourceReference, root: &Path) -> Result<Vec<u8>, CompileError> {
    let source_path = resource.source_path.as_str();
    let relative = Path::new(source_path);
    let access = |message: &str| CompileError::ResourceAccess {
        path: source_path.to_owned(),
        span: resource.span,
        message: message.to_owned(),
    };
    let image = |message: String| CompileError::Image {
        path: source_path.to_owned(),
        span: resource.span,
        message,
    };

    if relative.is_absolute() {
        return Err(access("absolute image paths are not allowed"));
    }
    if relative
        .components()
        .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return Err(access("image path must stay inside the document directory"));
    }

    let candidate = root.join(relative);
    // canonicalize раскрывает `..` и симлинки — только после этого можно
    // проверять принадлежность корню (ТЗ §33.2).
    let resolved = fs::canonicalize(&candidate)
        .map_err(|error| image(format!("cannot read file: {error}")))?;

    if !resolved.starts_with(root) {
        return Err(access("image resolves outside the document directory"));
    }

    let metadata =
        fs::metadata(&resolved).map_err(|error| image(format!("cannot read metadata: {error}")))?;
    if !metadata.is_file() {
        return Err(image("not a regular file".to_owned()));
    }
    if metadata.len() > limits::MAX_IMAGE_BYTES as u64 {
        return Err(CompileError::LimitExceeded {
            message: format!(
                "image {source_path} is {} bytes, limit is {}",
                metadata.len(),
                limits::MAX_IMAGE_BYTES
            ),
        });
    }

    fs::read(&resolved).map_err(|error| image(format!("cannot read file: {error}")))
}

/// Определение формата по содержимому, а не по расширению (ТЗ §33.3).
#[must_use]
pub fn detect_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(ImageFormat::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageFormat::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImageFormat::Gif);
    }
    if looks_like_svg(bytes) {
        return Some(ImageFormat::Svg);
    }
    None
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let Ok(text) = std::str::from_utf8(head) else {
        return false;
    };
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceSpan;
    use crate::typst_gen::generator::ResourceKind;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n----------------";

    fn reference(source: &str, logical: &str) -> ResourceReference {
        ResourceReference {
            logical_path: logical.to_owned(),
            source_path: source.to_owned(),
            kind: ResourceKind::Image,
            span: Some(SourceSpan::new(0, 1)),
        }
    }

    #[test]
    fn formats_are_detected_by_content() {
        assert_eq!(detect_format(PNG), Some(ImageFormat::Png));
        assert_eq!(
            detect_format(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(detect_format(b"GIF89a..."), Some(ImageFormat::Gif));
        assert_eq!(
            detect_format(b"<svg xmlns=\"\"></svg>"),
            Some(ImageFormat::Svg)
        );
        assert_eq!(
            detect_format(b"<?xml version=\"1.0\"?><svg></svg>"),
            Some(ImageFormat::Svg)
        );
        assert_eq!(detect_format(b"not an image"), None);
        assert_eq!(detect_format(b""), None);
    }

    #[test]
    fn format_names_are_reported() {
        assert_eq!(ImageFormat::Png.name(), "PNG");
        assert_eq!(ImageFormat::Jpeg.name(), "JPEG");
        assert_eq!(ImageFormat::Gif.name(), "GIF");
        assert_eq!(ImageFormat::Svg.name(), "SVG");
    }

    #[test]
    fn a_local_image_is_read() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("a.png"), PNG).expect("write image");

        let resolved = resolve_resources(
            &[reference("a.png", "/mdpdf-resources/000001.png")],
            dir.path(),
            "doc.md",
        )
        .expect("image resolves");

        assert_eq!(resolved.len(), 1);
        assert!(!resolved.is_empty());
        assert_eq!(resolved.get("/mdpdf-resources/000001.png"), Some(PNG));
        assert_eq!(resolved.get("/mdpdf-resources/000002.png"), None);
    }

    #[test]
    fn a_subdirectory_is_allowed() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("images")).expect("create dir");
        std::fs::write(dir.path().join("images/a.png"), PNG).expect("write image");

        let resolved = resolve_resources(
            &[reference("images/a.png", "/mdpdf-resources/000001.png")],
            dir.path(),
            "doc.md",
        )
        .expect("image resolves");
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn path_traversal_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let inner = dir.path().join("doc");
        std::fs::create_dir(&inner).expect("create dir");
        std::fs::write(dir.path().join("secret.png"), PNG).expect("write outside");

        let err = resolve_resources(
            &[reference("../secret.png", "/mdpdf-resources/000001.png")],
            &inner,
            "doc.md",
        )
        .expect_err("traversal must be rejected");
        assert!(matches!(err, CompileError::ResourceAccess { .. }));
    }

    #[test]
    fn absolute_paths_are_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let absolute = dir.path().join("a.png");
        std::fs::write(&absolute, PNG).expect("write image");

        let err = resolve_resources(
            &[reference(
                &absolute.display().to_string(),
                "/mdpdf-resources/000001.png",
            )],
            dir.path(),
            "doc.md",
        )
        .expect_err("absolute path must be rejected");
        assert!(matches!(err, CompileError::ResourceAccess { .. }));
    }

    #[test]
    fn a_missing_file_is_reported_as_an_image_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = resolve_resources(
            &[reference("nope.png", "/mdpdf-resources/000001.png")],
            dir.path(),
            "doc.md",
        )
        .expect_err("missing file");
        assert!(matches!(err, CompileError::Image { .. }));
    }

    #[test]
    fn a_directory_is_not_an_image() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("a.png")).expect("create dir");
        let err = resolve_resources(
            &[reference("a.png", "/mdpdf-resources/000001.png")],
            dir.path(),
            "doc.md",
        )
        .expect_err("directory is not a file");
        assert!(matches!(err, CompileError::Image { .. }));
    }

    #[test]
    fn an_unsupported_format_is_rejected_before_typst() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("a.png"), b"definitely not an image").expect("write");
        let err = resolve_resources(
            &[reference("a.png", "/mdpdf-resources/000001.png")],
            dir.path(),
            "doc.md",
        )
        .expect_err("format must be rejected");
        assert!(matches!(err, CompileError::Image { .. }));
    }

    #[test]
    fn too_many_images_are_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let many: Vec<ResourceReference> = (0..=limits::MAX_IMAGES)
            .map(|index| reference("a.png", &format!("/mdpdf-resources/{index:06}.png")))
            .collect();
        let err = resolve_resources(&many, dir.path(), "doc.md").expect_err("too many images");
        assert!(matches!(err, CompileError::LimitExceeded { .. }));
    }

    #[test]
    fn an_unreadable_base_directory_is_reported() {
        let err = resolve_resources(
            &[reference("a.png", "/mdpdf-resources/000001.png")],
            Path::new("/definitely/missing/directory"),
            "doc.md",
        )
        .expect_err("missing base dir");
        assert!(matches!(err, CompileError::ResourceAccess { .. }));
    }
}
