# Анализ кодовой базы `mdpdf`

| | |
|---|---|
| Дата | 2026-07-26 |
| Метод | Полный статический анализ всех слоёв (`markdown`, `ast`, `typst_gen`, `compiler`, app/CLI/IO, тесты, инфраструктура) + воспроизведение найденной паники |
| Состояние на момент анализа | Milestones 0–4 готовы, 215 тестов зелёные (`docs/progress.md`) |

Дополняет `docs/code-review-2026-07-26.md`: тот отчёт писался до закрытия
Milestone 4 и частично устарел (P0 про запись PDF уже исправлены).

## 1. Общая оценка

Кодовая база зрелая: конвейер Markdown → AST → Typst → PDF работает end-to-end,
границы слоёв соблюдены, production-код без `unsafe`/`unwrap`/`expect`/`panic!`
(единственное исключение — неявная паника в §2), `anyhow` только в тестах.

- `typst::*`/`typst_pdf::*` импортируются только в `src/compiler/`.
- `pulldown-cmark` — только в `src/markdown/` (оговорка — §4.1).
- Инъекций Typst не найдено: пользовательский текст идёт только через
  экранированные строковые литералы и типизированные вызовы шаблона.
- Детерминизм: `today() -> None`, ресурсы в `BTreeMap`, `HashMap` в World без
  итерации, `PdfOptions::default()` не пишет timestamp — чисто; слабое место
  только в тесте (§5).
- Сети, внешних процессов, системных шрифтов, домашнего каталога — нет.

## 2. Критично (P0)

### Паника на пользовательском вводе — `src/ast/validate.rs:232`

```rust
trimmed.len() >= scheme.len() && trimmed[..scheme.len()].eq_ignore_ascii_case(scheme)
```

`trimmed[..scheme.len()]` режет `str` не на границе символа. Документ
`![a](dat€:x)`: `€` занимает байты 3..6, индекс 5 (длина `"data:"`) — внутри
символа → паника `byte index 5 is not a char boundary`.

**Воспроизведено** отдельным прогоном — падение подтверждено. Путь до кода:
`![a](dat€:x)` → `MarkdownParser::parse` → `validate_document` →
`is_network_source` (`validate.rs:206`). Прямое нарушение инварианта «никаких
паник» и ТЗ §17. Proptest этот случай не ловит: в его алфавите нет
многобайтовых символов рядом со «схемоподобными» префиксами.

Исправление (одна строка):

```rust
trimmed.get(..scheme.len()).is_some_and(|p| p.eq_ignore_ascii_case(scheme))
```

## 3. Важное (P1)

1. **Неограниченная рекурсия по глубине AST** — `typst_gen` рекурсивен
   (`inlines.rs:21`, `blocks.rs:42,66`), парсер глубину не ограничивает, а
   `MAX_NESTING_DEPTH = 128` (`src/compiler/mod.rs:96`) нигде не применяется.
   Глубоко вложенный документ → переполнение стека и abort, а не `Err`.
   Задокументировано как долг Milestone 5, но это реальный вектор отказа.
2. **Лимит входа 32 MiB проверяется после полного чтения**
   (`src/source.rs:54-66`) — файл читается в память целиком до проверки в
   `decode_and_normalize` (`source.rs:75`). Для файла можно смотреть
   `metadata().len()` до чтения; для stdin — `Read::take(limit+1)`.
3. **`make golden-update` не обновляет Typst-голдены** — `Makefile:60-61`
   запускает только `--test markdown_parser` (AST-фикстуры), при этом сообщение
   об ошибке в `tests/typst_generator.rs:285` советует именно эту команду.
   Golden-тесты генератора она не починит.

## 4. Формальные нарушения инвариантов (P2)

1. **Утечка `pulldown-cmark` в публичный API** — `AstBuilder::handle(Event)`
   (`src/markdown/builder.rs:51`) торчит наружу через `pub mod builder` →
   `pub mod markdown` (`src/lib.rs:25`). ТЗ §15 и док-комментарий
   `src/markdown/mod.rs:4` утверждают обратное. Утечка «легализована» тестом
   инвариантов в `tests/markdown_parser.rs:443`. Лечится `pub(crate)` +
   переносом тестов в юнит-тесты внутри `src/markdown/`.
2. **`is_network_source` не ловит protocol-relative адреса `//host/x.png`**
   (`validate.rs:229`) — не уязвимость (сети в программе нет), но диагностика
   по ТЗ §10.12 неполная.
