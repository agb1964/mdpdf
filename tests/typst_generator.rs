//! Тесты этапа 2 (ТЗ §28, §29, §47).

use std::path::{Path, PathBuf};

use mdpdf::markdown::parser::MarkdownParser;
use mdpdf::typst_gen;
use mdpdf::typst_gen::generator::{GeneratedTypst, RenderOptions, TypstGenerator};

fn generate(markdown: &str) -> GeneratedTypst {
    generate_with(markdown, RenderOptions::default())
}

fn generate_with(markdown: &str, options: RenderOptions) -> GeneratedTypst {
    let document = MarkdownParser::default()
        .parse(markdown)
        .expect("markdown parses");
    TypstGenerator::new(options)
        .generate(&document)
        .expect("typst generates")
}

/// Тело без встроенного шаблона: именно оно строится из пользовательских данных.
/// Маркер — строка `#{`, открывающая блок выражений; искать `#show:` нельзя:
/// это слово встречается и в комментарии внутри шаблона.
fn body_of(generated: &GeneratedTypst) -> &str {
    let marker = "\n#{\n";
    let start = generated
        .source
        .find(marker)
        .expect("generated source contains the expression block");
    &generated.source[start + 1..]
}

/// Диапазоны строковых литералов Typst в тексте.
fn literal_ranges(source: &str) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                match bytes[index] {
                    b'\\' => index += 2,
                    b'"' => break,
                    _ => index += 1,
                }
            }
            index = index.min(bytes.len());
            ranges.push((start, index.min(bytes.len())));
        }
        index += 1;
    }
    ranges
}

/// Каждое вхождение подстроки лежит внутри строкового литерала, то есть
/// не может быть исполнено как Typst-код (ТЗ §22, §28).
fn only_inside_literals(source: &str, needle: &str) -> bool {
    let ranges = literal_ranges(source);
    source
        .match_indices(needle)
        .all(|(at, _)| ranges.iter().any(|(start, end)| at > *start && at < *end))
}

// --- встроенный шаблон (ТЗ §21) -----------------------------------------------

#[test]
fn template_is_embedded_in_the_binary() {
    assert!(!typst_gen::TEMPLATE.is_empty());
    assert!(typst_gen::TEMPLATE.contains("mdpdf-document"));
}

#[test]
fn template_imports_no_packages() {
    assert!(!typst_gen::TEMPLATE.contains("@preview"));
    assert!(!typst_gen::TEMPLATE.contains("#import"));
    assert!(!typst_gen::TEMPLATE.contains("#include"));
}

#[test]
fn template_defines_every_function_the_generator_calls() {
    for function in [
        "mdpdf-document",
        "mdpdf-code",
        "mdpdf-inline-code",
        "mdpdf-quote",
        "mdpdf-task",
        "mdpdf-list",
        "mdpdf-table",
        "mdpdf-image",
        "mdpdf-rule",
        "mdpdf-diagram",
    ] {
        assert!(
            typst_gen::TEMPLATE.contains(&format!("#let {function}(")),
            "template is missing {function}"
        );
    }
}

#[test]
fn resource_prefix_is_virtual() {
    assert_eq!(typst_gen::RESOURCE_PREFIX, "/mdpdf-resources/");
}

// --- форматирование и детерминированность (ТЗ §25, §26) -----------------------

#[test]
fn output_is_deterministic() {
    let markdown = "# Заголовок\n\n![a](one.png)\n\n![b](two.png)\n";
    let first = generate(markdown);
    let second = generate(markdown);
    assert_eq!(first.source, second.source);
    assert_eq!(first.resources, second.resources);
}

#[test]
fn output_has_no_trailing_whitespace_and_ends_with_one_newline() {
    let generated = generate("# Заголовок\n\nТекст.\n");
    assert!(generated.source.ends_with('\n'));
    assert!(!generated.source.ends_with("\n\n"));
    assert!(!generated.source.contains('\r'));
    for line in generated.source.lines() {
        assert_eq!(
            line,
            line.trim_end(),
            "line has trailing whitespace: {line}"
        );
    }
}

