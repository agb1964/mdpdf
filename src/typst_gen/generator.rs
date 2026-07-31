//! Генератор Typst: параметры рендеринга и точка входа этапа 2 (ТЗ §19, §20).

use std::fmt;
use std::str::FromStr;

use crate::ast::SourceSpan;
use crate::ast::document::Document;
use crate::typst_gen::error::TypstGenerationError;
use crate::typst_gen::{TEMPLATE, blocks, writer::TypstWriter};

/// Результат генерации (ТЗ §19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedTypst {
    /// Готовый Typst-исходник.
    pub source: String,
    /// Локальные ресурсы, которые компилятору разрешено предоставить.
    pub resources: Vec<ResourceReference>,
    /// Нефатальные предупреждения генерации (ТЗ §10.5: деградация
    /// mermaid-диаграммы до блока кода).
    pub warnings: Vec<String>,
}

/// Ссылка на локальный ресурс (ТЗ §19, §24.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReference {
    /// Виртуальный путь вида `/mdpdf-resources/000001.png`.
    pub logical_path: String,
    /// Откуда берутся байты. Источник ровно один — это закреплено типом,
    /// а не соглашением.
    pub source: ResourceSource,
    /// Вид ресурса.
    pub kind: ResourceKind,
    /// Диапазон в исходном Markdown.
    pub span: Option<SourceSpan>,
}

/// Источник байтов ресурса (ТЗ §33.2, §10.5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceSource {
    /// Файл рядом с документом: путь из Markdown, компилятор разрешает его
    /// относительно каталога документа.
    File {
        /// Путь в том виде, в каком он записан в Markdown.
        path: String,
    },
    /// Байты, порождённые самим `mdpdf` (SVG диаграммы Mermaid).
    /// Файловая система не участвует.
    Embedded {
        /// Содержимое ресурса.
        bytes: Vec<u8>,
    },
}

impl ResourceReference {
    /// Имя ресурса для сообщений об ошибках: путь из Markdown либо
    /// виртуальный путь, если байты порождены самим `mdpdf`.
    #[must_use]
    pub fn display_path(&self) -> &str {
        match &self.source {
            ResourceSource::File { path } => path,
            ResourceSource::Embedded { .. } => &self.logical_path,
        }
    }
}

/// Вид ресурса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    /// Изображение.
    Image,
}

/// Размер страницы (ТЗ §20.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaperSize {
    /// 210 × 297 мм.
    #[default]
    A4,
    /// 8.5 × 11 дюймов.
    Letter,
}

impl PaperSize {
    /// Имя размера в терминах Typst.
    #[must_use]
    pub const fn typst_name(self) -> &'static str {
        match self {
            Self::A4 => "a4",
            Self::Letter => "us-letter",
        }
    }

    /// Меньшая сторона страницы в миллиметрах — нужна для проверки полей.
    #[must_use]
    pub fn shorter_side_mm(self) -> f64 {
        match self {
            Self::A4 => 210.0,
            Self::Letter => 8.5 * 25.4,
        }
    }

    /// Большая сторона страницы в миллиметрах — нужна для бюджета высоты
    /// диаграмм (ТЗ §10.5).
    #[must_use]
    pub fn longer_side_mm(self) -> f64 {
        match self {
            Self::A4 => 297.0,
            Self::Letter => 11.0 * 25.4,
        }
    }
}

impl FromStr for PaperSize {
    type Err = TypstGenerationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "a4" => Ok(Self::A4),
            "letter" | "us-letter" => Ok(Self::Letter),
            other => Err(TypstGenerationError::InvalidOption {
                name: "paper".to_owned(),
                message: format!("unknown paper size: {other}"),
            }),
        }
    }
}

/// Единица длины (ТЗ §20.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    /// Пункты.
    Pt,
    /// Миллиметры.
    Mm,
    /// Сантиметры.
    Cm,
    /// Дюймы.
    In,
}

impl LengthUnit {
    const fn suffix(self) -> &'static str {
        match self {
            Self::Pt => "pt",
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::In => "in",
        }
    }

    fn to_mm(self, value: f64) -> f64 {
        match self {
            Self::Pt => value * 25.4 / 72.0,
            Self::Mm => value,
            Self::Cm => value * 10.0,
            Self::In => value * 25.4,
        }
    }

    /// Перевод в пункты напрямую: через миллиметры значение вроде `6pt`
    /// возвращалось бы как 5.999999999999999 и не проходило границу диапазона.
    fn to_pt(self, value: f64) -> f64 {
        match self {
            Self::Pt => value,
            Self::Mm => value * 72.0 / 25.4,
            Self::Cm => value * 720.0 / 25.4,
            Self::In => value * 72.0,
        }
    }
}

/// Длина. Произвольные Typst-выражения запрещены (ТЗ §20.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    /// Числовое значение.
    pub value: f64,
    /// Единица измерения.
    pub unit: LengthUnit,
}

