# Отчёт по code review: `mdpdf`

| | |
|---|---|
| Дата | 2026-07-26 |
| Репозиторий | `/Users/anatoliy/rust.projects/md2pdf` |
| Ветка | `master` |
| HEAD | `b009099` — «Журнал: Milestone 2 и 3 закрыты» |
| Объём | 33 `.rs` в `src/`, интеграционные тесты, `assets/template.typ`, `Cargo.toml`, `docs/progress.md` |
| Метод | Полный проход по слоям (app/CLI, markdown/AST, typst_gen, compiler) + точечная верификация P0/P1 |

**Цель отчёта:** зафиксировать состояние кода относительно ТЗ и `AGENTS.md`, перечислить дефекты по приоритету и указать следующий шаг к Definition of Done.

---

## 1. Резюме для руководства

CLI-утилита `mdpdf` реализует конвейер **Markdown → собственное AST → Typst source → встроенный Typst → PDF** в одном бинарнике без сети и внешних процессов.

| Milestone | Содержание | Статус |
|---|---|---|
| 0 | Каркас, CLI, ошибки, CI, шрифты | **готов** |
| 1 | Markdown → AST | **готов** |
| 2 | AST → Typst source | **готов** |
| 3 | Встроенная компиляция Typst → PDF | **готов** |
| 4 | Интеграция: atomic write, overwrite, сообщение | **частично** |
| 5 | Hardening: лимиты, fuzz, golden PDF | **не начат** |

**Главный вывод:** слои 1–3 и оркестрация выглядят зрелыми, границы модулей соблюдены, инъекции Typst и sandbox World закрыты на уровне текущего threat model. **Продуктовый сценарий `mdpdf document.md -o document.pdf` не завершается успехом:** после компиляции приложение намеренно возвращает `NotImplemented` (exit 1), PDF на диск не пишется. Это блокирует Definition of Done (§61) и закрытие Milestone 4.

**Критичных (P0) проблем безопасности / паник в production-коде не найдено.**  
**P0 по продукту:** запись PDF и защита от перезаписи.

---

## 2. Архитектура и инварианты

### 2.1 Конвейер

```
CLI (clap)
  → app::run
      → source::read_source
      → markdown::MarkdownParser → ast::Document
      → typst_gen::TypstGenerator → GeneratedTypst
      → compiler::EmbeddedTypstCompiler → PDF bytes
      → output::write_pdf_atomically   ← заглушка (M4)
```

### 2.2 Границы слоёв

| Модуль | Знает о | Не должен знать о | Факт |
|---|---|---|---|
| `src/markdown/` | `pulldown-cmark`, `ast` | Typst, PDF, FS-вывод | OK |
| `src/ast/` | — | Typst, Markdown-парсер | OK |
| `src/typst_gen/` | `ast` | pulldown, FS, compiler | OK |
| `src/compiler/` | `typst`, `typst_pdf`, `typst-layout` | Markdown | OK |
| `src/app.rs` | все слои (оркестрация) | прямые `typst::*` | OK |

### 2.3 Проверка инвариантов AGENTS.md / ТЗ

| Инвариант | Вердикт | Комментарий |
|---|---|---|
| `#![forbid(unsafe_code)]` | OK | `src/lib.rs` |
| Нет `unwrap` / `expect` / `panic!` в production | OK | Только в `#[cfg(test)]` и интеграционных тестах |
| `thiserror` в domain, без `anyhow` | OK | |
| `typst::*` только в `compiler/` | OK | |
| `pulldown-cmark` не re-export | Частично | Публичный `AstBuilder::handle(Event)` требует тип pulldown |
| Экранирование user text в Typst | OK | string literals + typed template calls |
| Детерминизм генерации | В основном OK | Нет time/HashMap/OS paths в gen; PDF byte-equal слабо проверен |
| Нет сети / внешних процессов / системных шрифтов | OK | World + pre-load resources |
| Exit codes 0–9 | Частично | Каркас есть; validation → 4 вместо 5; happy-path PDF → 1 |

---

## 3. Сводка findings

| Приоритет | Кол-во | Смысл |
|---|---|---|
| **P0** | 2 | Блокируют DoD / основной сценарий |
| **P1** | 3 | Неверное поведение, exit codes, UX диагностики, лимиты |
| **P2** | ~15 | Корректность API, hardening, согласованность |
| **P3** | ~15 | Долг, косметика, мёртвый код, слабые тесты |

---

## 4. Findings подробно

### 4.1 P0 — блокируют Definition of Done

#### P0-1. PDF не записывается на диск

