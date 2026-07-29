# Участие в разработке `mdpdf`

Спасибо за желание улучшить проект. `mdpdf` обрабатывает пользовательские
файлы и генерирует исполняемый Typst source, поэтому изменения должны сохранять
границы слоёв, детерминированность и модель безопасности.

## Подготовка окружения

Нужен стабильный Rust. Файл `rust-toolchain.toml` автоматически подключает
`rustfmt` и Clippy.

```bash
git clone https://github.com/agb1964/mdpdf.git
cd mdpdf
cargo build
```

Для полного набора локальных проверок установите `cargo-deny`:

```bash
cargo install cargo-deny
```

`cargo-llvm-cov`, nightly Rust и `cargo-fuzz` нужны только для покрытия и
fuzzing:

```bash
cargo install cargo-llvm-cov cargo-fuzz
rustup toolchain install nightly
```

## Обязательные инварианты

Перед изменением кода прочитайте:

- [`AGENTS.md`](AGENTS.md) — краткие запреты и границы модулей;
- [`docs/mdpdf-technical-spec-v2.md`](docs/mdpdf-technical-spec-v2.md) —
  полный контракт;
- [`docs/progress.md`](docs/progress.md) — принятые решения и текущий долг.

Особенно важно:

- `markdown` не знает о Typst, PDF и записи файлов;
- `ast` не зависит от Markdown parser и Typst;
- `mermaid` использует собственную модель диаграмм и не знает о Typst,
  Markdown parser и файловой системе;
- `typst_gen` знает только об AST и `mermaid` и не читает файлы;
- типы `typst`, `typst-layout` и `typst-pdf` используются только в `compiler`;
- пользовательский текст никогда не вставляется в Typst как исполняемый код;
- production-код не использует `unsafe`, `unwrap()`, `expect()` или `panic!()`;
- программа не обращается к сети и не запускает внешние процессы.

## Рабочий процесс

1. Создайте отдельную ветку.
2. Добавьте минимальное изменение и тест основного сценария.
3. Для практически возможной ошибки добавьте regression test.
4. Обновите документацию и `CHANGELOG.md`, если меняется видимое поведение.
5. Выполните:

   ```bash
   make ci
   ```

6. Проверьте `git diff --check` и просмотрите итоговый diff.

`make ci` запускает форматирование, `cargo check`, Clippy, тесты, rustdoc и
`cargo deny`. Coverage остаётся информационным показателем:

```bash
make coverage
```

## Golden-файлы

AST, Typst и структурные PDF golden-файлы изменяются только тогда, когда новый
результат является ожидаемым изменением контракта.

```bash
make golden-update
git diff -- tests/fixtures
```

Не принимайте обновлённый golden автоматически. Проверьте семантику AST,
экранирование Typst и визуальный результат PDF. Обычный `make ci` golden-файлы
не перезаписывает.

## Fuzzing

Fuzzing обязателен перед значимым релизом и после изменений parser, AST
validation или Typst escaping:

```bash
make fuzz
make fuzz FUZZ_TIME=300
```

Корпуса и артефакты fuzzing в Git не добавляются.

## Зависимости и встроенные ресурсы

`typst` и `typst-pdf` закреплены одной точной версией, `pulldown-cmark` также
закреплён. Их обновление выполняется отдельной задачей с полным `make ci`,
fuzzing и сравнением golden PDF.

Новая зависимость должна давать пользовательскую пользу и проходить
`cargo deny check`. Для шрифтов дополнительно соблюдайте процедуру из
[`assets/fonts/README.md`](assets/fonts/README.md).

## Pull request

В описании укажите:

- какую пользовательскую или техническую проблему решает изменение;
- какие риски затрагивает;
- какие проверки выполнены;
- почему изменились golden-файлы, если они изменились.

Большие структурные миграции, обновление Typst и функциональные изменения лучше
разделять на независимые pull request.
