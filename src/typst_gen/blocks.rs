//! Генерация блочных узлов (ТЗ §24.2–24.11).
//!
//! Всё выводится типизированными вызовами функций шаблона; ручная строковая
//! Typst-разметка не используется.

use crate::ast::Spanned;
use crate::ast::block::{
    Alignment, Block, CodeBlock, Heading, List, ListItem, ListKind, Table, TableRow,
};
use crate::typst_gen::error::TypstGenerationError;
use crate::typst_gen::escape::string_literal;
use crate::typst_gen::generator::ResourceReference;
use crate::typst_gen::inlines::{EMPTY_CONTENT, inline_expression};
use crate::typst_gen::writer::TypstWriter;

/// Пишет блоки верхнего уровня, по одному на строку, с пустой строкой между
/// ними (ТЗ §26).
///
/// # Errors
///
/// [`TypstGenerationError`] от вложенных узлов.
pub fn write_blocks(
    writer: &mut TypstWriter,
    blocks: &[Spanned<Block>],
    resources: &mut Vec<ResourceReference>,
) -> Result<(), TypstGenerationError> {
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            writer.blank_line();
        }
        let expression = block_expression(block, resources)?;
        writer.line(&expression);
    }
    Ok(())
}

/// Выражение Typst для одного блока.
///
/// # Errors
///
/// [`TypstGenerationError`] от вложенных узлов.
pub fn block_expression(
    block: &Spanned<Block>,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let expression = match &block.value {
        Block::Heading(heading) => heading_expression(heading, resources)?,
        Block::Paragraph(paragraph) => {
            format!("par({})", inline_expression(&paragraph.content, resources)?)
        }
        Block::CodeBlock(code) => code_expression(code),
        Block::Quote(quote) => {
            format!(
                "mdpdf-quote({})",
                blocks_expression(&quote.blocks, resources)?
            )
        }
        Block::List(list) => list_expression(list, resources)?,
        Block::Table(table) => table_expression(table, resources)?,
        Block::ThematicBreak => "mdpdf-rule()".to_owned(),
    };
    Ok(expression)
}

/// Последовательность блоков как одно content-выражение.
fn blocks_expression(
    blocks: &[Spanned<Block>],
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    if blocks.is_empty() {
        return Ok(EMPTY_CONTENT.to_owned());
    }
    let mut parts = Vec::with_capacity(blocks.len());
    for block in blocks {
        parts.push(block_expression(block, resources)?);
    }
    Ok(parts.join(" + "))
}

/// Заголовок (ТЗ §24.2). Нумерация выполняется шаблоном, а не Rust-кодом.
fn heading_expression(
    heading: &Heading,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let body = inline_expression(&heading.content, resources)?;
    Ok(format!(
        "heading(level: {}, {body})",
        heading.level.number()
    ))
}

/// Блок кода (ТЗ §24.7). Код передаётся строковым значением, а не raw-разметкой
/// с подбором количества обратных кавычек (ТЗ §23.3).
fn code_expression(code: &CodeBlock) -> String {
    let language = code
        .language
        .as_deref()
        .map_or_else(|| "none".to_owned(), string_literal);
    format!(
        "mdpdf-code(language: {language}, body: {})",
        string_literal(&code.code)
    )
}

/// Список (ТЗ §24.9). Символы ☐ и ☑ в Rust-коде не появляются: состояние
/// передаётся логически, отображение определяет шаблон.
fn list_expression(
    list: &List,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let mut items = Vec::with_capacity(list.items.len());
    for item in &list.items {
        items.push(list_item_expression(item, resources)?);
    }
    // Одноэлементный кортеж Typst требует завершающей запятой.
    let joined = if items.len() == 1 {
        format!("{},", items[0])
    } else {
        items.join(", ")
    };

    let (ordered, start) = match list.kind {
        ListKind::Unordered => ("false", 1),
        ListKind::Ordered { start } => ("true", start),
    };
    Ok(format!(
        "mdpdf-list(ordered: {ordered}, start: {start}, items: ({joined}))"
    ))
}

fn list_item_expression(
    item: &ListItem,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let body = blocks_expression(&item.blocks, resources)?;
    let checked = match item.checked {
        None => "none".to_owned(),
        Some(true) => "true".to_owned(),
        Some(false) => "false".to_owned(),
    };
    Ok(format!("mdpdf-task(checked: {checked}, {body})"))
}

/// Таблица (ТЗ §24.10): типизированный вызов функции шаблона, а не ручная
/// строковая разметка.
fn table_expression(
    table: &Table,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let columns = table.header.cells.len();
    let alignments: Vec<&str> = table.alignments.iter().map(alignment_name).collect();
    let alignments = tuple(&alignments);

    let header = row_expression(&table.header, resources)?;
    let mut rows = Vec::with_capacity(table.rows.len());
    for row in &table.rows {
        rows.push(row_expression(row, resources)?);
    }
    let rows = tuple(&rows.iter().map(String::as_str).collect::<Vec<_>>());

    Ok(format!(
        "mdpdf-table(columns: {columns}, alignments: {alignments}, header: {header}, rows: {rows})"
    ))
}

