# Mermaid через `mermaid-rs-renderer`: решение и план реализации

**Статус:** утверждённая политика (ТЗ §10.5, `deny.toml`); runtime **не**
внедрён.  
**Дата:** 2026-07-30  
**Норматив:** [`mdpdf-technical-spec-v2.md`](mdpdf-technical-spec-v2.md) §10.5,
§11.3, §15, §22; [`../deny.toml`](../deny.toml); [`../AGENTS.md`](../AGENTS.md).

---

## 1. Зачем этот документ

Зафиксировать **решение**, **отклонённые альтернативы** и **полный план
внедрения** конвейера:

```text
mermaid-блок  →  mermaid-rs-renderer  →  SVG  →  Typst #image  →  PDF
```

Документ — рабочий план для реализации. При расхождении с ТЗ приоритет у ТЗ;
этот файл уточняет «как режем работу», а не подменяет требования.

---

## 2. Контекст и проблема

### 2.1. Текущее состояние (legacy)

| Что | Сейчас |
|-----|--------|
| Вход | fenced code ` ```mermaid ` |
| AST | обычный `CodeBlock` |
| Рендер | `src/mermaid/` (свой parser + longest-path layout) |
| Typst | `mdpdf-diagram(...)` — `place`/`rect`/`line` |
| Качество | иерархия после фиксов читаема; визуально далеко от Mermaid.js |
| ТЗ/deny | **уже** описывают целевой путь mmdr; код — ещё legacy |

Проблемы legacy на реальных документах (например `03-architecture.md`):

- прямые рёбра без routing;
- подписи рёбер наезжают на узлы;
- subgraph/циклы ломали слои (частично починено);
- узкое подмножество типов (по сути flowchart + sequence).

### 2.2. Что нужно продукту

- Диаграммы **как у нормального Mermaid-рендерера** (layout, subgraph, стрелки),
  а не «чертёж из примитивов».
- Сохранить инварианты `mdpdf`: **один бинарь**, **без сети**, **без внешних
  процессов**, **без Chromium/JS**, детерминизм, `cargo deny`, слои архитектуры.

---

## 3. Решение

### 3.1. Выбранный путь

Использовать crates.io-крейт
[`mermaid-rs-renderer`](https://crates.io/crates/mermaid-rs-renderer)
(CLI-имя **mmdr**, MIT, pure Rust):

```text
Markdown
  → AST (CodeBlock lang=mermaid)          # без изменений
  → typst_gen:
       mmdr::render(src) → SVG bytes
       register in-memory resource
       emit mdpdf-image(path: "/mdpdf-resources/mermaid-NNNNNN.svg")
  → compiler: отдать байты в MdpdfWorld
  → typst_pdf → PDF
