# mdpdf

**[English](README.md) | [Русский](README.ru.md) | [中文](README.zh-CN.md)**

[![CI](https://github.com/agb1964/mdpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/agb1964/mdpdf/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/agb1964/mdpdf)](https://github.com/agb1964/mdpdf/releases)
[![downloads](https://img.shields.io/github/downloads/agb1964/mdpdf/total)](https://github.com/agb1964/mdpdf/releases)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

A self-contained CLI utility: Markdown → PDF.

```text
Markdown → custom AST → Typst source → embedded Typst compiler → PDF
```

A single executable. No Typst, Chromium, LaTeX, or Pandoc installation required.
No network access, no external processes. Fonts and the layout template are
embedded in the binary.

**Purpose.** `mdpdf` is a tool for **previewing** and locally building PDFs
from Markdown: "open a document, get a file, without a zoo of dependencies."
It is **not** a reference publishing pipeline and **not** a replacement for
pandoc + LaTeX / mermaid-cli + Chromium when you need byte-level parity with
GitHub or typographic control over the layout. For final print output and
"as in the browser" results, use specialized tools.

## Installation

Prebuilt binaries are published on the
[GitHub Releases](https://github.com/agb1964/mdpdf/releases) page:

| Platform | Archive |
|---|---|
| macOS Apple Silicon | `mdpdf-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `mdpdf-x86_64-apple-darwin.tar.gz` |
| Linux x86-64 | `mdpdf-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `mdpdf-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86-64 | `mdpdf-x86_64-pc-windows-msvc.zip` |

Extract the archive and place `mdpdf` (`mdpdf.exe` on Windows) in a directory
on your `PATH`. No Typst or additional runtime components need to be installed.

## Building from source

To build from source you need a stable Rust toolchain:

```bash
cargo build --release
```

Binary: `target/release/mdpdf`.

Common commands are in the `Makefile`: `make` prints the list, `make ci` runs
the checks that must pass before committing.

## Usage

```bash
mdpdf input.md
```

Without `--output`, the PDF is written next to the source: `input.md` → `input.pdf`.

```bash
mdpdf input.md --output output.pdf
mdpdf input.md -o output.pdf
cat input.md | mdpdf - --output output.pdf
mdpdf input.md --check
mdpdf input.md --emit-typst document.typ
mdpdf input.md --emit-ast ast.json
```

- When reading from stdin (`-`), the `--output` option is **required**.
- `--check` runs the full pipeline (including compilation) and does **not** write a PDF.
- If only `--emit-ast` or `--emit-typst` is given without `--output`, the program
  writes the requested intermediate result and exits **without** producing a PDF.
- An existing output file is not overwritten without `--overwrite`. The same
  applies to `--emit-ast` and `--emit-typst` files.

### Options

```text
mdpdf [OPTIONS] <INPUT>

Arguments:
  <INPUT>                      Markdown file or "-" for stdin

Options:
  -o, --output <FILE>          Output PDF
      --title <TEXT>           Document title
      --author <TEXT>          Author
      --paper <PAPER>          a4 or letter [default: a4]
      --margin <LENGTH>        Page margins [default: 20mm]
      --font-size <LENGTH>     Body text size [default: 11pt]
      --toc                    Generate a table of contents
      --heading-numbers        Number the headings
      --check                  Validate the document without writing a PDF
      --emit-ast <FILE>        Write the AST as JSON
      --emit-typst <FILE>      Write the generated Typst
      --overwrite              Allow replacing the output file
      --quiet                  Suppress the success message
      --verbose                Extended diagnostics
  -h, --help                   Help
  -V, --version                Version
```

## Supported Markdown

- headings 1–6, paragraphs;
- bold, italic, strikethrough, inline code;
- fenced code blocks (language shown as a caption, no syntax highlighting);
- bulleted and numbered lists, nested lists, task lists;
- block quotes (including nested);
- links and local images;
- tables, horizontal rules;
- Cyrillic and Unicode;
- Mermaid diagrams in code blocks with the `mermaid` language (see below).

YAML front matter is allowed: it is recognized and discarded; no metadata is
extracted from it.

Noto Color Emoji is embedded in the binary. If a character is missing from all
embedded fonts, `mdpdf` prints a warning to stderr and continues building the
PDF; no system font is used as a fallback.

Not supported: HTML, JavaScript, math, footnotes, bibliographies, PlantUML,
network images, custom Typst code, packages and templates, PDF/A, PDF/UA,
PDF signing and encryption.

## Mermaid diagrams

### Status: preview-quality rendering, not a reference

The built-in Mermaid support exists so you can **see the diagram in a PDF
without Node.js or a browser**. The output is **not** positioned as a reference
render of Mermaid.js / mermaid-cli / GitHub. Layout, arrows, `Note`/`alt` and
labels may differ — sometimes noticeably. If you need pixel parity with the
web, build your diagrams separately (mermaid-cli, etc.) and embed the
resulting SVG/PNG.

Pipeline (spec §10.5):

```text
mermaid block  →  mermaid-rs-renderer (Rust)  →  SVG  →  Typst image  →  PDF
```

No JavaScript, Chromium, Node.js, or external processes. The engine is
[`mermaid-rs-renderer`](https://crates.io/crates/mermaid-rs-renderer) (library
API, `default-features = false`). Unsupported syntax or a render error does not
abort the build: the block is rendered as plain code, and a warning is printed
to stderr.

Diagram types are those accepted by the mmdr version pinned in `Cargo.lock`
(flowchart, sequence, class, state, ER, gantt, etc.). Details and SVG security
constraints are in `docs/mdpdf-technical-spec-v2.md` §10.5.

A wide diagram that would shrink to an unreadable size in the portrait text
block is automatically moved to a separate **landscape** page; the text after
it continues in portrait orientation.

### How to write diagrams so the PDF stays readable

The mmdr engine handles **simple topology** best. On dense diagrams, curved
detours, overlapping labels, and cramped `alt`/`Note` blocks are typical.
Practical tips:

- short labels; put long text **inside the node** or in prose after the
  diagram, not on every edge;
- sequence: **few** `Note`s, short `alt` / `else` conditions;
- flowchart: avoid "outside → subgraph id" edges and multiple crossing
  entries into one node inside a cluster if a straight arrow matters;
- when in doubt, simplify the diagram: for a preview this is usually enough.

### Known limitations (not "doc bugs", but the engine's ceiling)

- geometry does **not** match mermaid-cli / GitHub; parity is **not** a goal;
- `click A "https://…"` and any SVG with an external link → code block
  (spec §33.3);
- sequence: auto-participants only if **no** `participant` is declared;
- long edge labels, dense notes/alt, and complex routing may look untidy —
  simplifying the source is more reliable than "one more workaround";
- an edge to a **subgraph id** (`A --> SubgraphId`) often produces a layout
  artifact in mmdr; it is more reliable to point the arrow at a **specific
  node** inside the subgraph (`mdpdf` has a partial post-fix that does not
  fully align with mermaid.js).

## Images

Paths are resolved relative to the Markdown file's directory (for stdin,
relative to the current directory). PNG, JPEG, GIF, and SVG without external
resources are supported; the format is detected from the content.

Rejected:

- network URLs and `data:` URIs;
- protocol-relative addresses (`//host/...`);
- paths outside the document directory (including via `..` and symlinks);
- SVGs with links to http/https/file/data.

Only virtual paths like `/mdpdf-resources/000001.png` appear in the Typst source.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | general runtime error |
| 2 | CLI argument error |
| 3 | input read error |
| 4 | Markdown error |
| 5 | AST validation error |
| 6 | Typst generation error |
| 7 | Typst compilation error |
| 8 | output write error |
| 9 | resource access policy violation |

## Documentation

| File | Contents |
|---|---|
| `docs/README.md` | Map of current and historical documentation |
| `docs/mdpdf-technical-spec-v2.md` | Technical specification, revision 2.0 |
| `docs/progress.md` | Work log, decisions, readiness for 1.0 |
| `CONTRIBUTING.md` | Local development and checks |
| `docs/releasing.md` | Release checklist |
| `SECURITY.md` | Threat model and vulnerability reporting |
| `CHANGELOG.md` | User-facing changes by version |
| `AGENTS.md` | Development invariants |

## Status

The first technical release has been published.
GitHub CI is verified on Ubuntu, macOS, and Windows; the release workflow
builds binaries for five targets. Version **1.0** has not been announced yet.
Current version: [`v0.3.3`](https://github.com/agb1964/mdpdf/releases/tag/v0.3.3)

### Maintenance

The project grew out of the author's **personal** needs (one binary, offline,
Markdown → PDF without a zoo of runtimes). Development time is limited: fixes
land as the author needs them, not according to a roadmap of "one more level of
parity with mermaid-cli / typography."

If you need a **different** degree of functionality (reference Mermaid, HTML,
math, custom themes, plugins, etc.) — the license allows it: **forks and PRs
are welcome**, nobody is blocking the path. Do not expect every wish about
diagram quality or layout to become an upstream priority: that is exactly
what forks and separate pipelines exist for.

## Licenses

Code — MIT (`LICENSE-MIT`).

The embedded Noto Sans and Noto Sans Mono are under the SIL Open Font License 1.1
(`assets/fonts/OFL.txt`); Noto Color Emoji is under the SIL Open Font License 1.1
(`assets/fonts/LICENSE-NotoColorEmoji.txt`). Versions, checksums, and the update
procedure are listed in `assets/fonts/README.md`.
