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

// Диаграмма Mermaid (ТЗ §10.5). Координаты вычисляет детерминированная
// Rust-раскладка, шаблон только рисует примитивы. Подписи приходят
// строковыми значениями и не интерпретируются как Typst-код.
// fit — масштаб вписывания в страницу: геометрия уже умножена на него
// в Rust, здесь он сжимает текст, отступы, штрихи и стрелки.
#let mdpdf-diagram(width: 0pt, height: 0pt, fit: 100%, boxes: (), lines: (), labels: ()) = {
  let edge-color = luma(110)
  let node-fill = luma(250)
  let stroke-width = 0.7pt * fit
  let node-stroke = stroke-width + luma(130)

  block(
    width: 100%, // иначе блок сжимается до ширины box и align(center) не работает
    breakable: false,
    above: 0.8em,
    below: 0.8em,
    align(center, box(width: width, height: height, {
      // Рёбра и линии жизни — под боксами.
      for (x1, y1, x2, y2, style) in lines {
        let stroke-spec = if style.starts-with("dashed") {
          (paint: edge-color, thickness: stroke-width, dash: "dashed")
        } else {
          (paint: edge-color, thickness: stroke-width)
        }
        place(dx: x1, dy: y1, line(start: (0pt, 0pt), end: (x2 - x1, y2 - y1), stroke: stroke-spec))
        if style != "plain" and style != "dashed" {
          // Стрелка: треугольник на конце линии, вычисленный по направлению
          // без поворотов (поворот вокруг точки требовал бы origin-трюков).
          let dx = (x2 - x1) / 1pt
          let dy = (y2 - y1) / 1pt
          let len = calc.sqrt(dx * dx + dy * dy)
          if len > 0 {
            let ux = dx / len
            let uy = dy / len
            let nx = -uy
            let ny = ux
            let back = 7.0 * fit
            let half = 3.5 * fit
            let base1 = (x2 - (ux * back - nx * half) * 1pt, y2 - (uy * back - ny * half) * 1pt)
            let base2 = (x2 - (ux * back + nx * half) * 1pt, y2 - (uy * back + ny * half) * 1pt)
            place(polygon(
              fill: if style.ends-with("filled-arrow") { edge-color } else { white },
              stroke: stroke-width + edge-color,
              (x2, y2),
              base1,
              base2,
            ))
          }
        }
      }
      // Боксы.
      for (x, y, w, h, label, shape) in boxes {
        let body = align(center + horizon, text(size: 0.85em * fit, label))
        place(dx: x, dy: y, if shape == "circle" {
          ellipse(width: w, height: h, fill: node-fill, stroke: node-stroke, inset: 2pt * fit, body)
        } else if shape == "rounded" {
          rect(width: w, height: h, fill: node-fill, stroke: node-stroke, radius: 4pt * fit, inset: 2pt * fit, body)
        } else if shape == "diamond" {
          polygon(
            fill: node-fill,
            stroke: node-stroke,
            (w / 2, 0pt),
            (w, h / 2),
            (w / 2, h),
            (0pt, h / 2),
          )
        } else {
          rect(width: w, height: h, fill: node-fill, stroke: node-stroke, inset: 2pt * fit, body)
        })
        if shape == "diamond" {
          place(dx: x, dy: y, box(width: w, height: h, body))
        }
      }
      // Подписи рёбер — поверх линий, с белой подложкой. Прямоугольники
      // посчитаны раскладкой: шаблон не измеряет текст, а рисует box
      // заданной ширины в заданной точке (перенос по ширине делает Typst,
      // разрывы длинных слов вставлены в текст нулевыми пробелами).
      for (x, y, w, label) in labels {
        place(
          dx: x,
          dy: y,
          box(
            width: w,
            fill: white,
            outset: 1.5pt * fit,
            align(center, text(size: 0.7em * fit, fill: luma(60), label)),
          ),
        )
      }
    })),
  )
}

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
  heading(level: 1, text("Диаграммы Mermaid"))

  heading(level: 2, text("Flowchart"))

  mdpdf-diagram(width: 161.12pt, height: 230.17pt, fit: 100%, boxes: ((52.76pt, 0pt, 55.6pt, 24.3pt, "Начало", "rect"), (30.8pt, 54.3pt, 99.52pt, 36.45pt, "Всё ок?", "diamond"), (0pt, 120.75pt, 82pt, 24.3pt, "Продолжаем", "rect"), (106pt, 120.75pt, 55.12pt, 55.12pt, "Стоп", "circle"), (56.06pt, 205.87pt, 49pt, 24.3pt, "Конец", "rounded")), lines: ((80.56pt, 24.3pt, 80.56pt, 54.3pt, "arrow"), (68.62pt, 90.75pt, 48.96pt, 120.75pt, "arrow"), (93.31pt, 90.75pt, 114.29pt, 120.75pt, "arrow"), (46.65pt, 145.05pt, 74.91pt, 205.87pt, "arrow"), (112.61pt, 175.87pt, 89.8pt, 205.87pt, "arrow")), labels: ((54.17pt, 100.75pt, 9.24pt, "да"), (96.87pt, 100.75pt, 13.86pt, "нет")))

  mdpdf-diagram(width: 200.4pt, height: 24.3pt, fit: 100%, boxes: ((0pt, 0pt, 49pt, 24.3pt, "alpha", "rect"), (79pt, 0pt, 42.4pt, 24.3pt, "beta", "rect"), (151.4pt, 0pt, 49pt, 24.3pt, "gamma", "rect")), lines: ((49pt, 12.15pt, 79pt, 12.15pt, "arrow"), (121.4pt, 12.15pt, 151.4pt, 12.15pt, "arrow")), labels: ())

  mdpdf-diagram(width: 183pt, height: 99.64pt, fit: 100%, boxes: ((80.2pt, 0pt, 22.6pt, 24.3pt, "A", "rect"), (80.2pt, 75.34pt, 22.6pt, 24.3pt, "B", "rect")), lines: ((91.5pt, 24.3pt, 91.5pt, 75.34pt, "arrow"),), labels: ((1.5pt, 29.8pt, 180pt, "очень длинная подпись ребра, которая не должна уезжать за пределы страницы, а переносится по ширине и резервирует себе место между узлами диаграммы"),))

  heading(level: 2, text("Sequence"))

  mdpdf-diagram(width: 151.2pt, height: 106.9pt, fit: 100%, boxes: ((0pt, 0pt, 55.6pt, 24.3pt, "Клиент", "rect"), (95.6pt, 0pt, 55.6pt, 24.3pt, "Сервер", "rect")), lines: ((27.8pt, 50.45pt, 123.4pt, 50.45pt, "filled-arrow"), (123.4pt, 82.75pt, 27.8pt, 82.75pt, "dashed-arrow"), (27.8pt, 24.3pt, 27.8pt, 106.9pt, "dashed"), (123.4pt, 24.3pt, 123.4pt, 106.9pt, "dashed")), labels: ((61.74pt, 45.44pt, 27.72pt, "запрос"), (64.05pt, 77.75pt, 23.1pt, "ответ")))

  heading(level: 2, text("Деградация до кода"))

  mdpdf-code(language: "mermaid", body: "gantt\ntitle План работ")
}
