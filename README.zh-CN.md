<p align="center">
  <a href="https://getartcraft.com/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/storytold/vectorcraft/main/docs/brand/artcraft-logo-white.png">
      <img alt="ArtCraft" src="https://raw.githubusercontent.com/storytold/vectorcraft/main/docs/brand/artcraft-logo.png" width="200">
    </picture>
  </a>
</p>

<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/ai.storyteller.figcraft.png" width="128" height="128" alt="FigCraft 图标">
</p>

<h1 align="center">FigCraft</h1>

<p align="center">
  <a href="README.md">English</a>
</p>

<p align="center">
  <b>界面设计与原型：开源、可协作，用纯 Rust 写成。</b><br>
  画框、自动布局、组件与变体、矢量网络、样式、变量和原型，<br>
  在一个打开即用、可离线工作的原生应用里。开源，归你所有。
</p>

<p align="center">
  <img alt="100% Rust" src="https://img.shields.io/badge/100%25-Rust-b7410e?style=flat-square&logo=rust">
  <img alt="macOS · Windows · Linux · FreeBSD · Web" src="https://img.shields.io/badge/macOS%20%C2%B7%20Windows%20%C2%B7%20Linux%20%C2%B7%20FreeBSD%20%C2%B7%20Web-native-0EA5C6?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-3a3a3a?style=flat-square">
  <img alt="Status: early alpha" src="https://img.shields.io/badge/status-early%20alpha-d69e2e?style=flat-square">
</p>

<p align="center">
  <a href="docs/zh-CN/README.md"><b>中文文档</b></a> ·
  <a href="docs/zh-CN/our-fork.md">我们的仓库怎么同步</a> ·
  <a href="https://getartcraft.com/apps/figcraft">官网</a> ·
  <a href="https://discord.gg/artcraft">Discord</a>
</p>

<br>

<p align="center">
  <img src="docs/images/shot-1-hero.png" alt="FigCraft 深色主题下编辑徒步应用的三个手机屏幕" width="100%">
  <br><sub><b>Ridgeline</b>：用组件和自动布局做的屏幕，一套颜色变量同时有浅色和深色 · <code>examples/ridgeline-app.figcraft</code></sub>
</p>

<br>

<table>
  <tr>
    <td width="25%" valign="top">
      <h3>摆在你熟悉的位置</h3>
      工具栏、图层面板、设计面板、菜单和快捷键都在习惯的地方。
    </td>
    <td width="25%" valign="top">
      <h3>原生，而且快</h3>
      Rust 引擎加上持久化文档模型：改动只碰变化的部分。没有 Electron，也没有网页套壳。
    </td>
    <td width="25%" valign="top">
      <h3>离线，也私密</h3>
      文件是你磁盘上的 <code>.figcraft</code> 包。没有账号，没有遥测。不分享就不会连服务器。可进出 SVG、PNG、JPG、WebP 和 PDF；<code>.fig</code> 能打开（尽力而为）。
    </td>
    <td width="25%" valign="top">
      <h3>给代理用的</h3>
      每个菜单、工具和拖拽都是一条命令。界面、命令行、JSON 控制通道和 MCP 走的是同一套命令。
    </td>
  </tr>
</table>

## 功能

<table>
  <tr>
    <td width="50%" valign="top">
      <h3>自己排好的布局</h3>
      两个方向的自动布局，可换行，也有网格（固定、适应和 <code>fr</code> 轨道，格子可以跨列）。内边距、间距、对齐（含文字基线）、适应内容 / 填充容器 / 固定尺寸、最小和最大尺寸、绝对定位。画布上可以直接拖内边距和间距。其余用约束和布局网格。
    </td>
    <td width="50%" valign="top">
      <h3>能铺开的组件</h3>
      主组件、带覆盖且改主组件后仍在的实例、变体组件集、属性、替换实例、分离、重置覆盖、转到主组件。资源面板有实时缩略图、搜索，可拖到画布上的画框和自动布局里。本地样式和变量（颜色、数字、字符串、布尔）有模式、别名和作用范围，在变量表里编辑，可绑到填充、描边、效果、布局网格和实例属性。
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <h3>矢量</h3>
      真正的矢量网络：钢笔和铅笔，矢量编辑模式（延伸和分叉路径、弯曲、油漆桶填闭合区域、套索、剪切）。点可以镜像手柄、单独设圆角。连接、拆开、删除时愈合。形状生成器在重叠区域上点或拖来合并，按住 Alt 去掉。布尔组（合并、减去、相交、排除）的操作数仍可在画布上改，结果跟着动。还有平滑圆角矩形、多边形、星形、圆弧和圆环。
    </td>
    <td width="50%" valign="top">
      <h3>颜料和效果</h3>
      纯色，以及线性、径向、角度和菱形渐变，手柄在画布上。图片填充（填充、适应、裁剪、平铺）可调整并保存预设。多个填充和描边，各种混合模式。外阴影和内阴影、图层模糊和背景模糊、噪点、纹理和玻璃、蒙版。文件可以是 sRGB 或 Display P3，导出带上文件的色彩配置。
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <h3>文字</h3>
      直接在画布上编辑（支持输入法），一段里可以混用样式。自动宽度和自动高度、行高、字距和段距、装饰、大小写、列表、OpenType、可变字体轴和文本样式。从右到左和双向文本（阿拉伯文、希伯来文）按 Unicode 双向算法排版。用 craft-fonts 里的开源字体，再加上你的系统字体。
    </td>
    <td width="50%" valign="top">
      <h3>原型和交付</h3>
      连接、触发、动作和智能动画式过渡。变量可以写条件。有演示视图、设备框和检查面板。按住 Alt 在画布上量距离和内边距。开发模式有标注、待开发列表和改动对比，并能生成 CSS、HTML、React、Vue、SwiftUI、Compose、ArkUI、Flutter 等代码。评论可以钉在画框上。任意图层可按比例导出为 PNG、JPG、WebP、SVG 或 PDF。
    </td>
  </tr>
