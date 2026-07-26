# mdpdf

Автономная консольная утилита для преобразования Markdown в PDF.

```text
Markdown → собственное AST → Typst source → встроенный Typst compiler → PDF
```

Один исполняемый файл. Не требует Typst, Chromium, LaTeX или Pandoc, не обращается
к сети и не запускает внешние процессы. Шрифты и шаблон оформления встроены в бинарник.

> **Статус:** Milestone 1. Работают CLI, коды завершения, чтение входа, парсер
> Markdown с построением AST и `--emit-ast`. Генератор Typst и компилятор PDF
> реализуются на Milestone 2–3, поэтому создание PDF пока возвращает ошибку
> «not implemented».

## Сборка

```bash
cargo build --release
```

Бинарник: `target/release/mdpdf`.

Частые команды собраны в `Makefile` — `make` печатает список, `make ci` прогоняет
всё, что гоняет CI.

## Использование

```bash
mdpdf input.md
```

Без `--output` результат пишется рядом с исходным файлом: `input.md` → `input.pdf`.

```bash
mdpdf input.md --output output.pdf
cat input.md | mdpdf - --output output.pdf
mdpdf input.md --check
mdpdf input.md --emit-typst output.typ
mdpdf input.md --emit-ast output.json
```

При чтении из stdin параметр `--output` обязателен.

### Параметры

```text
mdpdf [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Markdown-файл или "-" для stdin

Options:
  -o, --output <FILE>      Выходной PDF
      --title <TEXT>       Переопределить заголовок документа
      --author <TEXT>      Указать автора
      --paper <PAPER>      a4 или letter [default: a4]
      --margin <LENGTH>    Поля страницы [default: 20mm]
      --font-size <LENGTH> Основной размер текста [default: 11pt]
      --toc                Создать оглавление
      --heading-numbers    Нумеровать заголовки
      --check              Проверить документ без записи PDF
      --emit-ast <FILE>    Записать AST в JSON
      --emit-typst <FILE>  Записать сгенерированный Typst
      --overwrite          Разрешить замену выходного файла
      --quiet              Не выводить сообщения об успехе
      --verbose            Расширенная диагностика
  -h, --help               Справка
  -V, --version            Версия
```

## Поддерживаемый Markdown

Заголовки, абзацы, inline-форматирование (курсив, жирный, зачёркивание, код),
ссылки, изображения, цитаты, списки (включая вложенные и task lists), fenced
code blocks, таблицы, горизонтальные линии.

Не входит в первую версию: HTML внутри Markdown, математика, сноски, библиография,
Mermaid/PlantUML, пользовательские Typst-шаблоны и скрипты, PDF/A, PDF/UA,
подпись и шифрование PDF.

## Изображения

Пути разрешаются относительно каталога Markdown-файла (при чтении из stdin —
относительно текущего каталога). Поддерживаются PNG, JPEG, GIF и SVG без внешних
ресурсов; формат определяется по содержимому. Сетевые URL и `data:` URI отклоняются,
выход за пределы каталога документа запрещён.

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

## Лицензии

Код — MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).

Встроенные шрифты Noto Sans и Noto Sans Mono распространяются по SIL Open Font
License 1.1: `assets/fonts/OFL.txt`.