```

Свойство: **не** Mermaid.js, а **нативная** реализация parse/layout/SVG.
Без Node, без Puppeteer, без V8.

### 3.2. Отклонённые альтернативы

| Вариант | Почему нет |
|---------|------------|
| Допилить legacy layout «до dagre» | месяцы; всё равно не 23 типа; конкурируем с mmdr |
| V8/QuickJS + Mermaid.js | stock Mermaid требует **DOM**/измерение текста; без Chromium-класса среды нет паритета; ТЗ/deny запрещают |
| mermaid-cli / headless Chromium | внешний runtime, размер, запрет Chromium в ТЗ §2 |
| Pre-render SVG руками в docs | не решает `mdpdf` как продукт |
| Git-зависимость mmdr | `unknown-git = deny` |

### 3.3. Уже сделано (политика)

- [x] ТЗ §10.5 переписан под mmdr → SVG → image  
- [x] `deny.toml`: ban JS-runtime / WebDriver / Chromium crates; ban features
  `cli`/`png`/`benchmark` у mmdr; `Unlicense` для regex-стека  
- [x] `AGENTS.md`, README (статус), CHANGELOG, progress  

### 3.4. Не сделано (runtime)

- [ ] зависимость в `Cargo.toml`  
- [ ] in-memory ресурсы  
- [ ] вызов mmdr из `typst_gen`  
- [ ] security + лимиты SVG  
- [ ] детерминизм шрифтов  
- [ ] тесты / golden  
- [ ] удаление legacy  

---

## 4. Целевая архитектура

### 4.1. Границы слоёв (AGENTS.md)

| Модуль | Знает | Не знает |
|--------|--------|----------|
| `markdown` | pulldown-cmark, ast | mermaid, Typst, mmdr |
| `ast` | — | mermaid, Typst |
| `mermaid` (новый смысл) | обёртка mmdr, ошибки, лимиты исходника | pulldown-cmark, файловая система, PDF |
| `typst_gen` | ast, mermaid API, регистрация resource | компилятор, FS (кроме логических путей) |
| `compiler` | typst, виртуальные файлы, проверка SVG | Markdown, mermaid syntax |

**Запрет:** `typst::*` только в `compiler/`.  
**Запрет:** mmdr не вызывается из `markdown/` или `compiler/` напрямую —
только из `typst_gen` (или тонкого `mermaid` facade, который зовёт typst_gen).

Рекомендуемая раскладка после миграции:

```text
src/mermaid/
  mod.rs          # pub: render_to_svg(src) -> Result<Vec<u8>, MermaidError>
  error.rs        # thiserror; map из anyhow/ошибок mmdr
  limits.rs       # MAX_SOURCE_BYTES, MAX_SVG_BYTES
  # legacy parser/layout/model — удалить после фазы D

src/typst_gen/
  diagram.rs      # diagram_expression: mmdr → resource + mdpdf-image
  generator.rs    # ResourceReference с optional bytes
```

### 4.2. Поток данных

```text
blocks::code_expression(lang=mermaid)
    │
    ├─ Ok(svg_bytes)
    │     resources.push(ResourceReference {
    │         logical_path: "/mdpdf-resources/mermaid-000001.svg",
    │         source_path: "",          # или sentinel
    │         kind: Image,
    │         bytes: Some(svg_bytes),   # NEW
    │         span: ...
    │     })
    │     → "mdpdf-image(path: \"/mdpdf-resources/mermaid-000001.svg\", alt: \"diagram\")"
    │
    └─ Err(e)
          warnings.push(...)
          → обычный raw block / code block
```

Компилятор:

```text
resolve_resources(refs):
  for ref in refs:
    if ref.bytes is Some:
      validate_svg_security(bytes)
      check size limits
      insert into ResolvedResources
    else:
      read from disk as today
```

### 4.3. Typst surface

- Переиспользовать **`mdpdf-image`** (уже есть): `box(image(path, alt: alt))`.
- При необходимости: обёртка `mdpdf-diagram-image` с `width: 100%` и
  `block(breakable: false)` — чтобы диаграмма вела себя как сейчас
  (не рвать страницу). Предпочтительно **не** плодить API без нужды:
  расширить `mdpdf-image` опциональным `width` / `breakable`.

Legacy **`mdpdf-diagram`** (примитивы) удаляется из шаблона после миграции
тестов, чтобы не тащить мёртвый код.

### 4.4. Версия зависимости

| Параметр | Значение |
|----------|----------|
| Crate | `mermaid-rs-renderer` |
| Стартовая версия | `0.3.1` (или новее patch на момент внедрения, crates.io) |
| Features | `default-features = false` (**обязательно**) |
| Источник | только crates.io (`Cargo.lock`) |
| API | `render` / `render_with_options` → `String` SVG |

Закрепление exact/caret — как у прочих non-typst deps; lockfile в git.

---

## 5. Ключевые технические решения

### 5.1. In-memory ресурсы (обязательный plumbing)

Сейчас `ResourceReference` всегда читается с диска (`source_path` + base dir).
Для mmdr диск не нужен и даже вреден (временные файлы, гонки, FS policy).

**Изменение модели:**

```rust
pub struct ResourceReference {
    pub logical_path: String,
    /// Для дисковых картинок — путь из Markdown; для mmdr — пусто.
    pub source_path: String,
    /// Если Some — байты уже в памяти; FS не трогать.
    pub embedded_bytes: Option<Vec<u8>>,
    pub kind: ResourceKind,
    pub span: Option<SourceSpan>,
}
```

Инварианты:

- ровно один источник: либо `embedded_bytes`, либо читаемый `source_path`;
- `logical_path` уникален, последовательный (`000001`, …) **или**
  отдельный namespace `mermaid-000001.svg` — главное **детерминированный**
  порядок (порядок обхода AST);
- лимиты `MAX_IMAGES` / `MAX_TOTAL_IMAGE_BYTES` / per-image size применяются
  и к embedded SVG.

### 5.2. Безопасность SVG

Уже есть: `detect_format`, `looks_like_svg`, `svg_external_reference`.

Для mmdr-выхода:

1. Прогнать **тот же** `svg_external_reference` (http/https/file/data).
2. При нарушении — **не panic**: fallback code block + warning (или
   `CompileError::Image` — выбрать одно и зафиксировать в тестах;
   **предпочтение:** fallback + warning, чтобы документ с одной
   «плохой» диаграммой всё же собирался, согласовано с §10.5.5).
3. Не писать SVG пользователя/mmdr в Typst как raw markup — только через
   `#image` / virtual file.

