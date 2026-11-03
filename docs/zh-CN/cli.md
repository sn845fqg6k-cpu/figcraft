# 命令行

无窗口的 FigCraft：查看文件、导出、转换、渲染、跑命令脚本、列出命令、开 MCP。编译：

```sh
cargo build --release -p figcraft-cli
```

二进制是 `target/release/figcraft-cli`（Windows 上是 `figcraft-cli.exe`）。

Windows 安装包和便携 zip 里，旁边还有 `fig.cmd`。安装目录在 PATH 上时，下面两条是同一个程序：

```bat
fig --version
figcraft-cli --version
```

`fig` 后面的参数和 `figcraft-cli` 完全一样。下面用 `fig` 举例。旗标以 `fig --help` 和 `fig <子命令> --help` 为准。完整英文说明在 [cli.md](../cli.md)。

`--help` 或 `-h` 把帮助打到标准输出，退出码 0。`--version` 或 `-V` 打版本。参数不对是用法错误，退出码 2，信息在标准错误。做事失败退出码 1。

以 `-` 开头的值要写成 `--选项=值`，例如 `--filter=--json`。文件名自己以 `-` 开头时，选项放在 `--` 前面：

```sh
fig info --json -- --drawing.figcraft
```

## 字体

界面和排版用系统已安装的字体。可选地把 [craft-fonts](https://github.com/storytold/craft-fonts) 克隆到仓库旁边的 `../craft-fonts`，编译时嵌进去。没有它就用系统字体。

`fig --no-system-fonts` 或环境变量 `FIGCRAFT_NO_SYSTEM_FONTS=1`：只用编进程序的字体，不扫系统字体。

## 子命令

### `info <文件> [--json]`

页、节点数、顶层画框（id、类型、名称、尺寸）、样式 / 变量 / 图片数量。损坏修复和导入提示在标准错误，以 `note:` 开头（`--json` 时在 `notes`）。文件点名但本机没有的字体列为 `missing font`（`--json` 时在 `missingFonts`），文字用后备字体画。

也能读公开 REST API 的文件 JSON、尽力打开的 `.fig`，以及 Sketch 48 及以后存的 `.sketch`。没导入的内容同样以 `note:` 列出。

```text
fig info app.figcraft
```

### `export <文件> -o <输出> [选项]`

常用选项：`--format png|jpg|svg|webp|pdf|xml|fig|penpot`、`--scale N`（位图，默认 1，最大 64）、`--page 名称`、`--node id或名称`。

- `<输出>` 是目录：该页每个顶层画框一个文件，名字用画框名。没有画框就每个顶层图层一个文件。文件系统不允许的字符变成 `_`。
- `<输出>` 带图片扩展名：导出 `--node` 指定的图层，否则把整页合成一个文件。
- `--format fig` 或输出以 `.fig` 结尾：整份文件写成 `.fig`，实验性。省略了什么会打 `note:`。
- `--format xml`：Android Vector Drawable。`--vd-compat` 为 `bestEffort`（默认，近似）、`exact`（画不了就省略）或 `ignore`（不提示）。
- `--format penpot`：整份文件写成 Penpot 2.x 包。
- 不写 `--format` 时看输出扩展名，再没有就是 png。
- 不带 `--node` 的 `.pdf`：该页每个顶层画框一页 PDF，尺寸是画框的点大小。形状、文字和渐变保持矢量。

PDF 印刷：`--bleed 3mm`（或 `0.125in`，最大 2 英寸）、`--crop-marks`、`--bleed-marks`、`--cmyk`。CMYK 用简单公式从 sRGB 换算，没有输出意图配置，打样请对你的打印机配置。

```sh
fig export app.figcraft -o exports/ --scale 2
fig export app.figcraft -o hero.svg --node "Hero"
fig export app.figcraft -o screens.pdf --page "Screens"
fig export app.figcraft -o poster.pdf --node "Poster" --bleed 3mm --crop-marks --cmyk
fig export app.figcraft -o handoff.fig
```

### `convert <输入> <输出>`

按扩展名。输入：`.figcraft`、`.json`、`.fig`（尽力）、`.sketch`（Sketch 48 起，尽力）、`.svg`、常见位图。输出：`.figcraft`、`.json`、`.svg`、位图、`.pdf`、`.fig`、`.penpot`。

### `render <文件> -o <输出.png> [--scale N] [--page 名称]`

整页加上背景，输出 png、jpg 或 webp。很大的页会缩小到不超过 16384 像素。

### `run <脚本.json|脚本.jsonl> [--in <文件>] [--out <文件>]`

在无窗口会话里跑命令。新文档，或用 `--in` 打开已有文件。每步一行 JSON。整份脚本是**一个撤销步**，并且要么全部成功，要么全部不做。

一步可以写 `"as"`，后面的步用 `{"$ref": "名字"}` 引用它的 `id`。规则与 [mcp.md 的步骤引用](../mcp.md#step-references) 相同。

```json
[
  {"command": "node.create", "as": "card", "params": {"type": "frame", "name": "Card", "width": 200, "height": 100}},
  {"command": "node.create", "params": {"type": "text", "parent": {"$ref": "card"}, "text": "Hi"}}
]
```

```sh
fig run build-card.json --out card.figcraft
```

### `commands [--filter 文本]`

列出命令 id、标签和参数说明。`--filter` 按 id 或标签筛选。

```sh
fig commands --filter autoLayout
```

### `mcp`

给代理的 MCP 服务，走标准输入输出。

```sh
fig mcp
fig mcp --attach 47800
```

`--attach` 接到已经开着的桌面版（启动时加 `figcraft --control 47800`）。不带 `--attach` 时自己开一个无窗口会话。详见 [mcp.md](../mcp.md)。

## Windows

安装包：`dist\release\figcraft-<版本>-windows-x64.msi`。

- **Everyone on this computer**：安装目录写入系统 PATH，需要管理员。
- **Only for me**：写入当前用户的 PATH。

装完要新开一个终端，旧终端看不到新的 PATH。`fg` 不是短命令，那个名字和 shell 里的前台任务冲突。