#[test]
fn resources_are_numbered_in_traversal_order() {
    let generated = generate("![a](first.png)\n\n![b](second.jpg)\n");
    let paths: Vec<&str> = generated
        .resources
        .iter()
        .map(|resource| resource.logical_path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["/mdpdf-resources/000001.png", "/mdpdf-resources/000002.jpg"]
    );
    let sources: Vec<&str> = generated
        .resources
        .iter()
        .map(|resource| resource.source_path.as_str())
        .collect();
    assert_eq!(sources, vec!["first.png", "second.jpg"]);
}

#[test]
fn generated_source_never_contains_absolute_local_paths() {
    let generated = generate("![a](images/schema.png)\n");
    assert!(!generated.source.contains("images/schema.png"));
    assert!(generated.source.contains("/mdpdf-resources/000001.png"));
}

// --- инъекции (ТЗ §28) --------------------------------------------------------

const PAYLOADS: [&str; 10] = [
    "#panic(\"injected\")",
    "#include \"secret\"",
    "#import \"@preview/package\"",
    "$ x + y $",
    "[content]",
    "\"quoted\"",
    "\\escaped",
    "// comment",
    "/* comment */",
    "#let evil = 1",
];

#[test]
fn payloads_in_body_text_are_not_executable() {
    for payload in PAYLOADS {
        let markdown = format!("# {payload}\n\n{payload}\n\n- {payload}\n");
        let document = MarkdownParser::default()
            .parse(&markdown)
            .expect("markdown parses");
        let generated = TypstGenerator::new(RenderOptions::default())
            .generate(&document)
            .expect("typst generates");
        let body = body_of(&generated);
        assert!(
            only_inside_literals(body, payload.trim_start_matches('\\')),
            "payload escaped a literal: {payload}"
        );
    }
}

#[test]
fn payloads_in_metadata_are_not_executable() {
    for payload in PAYLOADS {
        let options = RenderOptions {
            title: Some(payload.to_owned()),
            author: Some(payload.to_owned()),
            ..RenderOptions::default()
        };
        let generated = generate_with("текст\n", options);
        let body = body_of(&generated);
        assert!(
            only_inside_literals(body, payload.trim_start_matches('\\')),
            "payload escaped a literal in metadata: {payload}"
        );
    }
}

#[test]
fn payloads_in_urls_and_alt_text_are_not_executable() {
    for payload in PAYLOADS {
        let markdown = format!("[{payload}](a.md) ![{payload}](pic.png)\n");
        let generated = generate(&markdown);
        let body = body_of(&generated);
        assert!(
            only_inside_literals(body, payload.trim_start_matches('\\')),
            "payload escaped a literal in link or alt text: {payload}"
        );
    }
}

#[test]
fn payloads_in_code_blocks_and_language_are_not_executable() {
    for payload in PAYLOADS {
        let markdown = format!("```{payload}\n{payload}\n```\n");
        let generated = generate(&markdown);
        let body = body_of(&generated);
        assert!(
            only_inside_literals(body, payload.trim_start_matches('\\')),
            "payload escaped a literal in a code block: {payload}"
        );
    }
}

#[test]
fn payloads_in_table_cells_are_not_executable() {
    for payload in PAYLOADS {
        let cell = payload.replace('|', "");
        let markdown = format!("| {cell} | b |\n|---|---|\n| {cell} | d |\n");
        let generated = generate(&markdown);
        let body = body_of(&generated);
        assert!(
            only_inside_literals(body, cell.trim_start_matches('\\')),
            "payload escaped a literal in a table cell: {payload}"
        );
    }
}

#[test]
fn backticks_inside_code_do_not_terminate_the_block() {
    let generated = generate("````\n```\nfake fence\n```\n````\n");
    let body = body_of(&generated);
    assert!(body.contains("mdpdf-code(language: none, body: \""));
    assert!(only_inside_literals(body, "```"));
}