impl Length {
    /// Создаёт длину с проверкой значения.
    ///
    /// # Errors
    ///
    /// [`TypstGenerationError::InvalidOption`], если значение не конечное
    /// или не положительное.
    pub fn new(value: f64, unit: LengthUnit, option: &str) -> Result<Self, TypstGenerationError> {
        if !value.is_finite() {
            return Err(TypstGenerationError::InvalidOption {
                name: option.to_owned(),
                message: "length must be finite".to_owned(),
            });
        }
        if value <= 0.0 {
            return Err(TypstGenerationError::InvalidOption {
                name: option.to_owned(),
                message: "length must be positive".to_owned(),
            });
        }
        Ok(Self { value, unit })
    }

    /// Значение в миллиметрах.
    #[must_use]
    pub fn as_mm(self) -> f64 {
        self.unit.to_mm(self.value)
    }

    /// Значение в пунктах.
    #[must_use]
    pub fn as_pt(self) -> f64 {
        self.unit.to_pt(self.value)
    }

    /// Разбирает строку вида `20mm` из CLI.
    ///
    /// # Errors
    ///
    /// [`TypstGenerationError::InvalidOption`] при неизвестной единице,
    /// нечисловом или неположительном значении.
    pub fn parse(input: &str, option: &str) -> Result<Self, TypstGenerationError> {
        let trimmed = input.trim();
        let invalid = |message: &str| TypstGenerationError::InvalidOption {
            name: option.to_owned(),
            message: message.to_owned(),
        };

        let (number, unit) = ["mm", "cm", "in", "pt"]
            .iter()
            .find_map(|suffix| trimmed.strip_suffix(suffix).map(|number| (number, *suffix)))
            .ok_or_else(|| invalid("length must end with pt, mm, cm or in"))?;

        let unit = match unit {
            "pt" => LengthUnit::Pt,
            "mm" => LengthUnit::Mm,
            "cm" => LengthUnit::Cm,
            _ => LengthUnit::In,
        };
        let value: f64 = number
            .trim()
            .parse()
            .map_err(|_| invalid("length must start with a number"))?;
        Self::new(value, unit, option)
    }
}

impl fmt::Display for Length {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Стабильное представление без экспоненты и без хвостовых нулей.
        let value = format!("{:.4}", self.value);
        let value = value.trim_end_matches('0').trim_end_matches('.');
        write!(formatter, "{value}{}", self.unit.suffix())
    }
}

/// Параметры рендеринга (ТЗ §20).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderOptions {
    /// Размер страницы.
    pub paper: PaperSize,
    /// Поля страницы.
    pub margin: Length,
    /// Основной размер текста.
    pub font_size: Length,
    /// Заголовок документа.
    pub title: Option<String>,
    /// Автор.
    pub author: Option<String>,
    /// Строить оглавление.
    pub toc: bool,
    /// Нумеровать заголовки.
    pub heading_numbers: bool,
}

impl RenderOptions {
    /// Минимальный размер основного текста, пункты (ТЗ §20.2).
    pub const MIN_FONT_SIZE_PT: f64 = 6.0;
    /// Максимальный размер основного текста, пункты (ТЗ §20.2).
    pub const MAX_FONT_SIZE_PT: f64 = 72.0;

    /// Проверяет параметры (ТЗ §20.2).
    ///
    /// # Errors
    ///
    /// [`TypstGenerationError::InvalidOption`], если поля больше половины
    /// меньшей стороны страницы или размер текста вне диапазона 6–72 pt.
    pub fn validate(&self) -> Result<(), TypstGenerationError> {
        let half_page = self.paper.shorter_side_mm() / 2.0;
        if self.margin.as_mm() > half_page {
            return Err(TypstGenerationError::InvalidOption {
                name: "margin".to_owned(),
                message: format!(
                    "margin {} exceeds half of the shorter page side ({half_page:.1}mm)",
                    self.margin
                ),
            });
        }

        let font_size = self.font_size.as_pt();
        if !(Self::MIN_FONT_SIZE_PT..=Self::MAX_FONT_SIZE_PT).contains(&font_size) {
            return Err(TypstGenerationError::InvalidOption {
                name: "font-size".to_owned(),
                message: format!(
                    "font size {} is outside {}pt..={}pt",
                    self.font_size,
                    Self::MIN_FONT_SIZE_PT,
                    Self::MAX_FONT_SIZE_PT
                ),
            });
        }
        Ok(())
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            paper: PaperSize::A4,
            margin: Length {
                value: 20.0,
                unit: LengthUnit::Mm,
            },
            font_size: Length {
                value: 11.0,
                unit: LengthUnit::Pt,
            },
            title: None,
            author: None,
            toc: false,
            heading_numbers: false,
        }
    }
}

/// Генератор Typst (ТЗ §19).
///
/// Чистое преобразование `Document + RenderOptions → Typst source`: файлы не
/// читаются, компилятор не вызывается, AST не изменяется.
#[derive(Debug, Clone)]
pub struct TypstGenerator {
    options: RenderOptions,
}

