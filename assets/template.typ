// Встроенный шаблон оформления mdpdf (ТЗ §21).
//
// Шаблон отвечает только за оформление. Ему запрещено:
//   * импортировать пакеты;
//   * обращаться к сети или произвольным файлам;
//   * зависеть от текущего времени, окружения или случайных значений.
//
// На Milestone 0 это заглушка: объявлены сигнатуры функций, которыми будет
// пользоваться генератор (ТЗ §24). Оформление наполняется на Milestone 2.

#let main-font = "Noto Sans"
#let mono-font = "Noto Sans Mono"

// Базовая настройка документа.
#let mdpdf-document(
  paper: "a4",
  margin: 20mm,
  font-size: 11pt,
  title: none,
  author: none,
  toc: false,
  heading-numbers: false,
  body,
) = {
  set document(title: if title == none { "" } else { title })
  set page(paper: paper, margin: margin, numbering: "1")
  set text(font: main-font, size: font-size, lang: "ru")
  set par(justify: false, leading: 0.65em)

  if heading-numbers {
    set heading(numbering: "1.1")
  }

  if title != none {
    align(center, text(size: font-size * 1.8, weight: "bold", title))
  }
  if author != none {
    align(center, text(size: font-size, author))
  }
  if toc {
    outline()
  }

  body
}

// Кодовый блок. Код передаётся строкой, а не raw-разметкой (ТЗ §23.3).
#let mdpdf-code(language: none, body: "") = block(
  width: 100%,
  fill: luma(245),
  inset: 8pt,
  radius: 3pt,
  breakable: true,
  {
    if language != none and language != "" {
      text(size: 0.75em, fill: luma(100), language)
      linebreak()
    }
    text(font: mono-font, size: 0.9em, raw(body))
  },
)

// Inline-код.
#let mdpdf-inline-code(body: "") = box(
  fill: luma(240),
  inset: (x: 3pt, y: 0pt),
  outset: (y: 3pt),
  radius: 2pt,
  text(font: mono-font, size: 0.9em, raw(body)),
)

// Цитата.
#let mdpdf-quote(body) = block(
  width: 100%,
  inset: (left: 12pt),
  stroke: (left: 2pt + luma(200)),
  body,
)

// Список. Вид маркера и состояние task-list определяются здесь, а не в Rust (ТЗ §24.9).
#let mdpdf-list(ordered: false, start: 1, items: ()) = {
  if ordered {
    enum(start: start, ..items)
  } else {
    list(..items)
  }
}

#let mdpdf-task(checked: none, body) = {
  let marker = if checked == none { "" } else if checked { "☑ " } else { "☐ " }
  [#marker#body]
}

// Таблица.
#let mdpdf-table(columns: 1, alignments: (), header: (), rows: ()) = table(
  columns: columns,
  align: (col, _) => if col < alignments.len() { alignments.at(col) } else { left },
  table.header(..header),
  ..rows.flatten(),
)

// Изображение по виртуальному пути (ТЗ §24.6).
#let mdpdf-image(path: "", alt: none) = figure(
  image(path, width: 100%),
  caption: if alt == none { none } else { alt },
)

// Горизонтальная линия.
#let mdpdf-rule() = line(length: 100%, stroke: 0.5pt + luma(180))
