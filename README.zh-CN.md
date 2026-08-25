# mdpdf

**[English](README.md) | [Русский](README.ru.md) | [中文](README.zh-CN.md)**

[![CI](https://github.com/agb1964/mdpdf/actions/workflows/ci.yml/badge.svg)](https://github.com/agb1964/mdpdf/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/agb1964/mdpdf)](https://github.com/agb1964/mdpdf/releases)
[![downloads](https://img.shields.io/github/downloads/agb1964/mdpdf/total)](https://github.com/agb1964/mdpdf/releases)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

一个独立的命令行工具：Markdown → PDF。

```text
Markdown → 自定义 AST → Typst 源码 → 内置 Typst 编译器 → PDF
```

单个可执行文件。无需安装 Typst、Chromium、LaTeX 或 Pandoc。
不访问网络，不启动外部进程。字体和排版模板已内嵌在二进制文件中。

**用途。** `mdpdf` 是用于**预览**和在本地从 Markdown 构建 PDF 的工具：
“打开文档，得到文件，没有一堆依赖。”
它**不是**参考级出版流水线，也**不能**替代 pandoc + LaTeX /
mermaid-cli + Chromium，如果你需要与 GitHub 逐字节一致或对排版有印刷级控制。
对于最终印刷输出和“与浏览器一致”的效果，请使用专业工具。

## 安装

预编译的二进制文件发布在
[GitHub Releases](https://github.com/agb1964/mdpdf/releases) 页面：

| 平台 | 压缩包 |
|---|---|
| macOS Apple Silicon | `mdpdf-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `mdpdf-x86_64-apple-darwin.tar.gz` |
| Linux x86-64 | `mdpdf-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `mdpdf-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86-64 | `mdpdf-x86_64-pc-windows-msvc.zip` |

解压压缩包，并将 `mdpdf`（Windows 上为 `mdpdf.exe`）放入 `PATH`
中的某个目录。无需安装 Typst 或其他运行时组件。

## 从源码构建

从源码构建需要稳定版 Rust：

```bash
cargo build --release
```

二进制文件：`target/release/mdpdf`。

常用命令在 `Makefile` 中：`make` 打印命令列表，`make ci` 运行
提交前必须通过的检查。

## 使用方法

```bash
mdpdf input.md
```

不带 `--output` 时，PDF 会写到源文件旁边：`input.md` → `input.pdf`。

```bash
mdpdf input.md --output output.pdf
mdpdf input.md -o output.pdf
cat input.md | mdpdf - --output output.pdf
mdpdf input.md --check
mdpdf input.md --emit-typst document.typ
mdpdf input.md --emit-ast ast.json
```

- 从 stdin（`-`）读取时，`--output` 参数为**必需**。
- `--check` 运行完整流水线（包括编译），但**不**写入 PDF。
- 如果只指定 `--emit-ast` 或 `--emit-typst` 而没有 `--output`，
  程序会写入所请求的中间结果，然后**不生成 PDF** 直接退出。
- 没有 `--overwrite` 时，已存在的输出文件不会被覆盖。
  `--emit-ast` 和 `--emit-typst` 文件同样如此。

### 参数

```text
mdpdf [OPTIONS] <INPUT>

Arguments:
  <INPUT>                      Markdown 文件，或用 "-" 表示 stdin

Options:
  -o, --output <FILE>          输出 PDF
      --title <TEXT>           重置文档标题
      --author <TEXT>          指定作者
      --paper <PAPER>          a4 或 letter [default: a4]
      --margin <LENGTH>        页边距 [default: 20mm]
      --font-size <LENGTH>     正文字号 [default: 11pt]
      --toc                    生成目录
      --heading-numbers        为标题编号
      --check                  校验文档，不写入 PDF
      --emit-ast <FILE>        将 AST 写为 JSON
      --emit-typst <FILE>      写出生成的 Typst 源码
      --overwrite              允许覆盖输出文件
      --quiet                  不显示成功消息
      --verbose                详细诊断信息
  -h, --help                   帮助
  -V, --version                版本
```

## 支持的 Markdown

- 1–6 级标题、段落；
- 粗体、斜体、删除线、行内代码；
- 围栏代码块（语言作为标题显示，无语法高亮）；
- 无序和有序列表、嵌套列表、任务列表；
- 引用块（包括嵌套引用）；
- 链接和本地图片；
- 表格、水平分隔线；
- 西里尔字母和 Unicode；
- 在语言为 `mermaid` 的代码块中的 Mermaid 图表（见下文）。

允许 YAML front matter：会被识别并丢弃，不从中提取任何元数据。

二进制文件中内嵌了 Noto Color Emoji。如果某个字符在所有内嵌字体中
都不存在，`mdpdf` 会向 stderr 输出警告并继续生成 PDF；
不会使用系统字体进行替代。

不支持：HTML、JavaScript、数学公式、脚注、参考文献、PlantUML、
网络图片、自定义 Typst 代码、扩展包和模板、PDF/A、PDF/UA、
PDF 签名和加密。

## Mermaid 图表

### 状态：预览级渲染，非参考实现

内置 Mermaid 的目的是让你**在不需要 Node.js 和浏览器的情况下在 PDF 中看到图表**。
输出**并非**定位为 Mermaid.js / mermaid-cli / GitHub 的参考渲染。
布局、箭头、`Note`/`alt` 和标签可能不同——有时差异明显。如果需要与
网页像素级一致，请单独构建图表（mermaid-cli 等），然后嵌入生成的
SVG/PNG。

流水线（规范 §10.5）：

```text
mermaid 代码块  →  mermaid-rs-renderer (Rust)  →  SVG  →  Typst image  →  PDF
```

不使用 JavaScript、Chromium、Node.js 或外部进程。引擎为
[`mermaid-rs-renderer`](https://crates.io/crates/mermaid-rs-renderer)
（库 API，`default-features = false`）。不支持的语法或渲染错误
不会中断构建：该代码块会按普通代码输出，并向 stderr 打印警告。

图表类型取决于 `Cargo.lock` 中锁定的 mmdr 版本所支持的类型
（flowchart、sequence、class、state、ER、gantt 等）。详细信息和 SVG
安全限制见 `docs/mdpdf-technical-spec-v2.md` §10.5。

在竖排版面中会缩小到无法阅读的宽幅图表，会自动移到单独的**横向**页面；
其后的文本继续以竖排方向排版。

### 如何编写图表以保持 PDF 可读

mmdr 引擎最适合**简单的拓扑结构**。在密集的图表上，常见弯曲的绕行、
重叠的标签和拥挤的 `alt`/`Note`。实用建议：

- 使用简短标签；长文本放在**节点内部**或图表之后的正文中，
  而不是放在每条边上；
- sequence 图：少用 `Note`，`alt` / `else` 条件保持简短；
- flowchart 图：如果直线箭头很重要，避免“外部 → 子图 id”的边
  以及指向集群内同一节点的多条交叉入口；
- 有疑问时，简化图表：对于预览而言通常已经足够。

### 已知限制（不是文档的 bug，而是引擎的上限）

- 几何布局**不**与 mermaid-cli / GitHub 一致；一致性**不是**目标；
- `click A "https://…"` 以及任何带外部链接的 SVG → 按代码块处理
  （规范 §33.3）；
- sequence 图：只有在**没有**声明任何 `participant` 时才自动添加参与者；
- 过长的边标签、密集的 notes/alt 和复杂的布线可能显得凌乱——
  简化源文件比“再加一个补丁”更可靠；
- 指向**子图 id** 的边（`A --> SubgraphId`）在 mmdr 中经常产生布局
  瑕疵；更可靠的做法是将箭头指向子图内的**具体节点**
  （`mdpdf` 有部分的后处理修正，但无法 100% 与 mermaid.js 对齐）。

## 图片

路径相对于 Markdown 文件所在目录解析（stdin 时相对于当前目录）。
支持 PNG、JPEG、GIF 和不含外部资源的 SVG；格式按内容识别。

会被拒绝的：

- 网络 URL 和 `data:` URI；
- 协议相对地址（`//host/...`）；
- 文档目录之外的路径（包括通过 `..` 和符号链接）；
- 包含 http/https/file/data 链接的 SVG。

Typst 源码中只出现 `/mdpdf-resources/000001.png` 这样的虚拟路径。

## 退出码

| 代码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 一般运行时错误 |
| 2 | CLI 参数错误 |
| 3 | 输入读取错误 |
| 4 | Markdown 错误 |
| 5 | AST 校验错误 |
| 6 | Typst 生成错误 |
| 7 | Typst 编译错误 |
| 8 | 输出写入错误 |
| 9 | 资源访问策略违规 |

## 文档

| 文件 | 内容 |
|---|---|
| `docs/README.md` | 当前及历史文档的索引 |
| `docs/mdpdf-technical-spec-v2.md` | 技术规范，2.0 版 |
| `docs/progress.md` | 工作日志、决策、1.0 准备情况 |
| `CONTRIBUTING.md` | 本地开发与检查 |
| `docs/releasing.md` | 发布检查清单 |
| `SECURITY.md` | 威胁模型与漏洞报告 |
| `CHANGELOG.md` | 各版本面向用户的变更 |
| `AGENTS.md` | 开发不变量 |

## 状态

首个技术版本已发布。
GitHub CI 已在 Ubuntu、macOS 和 Windows 上验证；发布工作流会为
五个目标平台构建二进制文件。**1.0** 版本尚未宣布。
当前版本 [`v0.3.3`](https://github.com/agb1964/mdpdf/releases/tag/v0.3.3)

### 维护

本项目源于作者的**个人**需求（单个二进制文件、离线、Markdown → PDF，
没有一堆运行时）。开发时间有限：修复按作者自身需求进行，而不是按照
“与 mermaid-cli / 印刷排版再接近一层”的路线图。

如果你需要**不同**程度的功能（参考级 Mermaid、HTML、数学公式、
自定义主题、插件等）——许可证允许这样做：**欢迎 fork 和 PR**，
没有人挡住这条路。不要指望每个关于图表质量或排版的愿望都会成为
上游的优先事项：fork 和独立的流水线正是为此而存在的。

## 许可证

代码 — MIT（`LICENSE-MIT`）。

内嵌的 Noto Sans 和 Noto Sans Mono 采用 SIL Open Font License 1.1
（`assets/fonts/OFL.txt`）；Noto Color Emoji 采用 SIL Open Font License 1.1
（`assets/fonts/LICENSE-NotoColorEmoji.txt`）。版本、校验和及更新
流程列在 `assets/fonts/README.md` 中。
��后的正文中,
  而不是放在每条边上;
- sequence 图:少用 `Note`,`alt` / `else` 条件保持简短;
- flowchart 图:如果直线箭头很重要,避免"外部 → 子图 id"的边
  以及指向集群内同一节点的多条交叉入口;
- 有疑问时,简化图表:对于预览而言通常已经足够。

### 已知限制(不是文档的 bug,而是引擎的上限)

- 几何布局**不**与 mermaid-cli / GitHub 一致;一致性**不是**目标;
- `click A "https://…"` 以及任何带外部链接的 SVG → 按代码块处理
  (规范 §33.3);
- sequence 图:只有在**没有**声明任何 `participant` 时才自动添加参与者;
- 过长的边标签、密集的 notes/alt 和复杂的布线可能显得凌乱——
  简化源文件比"再加一个补丁"更可靠;
- 指向**子图 id** 的边(`A --> SubgraphId`)在 mmdr 中经常产生布局
  瑕疵;更可靠的做法是将箭头指向子图内的**具体节点**
  (`mdpdf` 有部分的后处理修正,但无法 100% 与 mermaid.js 对齐)。

## 图片

路径相对于 Markdown 文件所在目录解析(stdin 时相对于当前目录)。
支持 PNG、JPEG、GIF 和不含外部资源的 SVG;格式按内容识别。

会被拒绝的:

- 网络 URL 和 `data:` URI;
- 协议相对地址(`//host/...`);
- 文档目录之外的路径(包括通过 `..` 和符号链接);
- 包含 http/https/file/data 链接的 SVG。

Typst 源码中只出现 `/mdpdf-resources/000001.png` 这样的虚拟路径。

## 退出码

| 代码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 一般运行时错误 |
| 2 | CLI 参数错误 |
| 3 | 输入读取错误 |
| 4 | Markdown 错误 |
| 5 | AST 校验错误 |
| 6 | Typst 生成错误 |
| 7 | Typst 编译错误 |
| 8 | 输出写入错误 |
| 9 | 资源访问策略违规 |

## 文档

| 文件 | 内容 |
|---|---|
| `docs/README.md` | 当前及历史文档的索引 |
| `docs/mdpdf-technical-spec-v2.md` | 技术规范,2.0 版 |
| `docs/progress.md` | 工作日志、决策、1.0 准备情况 |
| `CONTRIBUTING.md` | 本地开发与检查 |
| `docs/releasing.md` | 发布检查清单 |
| `SECURITY.md` | 威胁模型与漏洞报告 |
| `CHANGELOG.md` | 各版本面向用户的变更 |
| `AGENTS.md` | 开发不变量 |

## 状态

首个技术版本已发布。
GitHub CI 已在 Ubuntu、macOS 和 Windows 上验证;发布工作流会为
五个目标平台构建二进制文件。**1.0** 版本尚未宣布。
当前版本 [`v0.3.3`](https://github.com/agb1964/mdpdf/releases/tag/v0.3.3)

### 维护

本项目源于作者的**个人**需求(单个二进制文件、离线、Markdown → PDF,
没有一堆运行时)。开发时间有限:修复按作者自身需求进行,而不是按照
"与 mermaid-cli / 印刷排版再接近一层"的路线图。

如果你需要**不同**程度的功能(参考级 Mermaid、HTML、数学公式、
自定义主题、插件等)——许可证允许这样做:**欢迎 fork 和 PR**,
没有人挡住这条路。不要指望每个关于图表质量或排版的愿望都会成为
上游的优先事项:fork 和独立的流水线正是为此而存在的。

## 许可证

代码 — MIT(`LICENSE-MIT`)。

内嵌的 Noto Sans 和 Noto Sans Mono 采用 SIL Open Font License 1.1
(`assets/fonts/OFL.txt`);Noto Color Emoji 采用 SIL Open Font License 1.1
(`assets/fonts/LICENSE-NotoColorEmoji.txt`)。版本、校验和及更新
流程列在 `assets/fonts/README.md` 中。