// --- диаграммы Mermaid (ТЗ §10.5) ----------------------------------------------

#[test]
fn mermaid_flowchart_generates_a_diagram_call() {
    let generated =
        generate("```mermaid\ngraph TD\nA[Начало] --> B{Готово?}\nB -->|да| C[Конец]\n```\n");
    let body = body_of(&generated);
    assert!(body.contains("mdpdf-diagram(width: "), "{body}");
    assert!(body.contains("\"Начало\""));
    assert!(body.contains("\"diamond\""));
    assert!(body.contains("\"да\""));
    assert!(generated.warnings.is_empty());
}

#[test]
fn mermaid_sequence_generates_a_diagram_call() {
    let generated = generate("```mermaid\nsequenceDiagram\nA->>B: привет\nB-->A: ответ\n```\n");
    let body = body_of(&generated);
    assert!(body.contains("mdpdf-diagram(width: "), "{body}");
    assert!(body.contains("\"filled-arrow\""));
    assert!(body.contains("\"dashed\""));
    assert!(generated.warnings.is_empty());
}

#[test]
fn payloads_in_mermaid_labels_are_not_executable() {
    for payload in PAYLOADS {
        // Квадратные скобки — синтаксис узлов `id[…]`; форма `id["…"]` ещё
        // и снимает обрамляющие кавычки как синтаксис Mermaid. Проверяем
        // через `id(…)` и подпись ребра, где payload остаётся текстом.
        let label = payload.replace(['[', ']', '(', ')'], "");
        let markdown = format!("```mermaid\ngraph TD\nA({label}) -->|{label}| B\n```\n");
        let generated = generate(&markdown);
        let body = body_of(&generated);
        assert!(
            only_inside_literals(body, label.trim_start_matches('\\')),
            "payload escaped a literal in a mermaid label: {payload}"
        );
    }
}

#[test]
fn unsupported_mermaid_diagram_falls_back_to_code_with_a_warning() {
    let generated = generate("```mermaid\ngantt\ntitle План\n```\n");
    let body = body_of(&generated);
    assert!(body.contains("mdpdf-code(language: \"mermaid\""));
    assert!(!body.contains("mdpdf-diagram"));
    assert_eq!(generated.warnings.len(), 1);
    assert!(
        generated.warnings[0].contains("unsupported diagram type"),
        "{}",
        generated.warnings[0]
    );
}

#[test]
fn invalid_mermaid_falls_back_to_code_with_a_warning() {
    let generated = generate("```mermaid\ngraph TD\na[unclosed --> b\n```\n");
    assert_eq!(generated.warnings.len(), 1);
    let body = body_of(&generated);
    assert!(body.contains("mdpdf-code(language: \"mermaid\""));
}

#[test]
fn mermaid_output_is_deterministic() {
    let markdown = "```mermaid\ngraph TD\nA --> B\nB --> C\nC --> A\n```\n";
    assert_eq!(generate(markdown).source, generate(markdown).source);
}

// --- golden-тесты (ТЗ §47) ----------------------------------------------------

const FIXTURES: [&str; 5] = ["basic", "nesting", "table", "cyrillic", "mermaid"];

fn fixture_dir(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(kind)
}

#[test]
fn typst_matches_golden_files() {
    let update = std::env::var_os("MDPDF_UPDATE_GOLDEN").is_some();

    for name in FIXTURES {
        let markdown_path = fixture_dir("markdown").join(format!("{name}.md"));
        let golden_path = fixture_dir("expected_typst").join(format!("{name}.typ"));

        let source = std::fs::read_to_string(&markdown_path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", markdown_path.display()));
        let actual = generate(&source).source;

        if update {
            std::fs::write(&golden_path, &actual).expect("golden file is writable");
            continue;
        }

        let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|error| {
            panic!(
                "cannot read {}: {error}\nrun `make golden-update` to create it",
                golden_path.display()
            )
        });
        assert_eq!(
            actual, expected,
            "Typst for {name}.md differs from the golden file"
        );
    }
}
