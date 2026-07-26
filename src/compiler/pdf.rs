//! Компиляция Typst и экспорт в PDF (ТЗ §35, §39).

use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;

use crate::compiler::diagnostics::{self, Diagnostic, Severity};
use crate::compiler::error::CompileError;
use crate::compiler::world::MdpdfWorld;

/// Сигнатура, с которой обязан начинаться корректный PDF.
pub const PDF_MAGIC: &[u8] = b"%PDF-";

/// Минимальный правдоподобный размер PDF, байты.
pub const MIN_PDF_BYTES: usize = 512;

/// Результат компиляции: байты PDF и предупреждения, которые нельзя терять (ТЗ §38).
#[derive(Debug)]
pub struct CompiledPdf {
    /// Байты готового PDF.
    pub bytes: Vec<u8>,
    /// Предупреждения Typst.
    pub warnings: Vec<Diagnostic>,
}

/// Компилирует подготовленный `World` в PDF (ТЗ §35).
///
/// # Errors
///
/// [`CompileError::Typst`] при ошибках компиляции или экспорта,
/// [`CompileError::InvalidPdf`], если экспортер вернул неправдоподобные байты.
pub fn compile(world: &MdpdfWorld) -> Result<CompiledPdf, CompileError> {
    let compiled = typst::compile::<PagedDocument>(world);
    let warnings = diagnostics::convert(world, &compiled.warnings);

    let document = compiled.output.map_err(|errors| CompileError::Typst {
        diagnostics: diagnostics::convert(world, errors.as_slice()),
    })?;

    let bytes = typst_pdf::pdf(&document, &PdfOptions::default()).map_err(|errors| {
        CompileError::Typst {
            diagnostics: diagnostics::convert(world, errors.as_slice()),
        }
    })?;

    if !looks_like_pdf(&bytes) {
        return Err(CompileError::InvalidPdf {
            message: format!(
                "exporter returned {} bytes that do not look like a PDF",
                bytes.len()
            ),
        });
    }

    Ok(CompiledPdf { bytes, warnings })
}

/// Быстрая проверка байтов PDF (ТЗ §39).
///
/// Полный повторный парсинг PDF не выполняется.
#[must_use]
pub fn looks_like_pdf(bytes: &[u8]) -> bool {
    bytes.len() >= MIN_PDF_BYTES && bytes.starts_with(PDF_MAGIC)
}

/// Только ошибки из набора диагностик.
#[must_use]
pub fn errors_only(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::files::ResolvedResources;

    fn compile_source(source: &str) -> Result<CompiledPdf, CompileError> {
        let world = MdpdfWorld::new(source, &ResolvedResources::default()).expect("world builds");
        compile(&world)
    }

    #[test]
    fn empty_bytes_are_rejected() {
        assert!(!looks_like_pdf(b""));
    }

    #[test]
    fn wrong_magic_is_rejected() {
        let bytes = [b'X'; MIN_PDF_BYTES];
        assert!(!looks_like_pdf(&bytes));
    }

    #[test]
    fn too_small_is_rejected() {
        assert!(!looks_like_pdf(b"%PDF-1.7"));
    }

    #[test]
    fn plausible_pdf_is_accepted() {
        let mut bytes = PDF_MAGIC.to_vec();
        bytes.resize(MIN_PDF_BYTES, b'\n');
        assert!(looks_like_pdf(&bytes));
    }

    #[test]
    fn a_minimal_document_compiles_to_a_pdf() {
        let compiled = compile_source("Привет, мир\n").expect("document compiles");
        assert!(looks_like_pdf(&compiled.bytes));
        assert!(compiled.warnings.is_empty());
    }

    #[test]
    fn a_broken_document_reports_diagnostics_without_internal_details() {
        let err = compile_source("#panic(\"boom\")\n").expect_err("panic must fail compilation");
        match err {
            CompileError::Typst { diagnostics } => {
                assert!(!diagnostics.is_empty());
                for diagnostic in &diagnostics {
                    let rendered = diagnostic.render();
                    assert!(!rendered.contains("FileId"), "leaked FileId: {rendered}");
                    assert!(!rendered.contains("Span("), "leaked debug dump: {rendered}");
                }
            }
            other => panic!("expected Typst diagnostics, got {other:?}"),
        }
    }

    #[test]
    fn diagnostics_point_at_the_generated_typst() {
        let err = compile_source("\n\n#undefined-name\n").expect_err("unknown variable");
        let CompileError::Typst { diagnostics } = err else {
            panic!("expected Typst diagnostics");
        };
        let source = diagnostics[0].source.as_ref().expect("position is known");
        assert_eq!(source.file, crate::compiler::diagnostics::GENERATED_TYPST);
        assert_eq!(source.line, Some(3));
    }

    #[test]
    fn errors_are_separated_from_warnings() {
        let diagnostics = vec![
            Diagnostic {
                severity: Severity::Warning,
                message: "w".to_owned(),
                source: None,
                hints: vec![],
            },
            Diagnostic {
                severity: Severity::Error,
                message: "e".to_owned(),
                source: None,
                hints: vec![],
            },
        ];
        let errors = errors_only(&diagnostics);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "e");
    }
}