impl TypstGenerator {
    /// Создаёт генератор.
    #[must_use]
    pub const fn new(options: RenderOptions) -> Self {
        Self { options }
    }

    /// Преобразует документ в Typst-исходник.
    ///
    /// # Errors
    ///
    /// [`TypstGenerationError`] при недопустимых параметрах, неэкранируемых
    /// значениях или некорректном пути изображения.
    pub fn generate(&self, document: &Document) -> Result<GeneratedTypst, TypstGenerationError> {
        self.options.validate()?;

        let mut writer = TypstWriter::new();
        let mut resources = Vec::new();
        let mut warnings = Vec::new();

        writer.push_raw(TEMPLATE.trim_end());
        writer.blank_line();
        self.write_preamble(&mut writer);
        writer.blank_line();

        writer.line("#{");
        writer.indented(|writer| {
            blocks::write_blocks(
                writer,
                &document.blocks,
                &mut resources,
                &self.options,
                &mut warnings,
            )
        })?;
        writer.line("}");

        Ok(GeneratedTypst {
            source: writer.finish(),
            resources,
            warnings,
        })
    }

    /// `#show: mdpdf-document.with(...)` — настройки документа (ТЗ §21).
    fn write_preamble(&self, writer: &mut TypstWriter) {
        use crate::typst_gen::escape::string_literal;

        writer.line("#show: mdpdf-document.with(");
        writer.indented_infallible(|writer| {
            writer.line(&format!(
                "paper: {},",
                string_literal(self.options.paper.typst_name())
            ));
            writer.line(&format!("margin: {},", self.options.margin));
            writer.line(&format!("font-size: {},", self.options.font_size));
            writer.line(&format!(
                "title: {},",
                optional_literal(self.options.title.as_deref())
            ));
            writer.line(&format!(
                "author: {},",
                optional_literal(self.options.author.as_deref())
            ));
            writer.line(&format!("toc: {},", self.options.toc));
            writer.line(&format!(
                "heading-numbers: {},",
                self.options.heading_numbers
            ));
        });
        writer.line(")");
    }
}

fn optional_literal(value: Option<&str>) -> String {
    value.map_or_else(
        || "none".to_owned(),
        crate::typst_gen::escape::string_literal,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_are_parsed_with_every_unit() {
        assert_eq!(
            Length::parse("20mm", "margin").expect("mm"),
            Length {
                value: 20.0,
                unit: LengthUnit::Mm
            }
        );
        assert_eq!(Length::parse("2cm", "margin").expect("cm").as_mm(), 20.0);
        assert_eq!(Length::parse("1in", "margin").expect("in").as_mm(), 25.4);
        assert_eq!(Length::parse("72pt", "margin").expect("pt").as_mm(), 25.4);
    }

    #[test]
    fn lengths_reject_garbage() {
        assert!(Length::parse("20", "margin").is_err());
        assert!(Length::parse("mm", "margin").is_err());
        assert!(Length::parse("-5mm", "margin").is_err());
        assert!(Length::parse("0mm", "margin").is_err());
        assert!(Length::parse("nanmm", "margin").is_err());
        assert!(Length::parse("20 mm; #panic()", "margin").is_err());
    }

    #[test]
    fn length_display_is_stable() {
        assert_eq!(
            Length::parse("20mm", "margin").expect("mm").to_string(),
            "20mm"
        );
        assert_eq!(
            Length::parse("11.5pt", "font-size")
                .expect("pt")
                .to_string(),
            "11.5pt"
        );
    }

    #[test]
    fn paper_sizes_are_parsed() {
        assert_eq!(PaperSize::from_str("a4").expect("a4"), PaperSize::A4);
        assert_eq!(
            PaperSize::from_str("LETTER").expect("letter"),
            PaperSize::Letter
        );
        assert!(PaperSize::from_str("a3").is_err());
    }

    #[test]
    fn margin_cannot_exceed_half_the_page() {
        let options = RenderOptions {
            margin: Length::parse("120mm", "margin").expect("margin"),
            ..RenderOptions::default()
        };
        let err = options.validate().expect_err("margin too large");
        assert!(matches!(
            err,
            TypstGenerationError::InvalidOption { ref name, .. } if name == "margin"
        ));
    }

    #[test]
    fn font_size_must_stay_within_six_and_seventy_two_points() {
        for value in ["5pt", "73pt"] {
            let options = RenderOptions {
                font_size: Length::parse(value, "font-size").expect("font size"),
                ..RenderOptions::default()
            };
            assert!(options.validate().is_err(), "{value} must be rejected");
        }
        for value in ["6pt", "11pt", "72pt"] {
            let options = RenderOptions {
                font_size: Length::parse(value, "font-size").expect("font size"),
                ..RenderOptions::default()
            };
            assert!(options.validate().is_ok(), "{value} must be accepted");
        }
    }

    #[test]
    fn defaults_are_valid() {
        assert!(RenderOptions::default().validate().is_ok());
    }
}