| | |
|---|---|
| **Где** | `src/app.rs:124-127`, `src/output.rs:21-25` |
| **Суть** | После успешной компиляции `app::run` всегда возвращает `AppError::NotImplemented { feature: "atomic PDF write (Milestone 4)" }`. Функция `write_pdf_atomically` — заглушка и из `app` **не вызывается**. |
| **Влияние** | `mdpdf document.md -o document.pdf` завершается с кодом **1**, файл PDF не создаётся. Компилятор уже умеет отдавать валидные байты (`tests/compiler.rs`). |
| **Требование** | ТЗ §6.4, §61 (DoD), Milestone 4 |
| **Исправление** | 1) Реализовать atomic write: temp в каталоге назначения → write → flush → rename. 2) Вызвать из `app` при `config.output`. 3) Печатать `Created …` если не `--quiet`. |

#### P0-2. `--overwrite` / `OutputExists` (exit 8) не подключены

| | |
|---|---|
| **Где** | `src/cli.rs`, `src/config.rs` (поле есть), `src/error.rs` (вариант есть), `src/app.rs` / `src/output.rs` (логики нет) |
| **Суть** | Флаг принимается, код завершения 8 описан, но существование целевого PDF нигде не проверяется. |
| **Влияние** | Нарушение §6.2 (запрет молчаливой перезаписи) после появления записи; сценарий e2e §48.20. |
| **Исправление** | В `write_pdf_atomically`: если `path.exists() && !overwrite` → `AppError::OutputExists`; иначе atomic replace. |

---

### 4.2 P1 — correctness / UX / контракты

#### P1-1. Ошибки AST validation: exit 4 вместо 5, без `file:line:col`

| | |
|---|---|
| **Где** | `src/markdown/parser.rs` (вызов `validate_document`), `src/markdown/error.rs:66-73`, `src/app.rs:141-152`, `src/error.rs` |
| **Суть** | `validate_document` оборачивается в `MarkdownError::AstValidation`. `MarkdownError::span()` для этого варианта всегда `None`, хотя у `AstValidationError` span есть. `app::parse` мапит всё без span в `AppError::Markdown` → **exit 4**. Вариант `AppError::AstValidation` (**exit 5**) конвейером не порождается. |
| **Влияние** | Сетевые картинки, nested link, `0.`-списки, битые таблицы: нет позиции в диагностике (§16), неверный код выхода (§43). |
| **Исправление** | Пробросить span из `AstValidationError` в `MarkdownError::span()`; в `app::parse` маппить `AstValidation` → `AppError::AstValidation` / `MarkdownAt` с exit 5; добавить CLI-тест. |

#### P1-2. Лимит входа 32 MiB проверяется после полной загрузки

| | |
|---|---|
| **Где** | `src/source.rs` |
| **Суть** | `fs::read` / `read_to_end` читают весь поток, затем сравнивают с `MAX_INPUT_BYTES`. |
| **Влияние** | Злонамеренный/случайный гигабайтный input может вызвать OOM до ошибки `InvalidInput`. Заявлено как выполненное (§13/§40). |
| **Исправление** | `Read::take(limit+1)` и/или `metadata().len()` до чтения; unit-тест на oversize. |

#### P1-3. Интеграционные тесты закрепляют «PDF ещё не готов»

| | |
|---|---|
| **Где** | `tests/end_to_end.rs`, частично `tests/cli.rs` |
| **Суть** | E2E ожидает failure на обычном запуске; нет happy-path с `%PDF-`, stdin→PDF, overwrite, atomic leftovers. Комментарии устарели (компилятор уже есть). |
| **Влияние** | После M4 тесты станут ложно-красными/зелёными; Milestone 4/5 без e2e не закрыть. |
| **Исправление** | Переписать на success + проверка сигнатуры PDF; `--check` → success; сценарии overwrite и path policy. |

---

### 4.3 P2 — по слоям

#### App / CLI / IO

| ID | Где | Проблема |
|---|---|---|
| P2-A1 | `app.rs` emit paths | `--emit-ast` / `--emit-typst` всегда перезаписывают через `fs::write`, без политики overwrite |
| P2-A2 | `config.rs` + `error.rs` | Невалидные `--margin` / `--font-size` → exit **6** (generation), а не **2** (CLI) |
| P2-A3 | `config.rs` | Paper: `format!("{:?}", args.paper).to_lowercase()` — хрупко к `Debug` |
| P2-A4 | `app.rs`, README, e2e | Устаревшие комментарии («шаги до AST», «компилятор появится») |
| P2-A5 | `app.rs` vs `output.rs` | Два разных «NotImplemented»; stub не вызывается |

#### Markdown → AST

| ID | Где | Проблема |
|---|---|---|
| P2-M1 | `markdown/mod.rs`, `builder.rs` | Публичный `AstBuilder::handle(Event)` — слабая утечка `pulldown-cmark` (§15) |
| P2-M2 | `builder.rs` `pop_frame` | При mismatch кадр уже снят → стек после `Err` неконсистентен |
| P2-M3 | builder / validate | `MAX_AST_NODES` / nesting depth **не enforced** (долг M5) |
| P2-M4 | `ast/validate.rs` | Network image: нет `//host/...`, `file:` — только `http(s):` / `data:` |
| P2-M5 | `builder.rs` | Span неявного абзаца = span первого inline (узкая диагностика) |