3. **`path_literal` не требует префикса `/mdpdf-resources/`**
   (`src/typst_gen/escape.rs:119-146`) — принимает любой абсолютный путь без
   `..` и `\`. Сейчас безопасно: единственный вызов получает путь, построенный
   самим генератором (`inlines.rs:65`), но контракт слабее заявленного в
   docstring.
4. **`check_spans_within_source`** (`src/markdown/parser.rs:120-144`) рекурсирует
   только в `Quote` и `List`; span-ы ссылок/изображений и содержимое ячеек
   таблиц на выход за пределы источника не проверяются.

## 5. Мелочи и мёртвый код

- Мёртвые варианты ошибок: `MarkdownError::InvalidInput`
  (`src/markdown/error.rs:12-18`), `AppError::AstValidation` и
  `AppError::ResourcePolicy` (`src/error.rs:103-104, 157-161`),
  `ExitStatus::GeneralError`, 4 из 7 вариантов `TypstGenerationError`.
- Неиспользуемый production-код: `text_content` (`src/typst_gen/escape.rs:69`,
  требуется §23 — осознанно), `errors_only` (`src/compiler/pdf.rs:67`).
- `config.rs:95` — `paper` через `format!("{:?}").to_lowercase()`, хрупко к
  переименованию варианта; надёжнее явный `match`.
- `src/output.rs:103-126` — при цели-каталоге на Unix ветка rollback может
  переименовать чужой каталог в `*.backup`; нет fsync каталога после rename.
- `MAX_URL_BYTES` дублируется в `escape.rs:149` и `compiler/mod.rs:104` — два
  источника истины.
- `src/compiler/files.rs:24` обещает «SVG без внешних ресурсов», проверки нет
  (практически безвредно: usvg сети не имеет, относительные href упрутся в
  `World::file` → `NotFound`).
- Тест детерминизма PDF сравнивает только длины
  (`tests/compiler.rs:230-238`) — нужно побайтовое сравнение.
- Молча отбрасываются `Heading.id`, `Link.title`, `Image.title`,
  `DocumentMetadata` целиком (title/author берутся только из `RenderOptions`);
  ссылка `[x](#anchor)` эмитится как `link()` без соответствующего label —
  Typst выдаст warning «unknown label».
- TOCTOU-окно в `src/compiler/files.rs:165→172→187` (canonicalize → metadata →
  read); ущерб ограничен суммарным лимитом ресурсов, при локальной модели
  угроз — низкий риск, закрывается в Milestone 5.
- Эвристика SVG (`files.rs:208-215`): только первые 1024 байта, `<svgfoo` —
  ложноположительное срабатывание; последствие — лишь понятная ошибка.
- Архитектурное наблюдение: компилятор импортирует `ResourceReference` из
  `typst_gen` (`src/compiler/mod.rs:19`) — обратная зависимость
  compiler → typst_gen; таблица границ AGENTS.md это не запрещает, но связь
  стоит иметь в виду.

## 6. Пробелы в тестах

- Нет сквозных CLI-тестов флагов `--paper letter`, `--toc`,
  `--heading-numbers`, `--margin`, `--font-size`, `--verbose` — проверяется
  только наличие в `--help` (`tests/cli.rs:16-33`); невалидные
  `--margin`/`--font-size` (код выхода 6) через бинарь не прогоняются.
- Лимиты `MAX_IMAGE_BYTES` и 32 MiB входа не покрыты тестами.
- Нет end-to-end теста с реальным SVG-изображением.
- Ветки предупреждений Typst (непустой `CompiledPdf.warnings`, печать в
  `app.rs`) и подсказок `hints` в диагностиках не тестируются; на пути ошибки
  предупреждения теряются (`src/compiler/pdf.rs:33-43`).
- Все 4 Typst-голдены с дефолтными `RenderOptions` — ветки `toc: true`,
  `us-letter`, непустые title/author golden-покрытием не имеют.
- `markdown_soup_never_panics` (`tests/markdown_parser.rs:422`) без
  многобайтовых символов рядом с префиксами схем — именно комбинация из §2.
- `template_defines_every_function_the_generator_calls`
  (`tests/typst_generator.rs:81`) — строковый поиск `#let name(`, не проверяет
  сигнатуры; несоответствие аргументов всплывёт только на компиляции.

## 7. Рекомендации

1. Сразу: починить панику в `validate.rs:232` + регрессионный тест с
   не-ASCII адресом изображения.
2. В рамках Milestone 5 (уже запланирован): лимит глубины и числа узлов AST,
   ранняя проверка размера входа, побайтовое сравнение PDF, `pub(crate)` для
   builder-а, symlink/SVG hardening.
3. Попутно: расширить `make golden-update` на `tests/typst_generator.rs`;
   заменить Debug-маппинг `paper` на `match`; свести `MAX_URL_BYTES` к одной
   константе.