### 5.3. Детерминизм и шрифты

**Риск:** mmdr использует `fontdb` и может сканировать системные шрифты →
разная геометрия на Ubuntu vs macOS.

**План проверки (фаза B):**

1. Два прогона `render()` одной и той же диаграммы на CI matrix — сравнить
   SVG (нормализованный: без лишних float noise, если есть).
2. Если плавает — искать API mmdr: font config, `fastText`, disable system
   fonts, embed font bytes.
3. Целевой инвариант: **structural golden PDF** стабилен на всех платформах
   CI; идеал — **идентичный SVG** при фиксированном font set.

Если mmdr **не** позволяет зафиксировать шрифты без форка:

- зафиксировать в progress как known limitation;
- golden PDF только structural (как сейчас);
- не блокировать 1.0, но документировать;
- issue upstream mmdr / optional vendor patch — отдельное решение.

### 5.4. Ошибки и fallback

| Вход | Результат |
|------|-----------|
| mmdr Ok + SVG clean | image |
| mmdr Err (syntax/type) | code block + stderr warning |
| mmdr Ok + SVG fails security | code block + warning |
| source > 64 KiB | code block + warning (или до mmdr не звать) |
| SVG > 16 MiB | code block + warning |
| panic в mmdr | **не допускается** наружу: `catch_unwind` только если mmdr
  не гарантирует no-panic; предпочтительно полагаться на Result.
  Если catch_unwind — строго documented, count как Err |

Маппинг ошибок mmdr (`anyhow` внутри crate) → `MermaidError` / строка warning
через `Display`, без протекания `anyhow` в публичный API mdpdf.

### 5.5. Нумерация виртуальных путей

Сохранить единый счётчик изображений:

- markdown `![](a.png)` → `/mdpdf-resources/000001.png`
- mermaid → `/mdpdf-resources/000002.svg` (тот же счётчик)

Либо отдельный префикс `mermaid-` — **не** смешивать расширения без
нужды: расширение `.svg` обязательно для ясности и `detect_format`.

Порядок: document order (depth-first блоков), стабильный.

### 5.6. Масштаб на странице

- `mdpdf-image` / `image(..., width: 100%)` — вписать в ширину текста.
- Высота: Typst масштабирует SVG пропорционально; если нужна гарантия
  «не выше страницы» — `block(breakable: false)` + max-height через
  measure **или** оставить как у обычных картинок (допустимо по ТЗ:
  «как крупные изображения»).

Legacy `fit` для `mdpdf-diagram` **не** переносится 1:1; упростить до
width 100%.

---

## 6. План реализации (фазы)

Каждая фаза — вертикальный срез с критерием приёмки. Можно PR-ами по фазам.

### Фаза 0 — Spike (подтверждение)