#### typst_gen

| ID | Где | Проблема |
|---|---|---|
| P2-T1 | `generator.rs` | `document.metadata` не читается; title/author только из `RenderOptions` (CLI-путь ок) |
| P2-T2 | `escape.rs` | `path_literal` публичный, не требует префикса `/mdpdf-resources/` |
| P2-T3 | `inlines.rs` / `blocks.rs` | `Link.title`, `Image.title`, `Heading.id` молча отбрасываются |
| P2-T4 | generator | Лимит числа изображений только в compiler; `--emit-typst` без потолка |

**Инъекций P0/P1 в typst_gen не найдено.** Модель «typed template calls + string literals» соответствует §22–§28.

#### compiler

| ID | Где | Проблема |
|---|---|---|
| P2-C1 | `files.rs` | TOCTOU: `canonicalize` + `starts_with`, затем `read` — symlink может смениться |
| P2-C2 | `files.rs` | SVG по magic-byte; нет pre-reject внешних `href` / `xlink:href` |
| P2-C3 | tests | Нет unit-теста symlink → вне `base_dir` |
| P2-C4 | `template.typ` / tests | Нет явного `date: none`; determinism сравнивает `len()`, не байты |
| P2-C5 | `world.rs` | `file(main)` не отдаёт main, хотя `source` отдаёт — latent contract break |

---

### 4.4 P3 — долг (кратко)

- Мёртвые варианты ошибок в `typst_gen::error` и `MarkdownError::InvalidInput` (production-путь).
- `text_content` публичен и покрыт тестами, генератором не используется (зафиксировано в `progress.md`).
- `check_spans_within_source` не ходит по Link/Image spans.
- Атрибуты заголовка кроме `id` молча отбрасываются.
- Creator в PDF: `Typst $version` через default `PdfOptions`.
- Primary span only в diagnostics (trace Typst отбрасывается).
- Гейт coverage в CI — общий %, не помодульный (§17/§29).
- Репозиторий без remote: CI workflows не гонялись на GitHub.

---

## 5. Сильные стороны

1. **Чёткая слоистая архитектура** с документированными границами и ссылками на параграфы ТЗ.
2. **Markdown builder** — явный конечный автомат, типизированные кадры стека, reject HTML/math/footnotes, golden + proptest + determinism.
3. **AST** независимо от парсера и Typst; `Spanned` на блоках; валидация nested link/image, network image, tables.
4. **typst_gen** — четыре контекстных escape-функции (§23); body = вызовы шаблона, не raw markup; виртуальные пути `/mdpdf-resources/NNNNNN.ext`; реальные пути в source не попадают.
5. **compiler / MdpdfWorld** — только main + pre-registered resources + embedded fonts; `today = None`; нет пакетов/сети/системных шрифтов; magic-byte и path containment для изображений.
6. **Диагностики Typst** — user model, `generated Typst:line:col` (не имя `.md` для сгенерированного кода); `source_name` — для resource/base-dir ошибок без span (согласовано с §37 и предыдущим review-циклом).
7. **Exit codes 0–9** продуманы (включая code 9 для `ResourceAccess`).
8. **`--check`** проходит compile и не пишет PDF; warnings Typst → stderr (§38).
9. **Production без panic-path** в domain-модулях; `forbid(unsafe_code)`.
10. **Покрытие и объём тестов** (по progress: ~196 тестов; модули выше 90 % строк).

---

## 6. Покрытие тестами (наблюдение)

| Зона | Качество | Пробелы |
|---|---|---|
| markdown unit + golden | высокое | CLI path для validation exit 5 |
| typst_gen unit + golden + injection battery | высокое | metadata API, ignored fields |
| compiler unit + resource policy | высокое | symlink, SVG external, PDF byte equality |
| CLI / e2e | **низкое для DoD** | нет success-path PDF, overwrite, atomic, stdin→PDF |
| fuzz | proptest only | `cargo-fuzz` отложен на M5 |

---

## 7. Соответствие Milestone 4 / 5

### Milestone 4 (осталось для закрытия)

| Требование | Статус |
|---|---|
| Полный pipeline до PDF bytes | **есть** (compiler) |
| Атомарная запись PDF §6.4 | **нет** (stub) |
| Защита от перезаписи без `--overwrite` | **нет** |
| Сообщение `Created output.pdf` | **нет** (есть только для emit) |
| Exit 0 на happy-path | **нет** (сейчас 1) |
| E2E happy-path | **нет** |
| stdin + `-o` | чтение есть, запись PDF — через M4 |
| `--check` | **есть** |
| `--emit-ast` / `--emit-typst` | **есть** |
| Exit codes каркас | **есть** (баг validation 4 vs 5) |

