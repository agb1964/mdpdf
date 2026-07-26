# mdpdf

Автономная консольная утилита: Markdown → PDF.

```text
Markdown → собственное AST → Typst source → встроенный Typst compiler → PDF
```

Один исполняемый файл. Не требует установленного Typst, Chromium, LaTeX или Pandoc.
Не обращается к сети и не запускает внешние процессы. Шрифты и шаблон оформления
встроены в бинарник.

## Сборка

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
- кириллица и Unicode.

YAML front matter допускается: распознаётся и отбрасывается, метаданные из него
не извлекаются.

Не поддерживается: HTML, JavaScript, математика, сноски, библиография,
Mermaid/PlantUML, сетевые изображения, пользовательский Typst-код, пакеты и
шаблоны, PDF/A, PDF/UA, подпись и шифрование PDF.

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
| `docs/mdpdf-technical-spec-v2.md` | Техническое задание, редакция 2.0 |
| `docs/progress.md` | Журнал работ, решения, готовность к 1.0 |
| `AGENTS.md` | Инварианты для разработки |

## Статус

Конвейер и локальные проверки (`make ci` на macOS ARM64) готовы. Публичный
выпуск **1.0** и официальная поддержка Linux/Windows — после первого успешного
прогона CI на remote-репозитории.

## Лицензии

Код — MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).

Встроенные шрифты Noto Sans и Noto Sans Mono — SIL Open Font License 1.1:
`assets/fonts/OFL.txt`.