**Цель:** mmdr + наши диаграммы + SVG security + оценка шрифтов.

Задачи:

1. Временный binary/example или `tests/mmdr_spike.rs` (не в prod path).
2. Прогнать:
   - `tests/fixtures/markdown/mermaid.md`;
   - 4 блока из `03-architecture.md` (ТКМ), если доступен;
   - кириллица, subgraph, sequence alt/Note.
3. Сохранить SVG в `target/` / tmp; глазами сравнить с legacy PDF.
4. Проверить `svg_external_reference` на выходе mmdr.
5. Два прогона SVG hash на одной машине; отметить float variance.

**Приёмка:**

- [ ] flowchart + sequence + subgraph с кириллицей → SVG без Err  
- [ ] SVG проходит security-check  
- [ ] короткий отчёт в progress: «spike ok / blockers»  

**Не мержить** в прод-путь; spike можно удалить после фазы A.

---

### Фаза A — Зависимость и deny

**Цель:** mmdr в графе зависимостей легально.

Задачи:

1. `Cargo.toml`:
   ```toml
   mermaid-rs-renderer = { version = "0.3.1", default-features = false }
   ```
2. `cargo update -p mermaid-rs-renderer` / lockfile.
3. `cargo deny check` (features ban должен сработать).
4. `cargo check` — пока без вызовов (или `use` только в `mermaid` stub).
5. CHANGELOG: dependency added (Unreleased).

**Приёмка:**

- [ ] `make ci` / как минимум check + deny  
- [ ] в lockfile нет `cli`/`png` feature activation для mmdr  
- [ ] размер release измерить (записать в progress)  

---

### Фаза B — In-memory resources

**Цель:** compiler принимает байты без FS.

Задачи:

1. Расширить `ResourceReference` полем `embedded_bytes: Option<Vec<u8>>`.
2. `files::resolve_resources`: ветка embedded → validate format + SVG policy +
   limits → `ResolvedResources`.
3. Unit-тесты:
   - plain SVG embedded accepted;
   - SVG with `https://` rejected;
   - size limit;
   - disk path still works for png.
4. Не ломать существующие image fixtures.

**Приёмка:**

- [ ] все старые image tests green  
- [ ] новые unit tests на embedded SVG  
- [ ] нет записи temp files  

---

### Фаза C — Интеграция mmdr в генератор