### Milestone 5 (не начат; из review)

- Early enforcement лимита входа 32 MiB.
- Лимиты AST nodes / nesting depth.
- Symlink hardening (`O_NOFOLLOW` / fd) + тесты.
- SVG without external resources (pre-check).
- Golden PDF / byte-level determinism.
- `cargo-fuzz` (§50).
- Поставка / release workflow на remote.

---

## 8. Рекомендуемый план работ

### Фаза A — закрыть Milestone 4 (приоритет 1)

1. Реализовать `output::write_pdf_atomically(path, bytes, overwrite)`.
2. Вызвать из `app::run` после compile; печатать `Created …`.
3. Подключить `--overwrite` / `OutputExists` (exit 8).
4. Переписать `tests/end_to_end.rs`: success, `%PDF-`, check, overwrite, no temp leftovers.
5. Обновить `docs/progress.md`, README, устаревшие комментарии в `app.rs` / `output.rs`.

### Фаза B — быстрые correctness-фиксы (приоритет 2)

1. `AstValidation` → span + exit **5** + CLI-тест.
2. Early size limit в `source.rs` + unit-тест.

### Фаза C — hardening (Milestone 5)

1. Symlink + SVG policy.
2. AST depth/node limits.
3. Template `date: none` + byte-equal PDF test.
4. Fuzz targets, golden PDF, coverage per module.

---

## 9. Оценка рисков

| Риск | Уровень | Комментарий |
|---|---|---|
| Пользователь не получает PDF | **высокий** | P0-1, текущий master |
| Неверные exit codes в скриптах | средний | P1-1 |
| OOM на большом входе | средний | P1-2 |
| Typst injection из Markdown | **низкий** | generator + тесты |
| FS escape через image path | низкий* | containment есть; *TOCTOU/symlink — M5 |
| Утечка слоёв | низкий | один публичный `Event` в builder API |
| Недетерминизм PDF | низкий–средний | practically closed; тесты слабые |

\* Для однопользовательского CLI threat model приемлемо; для hostile concurrent FS — доработать.

---

## 10. Заключение

Кодовая база `mdpdf` на `master` (**HEAD `b009099`**) представляет собой **качественную реализацию этапов 0–3** технического задания: слои изолированы, production-код дисциплинирован (без `unsafe`/panic), генерация Typst устойчива к инъекциям, встроенный компилятор работает в ограниченном World.

**Единственный продуктовый разрыв, блокирующий DoD:** атомарная запись PDF и политика `--overwrite` (Milestone 4). Параллельно стоит исправить маппинг ошибок AST validation (exit 5 + позиция) и early limit входа.

**Рекомендация:** немедленно реализовать Фазу A (M4), затем Фазу B; Фазу C планировать как Milestone 5.

---

## Приложение A. Карта исходников

```
src/
  main.rs, lib.rs, app.rs, cli.rs, config.rs, error.rs, source.rs, output.rs
  markdown/   parser, builder, state, error
  ast/        document, block, inline, metadata, validate
  typst_gen/  generator, blocks, inlines, escape, writer, error
  compiler/   mod, world, files, fonts, pdf, diagnostics, error
assets/
  template.typ
  fonts/      Noto Sans (+ Mono) + OFL
tests/
  cli, end_to_end, markdown_parser, typst_generator, compiler
  fixtures/   markdown, expected_ast, expected_typst, images
```

## Приложение B. Связь с предыдущим review

В сессии Codex `019f9c31-6db4-7620-8687-556943c4fd77` рассматривалось замечание про `CompileInput::source_name` vs диагностики Typst. **На текущем master оно закрыто:**

- Typst-позиции → константа `diagnostics::GENERATED_TYPST` (`"generated Typst"`);
- `source_name` документирован и используется для resource/base-dir ошибок без span;
- unit-тесты на формат `generated Typst:…` присутствуют.

Этот отчёт **шире**: полный обзор всех слоёв, не только compiler diagnostics.

## Приложение C. Критерии приёмки после M4

- [ ] `mdpdf document.md -o document.pdf` → exit 0, файл `%PDF-…`, размер ≥ минимума
- [ ] Повтор без `--overwrite` → exit 8, старый PDF не повреждён
- [ ] С `--overwrite` → exit 0, PDF обновлён
- [ ] `--check` → exit 0, PDF не создаётся
- [ ] stdin + `-o` → exit 0
- [ ] Temp-файлы atomic write не остаются после success/fail
- [ ] `make ci` зелёный
- [ ] `docs/progress.md`: Milestone 4 = готов
)
