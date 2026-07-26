// Встроенный шаблон оформления mdpdf (ТЗ §21).
//
// Шаблон отвечает только за оформление. Ему запрещено:
//   * импортировать пакеты;
//   * обращаться к сети или произвольным файлам;
//   * выполнять нестабильные вычисления;
//   * зависеть от текущего времени, окружения или случайных значений.
//
// Все функции ниже вызываются генератором (ТЗ §24). Пользовательский текст
// приходит строковыми значениями и никогда не интерпретируется как Typst-код.

#let main-font = "Noto Sans"
#let mono-font = "Noto Sans Mono"

// Титульная часть и оглавление перед основным содержимым.
// Определена до mdpdf-document: Typst связывает имена в порядке чтения файла.
#let body-with-front-matter(title, author, toc, body) = {
  if title != none {
    align(center, text(size: 1.8em, weight: "bold", title))
    v(0.4em)
  }
  if author != none {
    align(center, text(size: 1em, style: "italic", author))
    v(0.8em)
  }
  if toc {
    outline(indent: auto)
    v(0.8em)
  }
  body
}

// Настройка документа. Применяется через `#show: mdpdf-document.with(...)`.
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
  set document(
    title: if title == none { "" } else { title },
    author: if author == none { () } else { (author,) },
  )
  set page(paper: paper, margin: margin, numbering: "1")
  set text(font: main-font, size: font-size, lang: "ru")
  set par(justify: false, leading: 0.65em)

  show heading: set block(above: 1.2em, below: 0.6em)
  show heading.where(level: 1): set text(size: 1.6em)
  show heading.where(level: 2): set text(size: 1.35em)
  show heading.where(level: 3): set text(size: 1.15em)

  if heading-numbers {
    set heading(numbering: "1.1")
    body-with-front-matter(title, author, toc, body)
  } else {
    body-with-front-matter(title, author, toc, body)
  }
}

// Блок кода. Код приходит строкой, а не raw-разметкой (ТЗ §23.3).
// Подсветка синтаксиса в первой версии не выполняется (ТЗ §24.7).
#let mdpdf-code(language: none, body: "") = block(
  width: 100%,
  fill: luma(246),
  stroke: 0.5pt + luma(220),
  inset: 8pt,
  radius: 3pt,
  breakable: true,
  {
    if language != none and language != "" {
      align(right, text(size: 0.7em, fill: luma(110), language))
    }
    set par(justify: false, leading: 0.55em)
    // Именно `show raw:`, а не `text(font: ..)`: элемент `raw` ставит свой
    // шрифт собственным show-правилом, и обёртка в `text` его не перебивает —
    // код молча выводился основным пропорциональным шрифтом.
    show raw: set text(font: mono-font, size: 0.85em)
    raw(body, block: true)
  },
)

// Inline-код.
#let mdpdf-inline-code(body) = box(
  fill: luma(240),
  inset: (x: 3pt, y: 0pt),
  outset: (y: 3pt),
  radius: 2pt,
  {
    // См. комментарий в mdpdf-code: `raw` перебивает внешний `text(font: ..)`.
    show raw: set text(font: mono-font, size: 0.9em)
    raw(body)
  },
)

// Цитата. Может содержать вложенные блоки, включая другие цитаты.
#let mdpdf-quote(body) = block(
  width: 100%,
  inset: (left: 12pt, top: 4pt, bottom: 4pt),
  stroke: (left: 2pt + luma(200)),
  body,
)

// Элемент списка. Отображение состояния task-list определяется здесь,
// а не в Rust-коде (ТЗ §24.9).
#let mdpdf-task(checked: none, body) = {
  if checked == none {
    body
  } else if checked {
    [☑ #body]
  } else {
    [☐ #body]
  }
}

// Список. `items` — кортеж уже готового содержимого.
#let mdpdf-list(ordered: false, start: 1, items: ()) = {
  if ordered {
    enum(start: start, ..items)
  } else {
    list(..items)
  }
}

// Таблица. `header` — кортеж ячеек, `rows` — кортеж кортежей.
#let mdpdf-table(columns: 1, alignments: (), header: (), rows: ()) = {
  if columns == 0 {
    return
  }
  table(
    columns: columns,
    align: (col, _) => {
      if col < alignments.len() { alignments.at(col) } else { auto }
    },
    stroke: 0.5pt + luma(200),
    inset: 6pt,
    table.header(..header),
    ..rows.flatten(),
  )
}

// Изображение по виртуальному пути (ТЗ §24.6).
// Остаётся inline-элементом: в Markdown изображение живёт внутри абзаца,
// а блочная figure вызывала бы предупреждение Typst внутри par().
// alt — это описание для PDF, а не подпись под картинкой.
#let mdpdf-image(path: "", alt: none) = box(image(path, alt: alt))

// Горизонтальная линия.
#let mdpdf-rule() = block(
  width: 100%,
  above: 1em,
  below: 1em,
  line(length: 100%, stroke: 0.5pt + luma(180)),
)

#show: mdpdf-document.with(
  paper: "a4",
  margin: 20mm,
  font-size: 11pt,
  title: none,
  author: none,
  toc: false,
  heading-numbers: false,
)

#{
  heading(level: 1, text("Ёжик, ёлка и «кавычки» — тире"))

  par(text("Строка с мягким переносом") + text(" ") + text("и продолжением, а также с жёстким переносом") + linebreak() + text("после двух пробелов."))

  mdpdf-code(language: none, body: "отступной блок кода")
}