**Цель:** ` ```mermaid ` → image в PDF.

Задачи:

1. `src/mermaid/mod.rs` (или новый facade):
   - `pub fn render_svg(source: &str) -> Result<Vec<u8>, MermaidError>`
   - проверка `source.len() ≤ MAX`
   - вызов `mermaid_rs_renderer::render` / `render_with_options`
   - `svg.into_bytes()`, проверка `MAX_SVG_BYTES`
2. `typst_gen/diagram.rs`:
   - вместо parse/layout/mdpdf-diagram → render_svg + register resource
   - сигнатура: доступ к `&mut Vec<ResourceReference>`
3. `blocks::code_expression` — прокинуть `resources` (сейчас только
   warnings/options; **потребуется** расширить API генерации блоков).
4. Typst: `mdpdf-image` с `width: 100%` (правка template при необходимости).
5. stderr warning path без изменений по смыслу.
6. Убрать использование `mdpdf-diagram` из happy path.

**Приёмка:**

- [ ] `mdpdf tests/fixtures/markdown/mermaid.md -o /tmp/m.pdf` — PDF с
  изображениями (не code blocks) для валидных диаграмм  
- [ ] gantt/unknown → code block + warning  
- [ ] `--emit-typst` содержит `mdpdf-image` / `image(` и virtual `.svg`  
- [ ] end-to-end test в `tests/`  

---

### Фаза D — Тесты, golden, лимиты, fuzz

**Цель:** регрессионная сетка под новый путь.

Задачи:

1. Обновить `tests/fixtures/expected_typst/mermaid.typ` (больше не
   `mdpdf-diagram` tuples).
2. `expected_pdf/mermaid.json` — число images ≥ 1, structural.
3. `expected_ast` — без изменений (AST тот же CodeBlock), проверить.
4. Новые unit tests: fallback, limit source, security reject.
5. Fuzz: заменить/дополнить `fuzz_mermaid_parser` → fuzz на
   `render_svg` + «не паникует» (или оставить legacy fuzz до удаления кода).
6. Лимиты в `limits` module + ТЗ §15 уже описаны.
7. Ручная проверка: architecture-like fixture в repo (скопировать 1–2
   диаграммы в `tests/fixtures/markdown/architecture_mermaid.md`).

**Приёмка:**

- [ ] `cargo test --all-targets --all-features` green  
- [ ] `make golden-update` **только** после явного решения; новые golden
  закоммичены  
- [ ] CI matrix green  

---

### Фаза E — Детерминизм шрифтов (жёсткая)

**Цель:** максимально стабильная геометрия.

Задачи:

1. Исследовать API mmdr 0.3.x: theme, fontFamily, fastText, font db.
2. Зафиксировать config в `render_with_options` (детерминированные
   themeVariables, fontSize).
3. CI job или test: SVG snapshot (hash) optional feature `mermaid-svg-snap`
   только на linux если mac плавает — **не** желательно; лучше единый SVG.
4. Документировать итог в progress.

**Приёмка:**

- [ ] зафиксированный config в коде  
- [ ] progress: «детерминизм: полный / partial / limitation»  
- [ ] если limitation — issue + не регрессировать structural PDF  

---

### Фаза F — Удаление legacy

**Цель:** нет мёртвого dual-path.

Задачи:

1. Удалить `src/mermaid/parser.rs`, `layout.rs`, `model.rs` (или весь
   legacy), `mdpdf-diagram` из `assets/template.typ`.
2. Удалить unit tests, заточенные под placed boxes.
3. Упростить `diagram.rs` до thin wrapper.
4. Fuzz target rename.
5. README: убрать «Статус: интеграция отдельно».
6. progress: legacy removed.

**Приёмка:**

- [ ] `rg mdpdf-diagram` — пусто (кроме CHANGELOG/history)  
- [ ] `rg assign_layers|PlacedDiagram` — пусто  
- [ ] размер бинаря не вырос сверх ожидаемого от mmdr (legacy code gone)  
- [ ] `cargo deny check` + full CI  

---

### Фаза G — Документация и release-готовность

**Цель:** закрыть петлю для 1.0.

Задачи:

1. README: полный user-facing текст (типы = mmdr, fallback).
2. ТЗ: убрать формулировки «legacy layout на время миграции» если код удалён.
3. `docs/progress.md`, CHANGELOG user-visible.
4. Ручной чеклист releasing: mermaid fixtures + architecture sample.
5. Оценка binary size / perf (1 diagram, 50 diagrams) в progress.

**Приёмка:**

- [ ] docs/README index ссылается на этот план как historical/approved  
- [ ] releasing checklist включает mermaid  
- [ ] нет расхождения ТЗ ↔ код  

---

## 7. Порядок PR (рекомендация)

| PR | Содержание | Риск |
|----|------------|------|
| PR1 | Фаза A (dep + lock + deny already ok) | низкий |
| PR2 | Фаза B (embedded resources) | средний (compiler) |
| PR3 | Фаза C (mmdr wire-up) + минимальные tests | высокий (поведение) |
| PR4 | Фаза D golden + fixtures | средний |
| PR5 | Фаза E fonts | средний |
| PR6 | Фаза F delete legacy | средний (большой diff) |
| PR7 | Фаза G docs polish | низкий |

Фазы 0 можно не PR-ить (локальный spike). PR2 и PR3 можно объединить, если
ревью позволяет.

---

## 8. Критерии готовности всей фичи

«Mermaid mmdr done» когда:

1. Happy path: valid mermaid → SVG image в PDF.  
2. Fallback: invalid → code + warning, exit code 0.  
3. Нет JS/Chromium/process/network.  
4. `cargo deny check` green с mmdr features ban.  
5. `make ci` green на трёх ОС.  
6. Legacy `mdpdf-diagram` удалён.  
7. ТЗ §10.5 = код.  
8. Binary size и perf зафиксированы в progress.  
9. Ручная визуальная проверка 2–3 документов (в т.ч. subgraph + sequence).

---

## 9. Риски и митигации

| Риск | Вероятность | Митигация |
|------|-------------|-----------|
| mmdr не детерминирует шрифты | средняя | фаза E; structural golden; upstream |
| mmdr panic на злом вводе | низкая/средняя | fuzz; catch_unwind только как last resort |
| mmdr SVG с external ref | низкая | security pipeline |
| Рост бинаря +5–15 МБ | средняя | default-features false; measure |
| Breaking change visual vs legacy golden | высокая | golden-update осознанно |
| mmdr 0.3.x early, API drift | средняя | pin version; upgrade as task |
| `anyhow` в mmdr | низкая | map at boundary |
| Edition 2024 у mmdr | низкая | mdpdf уже 2024 |

---

## 10. Тест-матрица (сводка)

| Уровень | Что |
|---------|-----|
| Unit | embedded resource; security; limits; fallback mapping |
| Integration | mermaid.md → PDF has images; emit-typst paths |
| Golden AST | без изменений (CodeBlock) |
| Golden Typst | image() вместо mdpdf-diagram |
| Golden PDF | page count, image count, fonts |
| Fuzz | render_svg no panic |
| Manual | architecture fixture, dense flowchart, long labels |
| Deny | features ban, no git |

---

## 11. Изменения по файлам (ориентир)

| Область | Файлы |
|---------|--------|
| deps | `Cargo.toml`, `Cargo.lock` |
| mermaid facade | `src/mermaid/*` (rewrite) |
| gen | `src/typst_gen/diagram.rs`, `blocks.rs`, `generator.rs` |
| compiler | `src/compiler/files.rs`, возможно `world.rs` |
| template | `assets/template.typ` |
| limits | `src/.../limits` если есть, иначе const в mermaid |
| tests | `tests/fixtures/**`, `tests/typst_generator.rs`, e2e |
| docs | ТЗ (мелкие правки после F), README, progress, CHANGELOG |
| remove | legacy mermaid layout/parser tests, mdpdf-diagram |

---

## 12. Оценка трудозатрат (грубо)

| Фаза | Оценка |
|------|--------|
| 0 Spike | 0.5–1 день |
| A Dep | 0.5 дня |
| B Resources | 1 день |
| C Integration | 1–2 дня |
| D Tests/golden | 1–2 дня |
| E Determinism | 0.5–2 дня (зависит от mmdr API) |
| F Delete legacy | 0.5–1 день |
| G Docs | 0.5 дня |
| **Итого** | **~5–10 рабочих дней** |

Один разработчик, последовательно; PR2–3 критический путь.

---

## 13. Что не входит в этот план

- Побайтовый паритет с mermaid-cli / GitHub.  
- Все 28+ типов «как в каталоге Mermaid.js», если mmdr их не умеет.  
- Интерактив / click / live preview.  
- PNG-экспорт диаграмм.  
- Параллельный dual-engine (legacy + mmdr) в релизе — только краткая
  миграция внутри PR, не публичный флаг.

---

## 14. Следующий конкретный шаг

1. **Фаза 0 (spike)** локально: mmdr на fixtures + architecture blocks,
   отчёт в progress (1 абзац).  
2. **PR1 = Фаза A** (зависимость).  
3. **PR2 = B+C** или раздельно — по результатам spike (если mmdr API
   неудобен, скорректировать фазу C).

Реализация кода **не** начинается в этом документе: документ — вход в
execute-plan / ручную работу.

---

## 15. История

| Дата | Событие |
|------|---------|
| 2026-07-30 | Политика ТЗ + deny; выбран mmdr вместо JS/Chromium и вместо «вечного» legacy layout |
| 2026-07-30 | Настоящий план реализации |