fn row_expression(
    row: &TableRow,
    resources: &mut Vec<ResourceReference>,
) -> Result<String, TypstGenerationError> {
    let mut cells = Vec::with_capacity(row.cells.len());
    for cell in &row.cells {
        cells.push(inline_expression(&cell.content, resources)?);
    }
    Ok(tuple(&cells.iter().map(String::as_str).collect::<Vec<_>>()))
}

const fn alignment_name(alignment: &Alignment) -> &'static str {
    match alignment {
        // `auto` оставляет решение шаблону, как и отсутствие выравнивания в Markdown.
        Alignment::None => "auto",
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
    }
}

/// Кортеж Typst со стабильной формой: одноэлементный получает запятую.
fn tuple(items: &[&str]) -> String {
    match items {
        [] => "()".to_owned(),
        [single] => format!("({single},)"),
        many => format!("({})", many.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SourceSpan;
    use crate::ast::block::{Paragraph, TableCell};
    use crate::ast::inline::Inline;

    fn spanned(block: Block) -> Spanned<Block> {
        Spanned::new(block, SourceSpan::new(0, 1))
    }

    fn generate(block: Block) -> String {
        let mut resources = Vec::new();
        block_expression(&spanned(block), &mut resources).expect("block generates")
    }

    #[test]
    fn heading_level_is_passed_as_a_number() {
        let block = Block::Heading(Heading {
            level: crate::ast::block::HeadingLevel::H3,
            content: vec![Inline::Text("Заголовок".to_owned())],
            id: None,
        });
        assert_eq!(generate(block), "heading(level: 3, text(\"Заголовок\"))");
    }

    #[test]
    fn code_block_is_passed_as_a_string_value() {
        let block = Block::CodeBlock(CodeBlock {
            language: Some("rust".to_owned()),
            code: "let a = \"```\";".to_owned(),
        });
        assert_eq!(
            generate(block),
            "mdpdf-code(language: \"rust\", body: \"let a = \\\"```\\\";\")"
        );
    }

    #[test]
    fn code_block_without_language_passes_none() {
        let block = Block::CodeBlock(CodeBlock {
            language: None,
            code: String::new(),
        });
        assert_eq!(generate(block), "mdpdf-code(language: none, body: \"\")");
    }

    #[test]
    fn task_state_is_logical_and_never_a_unicode_box() {
        let block = Block::List(List {
            kind: ListKind::Unordered,
            items: vec![
                ListItem {
                    checked: Some(true),
                    blocks: vec![],
                },
                ListItem {
                    checked: None,
                    blocks: vec![],
                },
            ],
        });
        let expression = generate(block);
        assert!(expression.contains("checked: true"));
        assert!(expression.contains("checked: none"));
        assert!(!expression.contains('☐'));
        assert!(!expression.contains('☑'));
    }

    #[test]
    fn ordered_list_start_is_preserved() {
        let block = Block::List(List {
            kind: ListKind::Ordered { start: 7 },
            items: vec![ListItem {
                checked: None,
                blocks: vec![],
            }],
        });
        let expression = generate(block);
        assert!(expression.contains("ordered: true"));
        assert!(expression.contains("start: 7"));
        // Одноэлементный кортеж обязан иметь завершающую запятую.
        assert!(expression.contains("items: (mdpdf-task(checked: none, []),)"));
    }

    #[test]
    fn table_is_a_typed_call() {
        let cell = |text: &str| TableCell {
            content: vec![Inline::Text(text.to_owned())],
        };
        let block = Block::Table(Table {
            alignments: vec![Alignment::Left, Alignment::None],
            header: TableRow {
                cells: vec![cell("a"), cell("b")],
            },
            rows: vec![TableRow {
                cells: vec![cell("1"), cell("2")],
            }],
        });
        assert_eq!(
            generate(block),
            "mdpdf-table(columns: 2, alignments: (left, auto), \
             header: (text(\"a\"), text(\"b\")), rows: ((text(\"1\"), text(\"2\")),))"
        );
    }

    #[test]
    fn thematic_break_is_a_template_call() {
        assert_eq!(generate(Block::ThematicBreak), "mdpdf-rule()");
    }

    #[test]
    fn quote_nests_blocks() {
        let inner = spanned(Block::Paragraph(Paragraph {
            content: vec![Inline::Text("a".to_owned())],
        }));
        let block = Block::Quote(crate::ast::block::BlockQuote {
            blocks: vec![inner],
        });
        assert_eq!(generate(block), "mdpdf-quote(par(text(\"a\")))");
    }

    #[test]
    fn blocks_are_separated_by_one_blank_line() {
        let mut writer = TypstWriter::new();
        let mut resources = Vec::new();
        let blocks = vec![spanned(Block::ThematicBreak), spanned(Block::ThematicBreak)];
        write_blocks(&mut writer, &blocks, &mut resources).expect("writes");
        assert_eq!(writer.finish(), "mdpdf-rule()\n\nmdpdf-rule()\n");
    }
}
