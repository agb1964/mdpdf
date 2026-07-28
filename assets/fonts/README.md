# Встроенные шрифты

`mdpdf` не читает системные шрифты. Все шрифты подключаются в бинарник через
`include_bytes!` и регистрируются в фиксированном порядке.

## Состав

| Файл | Семейство и начертание | Версия из metadata | SHA-256 |
|---|---|---:|---|
| `NotoSans-Regular.ttf` | Noto Sans Regular | 2.015 | `478c558ea716033cd60c03438f628dfa75694dcf6b5f6d505a2f05fd2b4f3823` |
| `NotoSans-Bold.ttf` | Noto Sans Bold | 2.015 | `1df075a380fc7cb898acf64c1f7b3b4dd780de3caa860178bf929de35817a913` |
| `NotoSans-Italic.ttf` | Noto Sans Italic | 2.015 | `467e3f89eeca4108bb8710a2b9e0cf2281ac56d5b0609211a83776d0505eecb5` |
| `NotoSans-BoldItalic.ttf` | Noto Sans Bold Italic | 2.015 | `1b602a9d6353be42c91df097a4857b69fa2696f26703d7a33b54a15d87c2622c` |
| `NotoSansMono-Regular.ttf` | Noto Sans Mono Regular | 2.014 | `65b5e2b2c4a1fba9ae8be1f026cb35b03dcb8886d9b2a4147054fde12f7e767d` |
| `NotoColorEmoji.ttf` | Noto Color Emoji Regular | 2.051 | `72a635cb3d2f3524c51620cdde406b217204e8a6a06c6a096ff8ed4b5fd6e27b` |

Noto Sans и Noto Sans Mono происходят из проекта
[Noto Latin, Greek and Cyrillic](https://github.com/notofonts/latin-greek-cyrillic).
Noto Color Emoji происходит из
[googlefonts/noto-emoji](https://github.com/googlefonts/noto-emoji).

Все файлы распространяются по SIL Open Font License 1.1:

- `OFL.txt` — Noto Sans и Noto Sans Mono;
- `LICENSE-NotoColorEmoji.txt` — Noto Color Emoji.

## Назначение

- четыре начертания Noto Sans используются для обычного, жирного и курсивного
  текста;
- Noto Sans Mono используется в inline и fenced code;
- Noto Color Emoji предоставляет цветные bitmap-глифы CBDT/CBLC.

Если символ отсутствует во всех шести шрифтах, `mdpdf` выводит предупреждение
в stderr и продолжает создание PDF. Системный fallback намеренно запрещён,
поэтому результат остаётся автономным и детерминированным.

## Обновление

Обновление шрифтов выполняется отдельной задачей:

1. подтвердить пользовательский сценарий и источник файла;
2. скачать файл только при разработке — runtime `mdpdf` остаётся без сети;
3. проверить название, версию и лицензию;
4. заменить файл и пересчитать `shasum -a 256 assets/fonts/*.ttf`;
5. обновить эту таблицу и соответствующий license-файл;
6. проверить порядок `TEXT_FONTS` и `EMBEDDED_FONTS`;
7. выполнить тесты покрытия кириллицы, mono и emoji;
8. выполнить `make ci` и визуально сравнить golden PDF;
9. сравнить размер release-бинарника;
10. обновить `README.md`, ТЗ и `CHANGELOG.md`.

Release workflow должен публиковать обе лицензии шрифтов вместе с лицензиями
проекта.