</table>

## 给代理用

菜单、工具和手势都进同一套命令登记（400 条以上）。界面、命令行、JSON 控制通道和 MCP 调的是同一批命令。Windows 上安装之后，短命令 `fig` 和 `figcraft-cli` 是同一个程序。

```sh
fig render landing.figcraft -o landing.png --scale 2
fig export landing.figcraft -o out/ --format svg
fig run build-card.json --out card.figcraft
fig commands --filter autoLayout
fig mcp
fig mcp --attach 47800
```

桌面版还可以只在本机开一个控制通道：`figcraft --control <端口>`。说明见 [中文文档索引](docs/zh-CN/README.md)、[`docs/cli.md`](docs/cli.md) 和 [`docs/mcp.md`](docs/mcp.md)。

## 开始使用

从源码运行（需要 Rust）：

```sh
git clone https://github.com/sn845fqg6k-cpu/figcraft.git
cd figcraft
cargo run --release -p figcraft
```

界面和文字工具的字体来自 [craft-fonts](https://github.com/storytold/craft-fonts)，克隆到本仓库旁边的 `../craft-fonts`。没有它就用系统字体。Windows 上简体中文界面会用微软雅黑。语言在偏好设置里选，系统语言是简体中文时会自动用简体中文。

Windows 安装包把桌面版和命令行放在一起，并把安装目录加进 PATH。装给所有用户时写入系统 PATH，只装给自己时写入用户 PATH。装完新开一个终端：

```bat
fig --version
figcraft-cli --version
```

我们自己的仓库怎么提交、以及怎么把上游更新合并进来，写在 [`docs/zh-CN/our-fork.md`](docs/zh-CN/our-fork.md)。英文在 [`docs/our-fork.md`](docs/our-fork.md)。

> **状态：** FigCraft 仍是 **alpha**。日常设计流程在它自己的文件上能走通，`.fig` 只是尽力打开，多人协作还是预览。和现有设计工具的差距见 [路线图](ROADMAP.md) 和 [`docs/target-app-parity.md`](docs/target-app-parity.md)（这两份仍是英文，因为要跟着上游逐行更新）。

## 隐私和在线分享

默认可离线：没有账号，没有遥测，不分享、不加入文件就不会连服务器。在线会话是预览。

- **分享 ▸ 在线** 会把文件上传到服务器。默认是 `collab.craftapps.dev`，也可以在偏好设置里改成你自己的。
- **不是端到端加密。** 到服务器的连接有 TLS，但服务器上的文件不加密，运行服务器的人可以读到。机密内容不要交到你不信任的服务器上。
- **代码或链接就是权限。** 没有账号。有编辑链接的人能改文件，有查看链接的人能打开。停止分享会收回这些链接。
- **分享 ▸ 本地网络** 不经过服务器，但也没有加密，只在你信任的网络上用。

## 许可

FigCraft 以 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 双许可，任选其一。版权所有 (c) 2026 ArtCraft Team 与 FigCraft 贡献者。必要声明在 [NOTICE](NOTICE)。图标在代码里绘制，或为本项目原创。资源及其许可列在 [ATTRIBUTION.md](ATTRIBUTION.md)。

`docs/brand/` 里的 ArtCraft 名称和标志是商标，不在上述许可里。Fork 和修改版必须去掉它们。

英文原文：[README.md](README.md)。
