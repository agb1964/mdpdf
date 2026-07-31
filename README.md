# mdpdf

Автономная консольная утилита: Markdown → PDF.

```text
Markdown → собственное AST → Typst source → встроенный Typst compiler → PDF
```

Один исполняемый файл. Не требует установленного Typst, Chromium, LaTeX или Pandoc.
Не обращается к сети и не запускает внешние процессы. Шрифты и шаблон оформления
встроены в бинарник.

## Установка

Готовые бинарники публикуются на странице
[GitHub Releases](https://github.com/agb1964/mdpdf/releases):

| Платформа | Архив |
|---|---|
| macOS Apple Silicon | `mdpdf-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `mdpdf-x86_64-apple-darwin.tar.gz` |
| Linux x86-64 | `mdpdf-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `mdpdf-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86-64 | `mdpdf-x86_64-pc-windows-msvc.zip` |

Распакуйте архив и поместите `mdpdf` (`mdpdf.exe` на Windows) в каталог из
`PATH`. Typst и дополнительные runtime-компоненты устанавливать не требуется.

## Сборка

Для сборки из исходников нужен стабильный Rust:

```bash
cargo build --release
```

Бинарник: `target/release/mdpdf`.

Частые команды — в `Makefile`: `make` печатает список, `make ci` прогоняет
проверки, которые должны выполняться перед коммитом.

## Использование

```bash
mdpdf input.md
```

Без `--output` PDF пишется рядом с исходником: `input.md` → `input.pdf`.

```bash
mdpdf input.md --output output.pdf
mdpdf input.md -o output.pdf
cat input.md | mdpdf - --output output.pdf
mdpdf input.md --check
mdpdf input.md --emit-typst document.typ
mdpdf input.md --emit-ast ast.json
```

- При чтении из stdin (`-`) параметр `--output` **обязателен**.
- `--check` проходит весь конвейер (включая компиляцию) и **не** записывает PDF.
- Если указан только `--emit-ast` или `--emit-typst` без `--output`, программа
  записывает запрошенный промежуточный результат и завершается **без** PDF.
- Существующий выходной файл не перезаписывается без `--overwrite`.

### Параметры

```text
mdpdf [OPTIONS] <INPUT>

Arguments:
  <INPUT>                      Markdown-файл или "-" для stdin

Options:
  -o, --output <FILE>          Выходной PDF
      --title <TEXT>           Заголовок документа
      --author <TEXT>          Автор
      --paper <PAPER>          a4 или letter [default: a4]
      --margin <LENGTH>        Поля страницы [default: 20mm]
      --font-size <LENGTH>     Размер основного текста [default: 11pt]
      --toc                    Создать оглавление
      --heading-numbers        Нумеровать заголовки
      --check                  Проверить документ без записи PDF
      --emit-ast <FILE>        Записать AST в JSON
      --emit-typst <FILE>      Записать сгенерированный Typst
      --overwrite              Разрешить замену выходного файла
      --quiet                  Не выводить сообщение об успехе
      --verbose                Расширенная диагностика
  -h, --help                   Справка
  -V, --version                Версия
```

## Поддерживаемый Markdown

- заголовки 1–6, абзацы;
- жирный, курсив, зачёркивание, inline code;
- fenced code blocks (язык — подпись, без подсветки синтаксиса);
- маркированные и нумерованные списки, вложенные списки, task lists;
- цитаты (в том числе вложенные);
- ссылки и локальные изображения;
- таблицы, горизонтальные разделители;
- кириллица и Unicode;
- диаграммы Mermaid в code blocks с языком `mermaid` (см. ниже).

YAML front matter допускается: распознаётся и отбрасывается, метаданные из него
не извлекаются.

В бинарник встроен Noto Color Emoji. Если символ отсутствует во всех
встроенных шрифтах, `mdpdf` выводит предупреждение в stderr и продолжает
создание PDF; системный шрифт для подстановки не используется.

Не поддерживается: HTML, JavaScript, математика, сноски, библиография,
PlantUML, сетевые изображения, пользовательский Typst-код, пакеты и
шаблоны, PDF/A, PDF/UA, подпись и шифрование PDF.

## Диаграммы Mermaid

Конвейер (ТЗ §10.5):

```text
mermaid-блок  →  mermaid-rs-renderer (Rust)  →  SVG  →  Typst image  →  PDF
```

Без JavaScript, Chromium, Node.js и внешних процессов. Движок —
[`mermaid-rs-renderer`](https://crates.io/crates/mermaid-rs-renderer) (library
API, `default-features = false`). Неподдерживаемый синтаксис или ошибка
рендера не прерывают сборку: блок выводится как обычный код, в stderr —
предупреждение.

Типы диаграмм — те, что принимает закреплённая в `Cargo.lock` версия mmdr
(flowchart, sequence, class, state, ER, gantt и др.). Подробности и
ограничения безопасности SVG — в `docs/mdpdf-technical-spec-v2.md` §10.5.

Известные расхождения с mermaid.js:

- диаграмма деградирует до блока кода, если в SVG появляется ссылка на
  внешний ресурс — так ведёт себя `click A "https://…"` (ТЗ §33.3);
- в sequence-диаграмме авто-создание участников работает, только пока не
  объявлен **ни один** `participant`; если объявлен хотя бы один, ссылка на
  необъявленное имя считается опечаткой и диаграмма деградирует;
- подпись ребра большой длины может наезжать на узел: mmdr не резервирует
  под неё место в раскладке;
- геометрия не совпадает с mermaid-cli побайтово — паритет не является целью.

## Изображения

Пути разрешаются относительно каталога Markdown-файла (при stdin — относительно
текущего каталога). Поддерживаются PNG, JPEG, GIF и SVG без внешних ресурсов;
формат определяется по содержимому.

Отклоняются:

- сетевые URL и `data:` URI;
- protocol-relative адреса (`//host/...`);
- пути за пределами каталога документа (в том числе через `..` и symlink);
- SVG со ссылками на http/https/file/data.

В Typst source попадают только виртуальные пути вида `/mdpdf-resources/000001.png`.

## Коды завершения

| Код | Значение |
|---|---|
| 0 | успех |
| 1 | общая ошибка выполнения |
| 2 | ошибка аргументов CLI |
| 3 | ошибка чтения входа |
| 4 | ошибка Markdown |
| 5 | ошибка AST validation |
| 6 | ошибка генерации Typst |
| 7 | ошибка компиляции Typst |
| 8 | ошибка записи результата |
| 9 | нарушение политики доступа к ресурсу |

## Документация

| Файл | Содержание |
|---|---|
| `docs/README.md` | Карта актуальной и исторической документации |
| `docs/mdpdf-technical-spec-v2.md` | Техническое задание, редакция 2.0 |
| `docs/progress.md` | Журнал работ, решения, готовность к 1.0 |
| `CONTRIBUTING.md` | Локальная разработка и проверки |
| `docs/releasing.md` | Чек-лист выпуска |
| `SECURITY.md` | Модель угроз и сообщение об уязвимостях |
| `CHANGELOG.md` | Пользовательские изменения по версиям |
| `AGENTS.md` | Инварианты для разработки |

## Статус

Первый технический выпуск
[`v0.1.0`](https://github.com/agb1964/mdpdf/releases/tag/v0.1.0) опубликован.
GitHub CI подтверждён на Ubuntu, macOS и Windows; release workflow создаёт
бинарники для пяти target. Версия **1.0** пока не объявлена.

## Лицензии

Код — MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).

Встроенные Noto Sans и Noto Sans Mono — SIL Open Font License 1.1
(`assets/fonts/OFL.txt`); Noto Color Emoji — SIL Open Font License 1.1
(`assets/fonts/LICENSE-NotoColorEmoji.txt`). Версии, контрольные суммы и
процедура обновления перечислены в `assets/fonts/README.md`.
