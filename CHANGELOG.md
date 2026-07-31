# Changelog

Здесь фиксируются пользовательские изменения `mdpdf`. Формат основан на
[Keep a Changelog](https://keepachangelog.com/ru/1.1.0/), версии следуют
[Semantic Versioning](https://semver.org/lang/ru/).

## Unreleased

### Added

- Поддержка диаграмм Mermaid в code blocks с языком `mermaid`: подмножество
  flowchart (TD/TB/LR) и sequence рендерится собственным парсером и
  детерминированной раскладкой примитивами Typst, без JavaScript и новых
  зависимостей. Диаграммы вне подмножества или с ошибками выводятся как
  обычный блок кода с предупреждением в stderr.
- Расширение Mermaid: `subgraph` (вложенные, рёбра к id подграфа), формы
  `[(…)]` / `[/…/]` / `["…"]`, `<br/>` в подписях; sequence `Note over`,
  `alt` / `else` / `end`.
- Fuzz target `fuzz_mermaid_parser`.
- Встроен Noto Color Emoji, чтобы распространённые emoji не исчезали из PDF.
- Добавлено предупреждение о символах, для которых нет глифа ни в одном
  встроенном шрифте; предупреждение не останавливает создание PDF.
- Добавлена команда `make release-tag`: она читает версию из `Cargo.toml`,
  создаёт аннотированный тег и отправляет его в `origin`.
- Добавлена документация для контрибьюторов, релизов, безопасности и
  происхождения шрифтов.

### Changed

- Coverage снова является информационным показателем в соответствии с ТЗ 2.0
  и больше не блокирует CI по единственному общему проценту.
- Актуализирована документация после первого успешного GitHub CI и релиза.
- ТЗ §10.5 и `deny.toml`: целевой рендер Mermaid — `mermaid-rs-renderer` →
  SVG → Typst `#image` (без JS/Chromium); запрещены features `cli`/`png`/
  `benchmark` mmdr, крейты headless-браузера и JS-движков; лицензия
  `Unlicense` допущена для транзитивов regex-стека.
- Документ `docs/mermaid-mmdr-plan.md`: решение и пошаговый план внедрения
  mmdr (фазы spike → dep → in-memory SVG → интеграция → golden → удаление
  legacy).

### Fixed

- Раскладка flowchart: DFS для обратных рёбер стартует с истоков (in-degree 0),
  а рёбра к/от id `subgraph` разворачиваются в рёбра между листьями. Иначе цикл
  вроде `App → Telegram → Server → Caddy → App` раздувал номер слоя до
  O(nodes·edges) (диаграмма ~9000 pt в ширину) либо ставил приложение в слой 0.

## [0.1.0] — 2026-07-26

Первый публичный технический выпуск.

### Added

- Автономный конвейер Markdown → собственный AST → Typst → PDF.
- Встроенный Typst compiler, шаблон и пять начертаний Noto Sans/Noto Sans Mono.
- CLI со stdin, `--check`, `--emit-ast`, `--emit-typst`, настройками страницы
  и защитой от перезаписи.
- Локальные PNG, JPEG, GIF и SVG с ограничением путей и запретом сетевых
  ресурсов.
- Атомарная запись PDF, диагностические коды 0–9 и защитные лимиты.
- Unit, integration, fuzz и golden-тесты.
- CI для Ubuntu, macOS и Windows и релизные архивы для пяти target.

[Unreleased]: https://github.com/agb1964/mdpdf/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/agb1964/mdpdf/releases/tag/v0.1.0
