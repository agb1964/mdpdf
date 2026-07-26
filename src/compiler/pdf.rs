//! Компиляция Typst и экспорт в PDF (ТЗ §35, §39).

use typst::diag::SourceDiagnostic;
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

    // `PdfCompiler::compile` (ТЗ §31) закреплена и возвращает только байты, а
    // `CompileError::Typst` несёт один список диагностик — своего поля для
    // предупреждений у него нет и заводить его нельзя, не сломав сопоставление
    // с образцом в src/app.rs (там переменная не помечена как исчерпывающая).
    // Поэтому предупреждения, собранные выше, подмешиваются в тот же список
    // диагностик об ошибке: `Diagnostic::render` уже печатает уровень
    // (`warning`/`error`), так что они остаются различимы, а ТЗ §38 —
    // «предупреждения нельзя терять» — соблюдается и на пути ошибки.
    let document = compiled.output.map_err(|errors| CompileError::Typst {
        diagnostics: merge_diagnostics(world, errors.as_slice(), &warnings),
    })?;

    let bytes = typst_pdf::pdf(&document, &PdfOptions::default()).map_err(|errors| {
        CompileError::Typst {
            diagnostics: merge_diagnostics(world, errors.as_slice(), &warnings),
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

/// Ошибки Typst плюс ранее собранные предупреждения в одном списке
/// диагностик (ТЗ §38, см. комментарий в `compile`).
fn merge_diagnostics(
    world: &MdpdfWorld,
    errors: &[SourceDiagnostic],
    warnings: &[Diagnostic],
) -> Vec<Diagnostic> {
    let mut diagnostics = diagnostics::convert(world, errors);
    diagnostics.extend(warnings.iter().cloned());
    diagnostics
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

    /// ТЗ §38: предупреждения нельзя терять — в том числе когда компиляция
    /// в итоге проваливается. Раньше `warnings`, собранные до вызова
    /// `compiled.output`, в этом случае отбрасывались (code-analysis §5).
    #[test]
    fn warnings_survive_a_failed_compilation() {
        let err = compile_source(
            "#set text(font: \"NonexistentFontXYZ123\")\nтекст\n\n#undefined-name\n",
        )
        .expect_err("undefined variable must fail compilation");
        let CompileError::Typst { diagnostics } = err else {
            panic!("expected Typst diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error
                    && diagnostic.message.contains("undefined-name")),
            "error diagnostic is missing: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Warning
                    && diagnostic.message.contains("unknown font family")),
            "warning was lost on the error path: {diagnostics:?}"
        );
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
