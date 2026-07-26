//! Тесты этапа 2 (ТЗ §29, §47). Наполняются на Milestone 2:
//! golden-тесты Typst, injection-тесты, детерминированность вывода.

use mdpdf::typst_gen;

#[test]
fn template_is_embedded_in_the_binary() {
    assert!(!typst_gen::TEMPLATE.is_empty());
    assert!(typst_gen::TEMPLATE.contains("mdpdf-document"));
}

#[test]
fn template_imports_no_packages() {
    // ТЗ §21: шаблон не должен импортировать пакеты и обращаться наружу.
    assert!(!typst_gen::TEMPLATE.contains("@preview"));
    assert!(!typst_gen::TEMPLATE.contains("#import"));
    assert!(!typst_gen::TEMPLATE.contains("#include"));
}

#[test]
fn resource_prefix_is_virtual() {
    assert_eq!(typst_gen::RESOURCE_PREFIX, "/mdpdf-resources/");
}
