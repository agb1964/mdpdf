# AGENTS.md

Инварианты проекта `mdpdf`. Полный текст требований — `docs/mdpdf-technical-spec-v2.md`
(редакция 2.0). Старое ТЗ в `docs/Техническое задание.pdf` заменено.

## Что это

CLI-утилита: `Markdown → собственное AST → Typst source → встроенный Typst-компилятор → PDF`.
Один бинарный файл, без сети и внешних процессов.

## Границы слоёв — нарушать нельзя

| Модуль | Знает о | Не знает о |
|---|---|---|
| `src/markdown/` | `pulldown-cmark`, `ast` | Typst, PDF, вывод в файл |
| `src/ast/` | — | Typst, Markdown-парсер |
| `src/mermaid/` | интеграция mmdr / legacy layout на время миграции (ТЗ §10.5) | `pulldown-cmark`, файловая система, сеть, JS |
| `src/typst_gen/` | `ast`, mermaid→SVG resource | `pulldown-cmark`, компилятор |
| `src/compiler/` | `typst`, `typst_pdf` | Markdown |

`typst::*` и `typst_pdf::*` импортируются **только** внутри `src/compiler/`.
Типы `pulldown-cmark` не экспортируются наружу из `src/markdown/`.

## Запрещено

- Вызов внешних процессов, включая `typst` CLI, Node.js, mermaid-cli.
- Сетевой доступ, загрузка ресурсов по HTTP/HTTPS, Typst Universe и любые внешние пакеты.
- Chromium, Puppeteer, WebView, HTML/CSS как промежуточный формат, Pandoc, LaTeX.
- Встраивание JS-движка (V8/QuickJS/Boa) для прогона Mermaid.js; рендер Mermaid —
  только `mermaid-rs-renderer` → SVG → Typst image (ТЗ §10.5, `deny.toml`).
- Системные шрифты, кеш в домашнем каталоге, чтение произвольных файлов.
- `unsafe` (`#![forbid(unsafe_code)]`), `unwrap()`, `expect()`, `panic!` в production-коде.
- `anyhow` в доменных модулях — только `thiserror`; `anyhow` допустим в тестовых хелперах.
- Вставка пользовательского текста в Typst без экранирования: **ни один** пользовательский
  фрагмент не должен интерпретироваться как Typst-код.
- Недетерминированность генерации: время, случайные значения, обход `HashMap` без сортировки,
  абсолютные пути, сведения об ОС.

## Обязательные команды перед коммитом

Всё сразу:

```bash
make ci
```

По отдельности:

```bash
cargo fmt --all --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo doc --no-deps
cargo deny check
```

## Версии зависимостей

`typst` и `typst-pdf` закреплены точной равной версией одной линии (`=0.15.1`),
`pulldown-cmark` — `=0.13.4`. `Cargo.lock` хранится в Git. Обновление Typst —
отдельной задачей с полным test suite и сравнением golden PDF.

## Статус

Журнал работ, принятые решения и долги — `docs/progress.md`; карта документации
— `docs/README.md`. Обновлять при значимых изменениях и перед выпуском.

Remote, кроссплатформенный GitHub CI и технический release `v0.1.0` проверены.
Версия **1.0** пока не объявлена; актуальные условия выпуска — ТЗ §24 и
`docs/releasing.md`.

Эталонные AST/Typst/PDF обновляются только явной командой `make golden-update`.
