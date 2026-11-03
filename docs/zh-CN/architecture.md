# 架构

FigCraft 怎么搭起来的。引擎在前，界面很薄、可以换掉，用户能看见的行为都是命令。数据模型是一棵带自动布局、组件和变量的场景图。英文原文和修订记录在 [architecture.md](../architecture.md)。

`cargo xtask layers` 检查分层（`xtask/src/layers.rs`）：一个 crate 只能依赖更低的层。L6 以下不依赖 egui、eframe、rfd 或 winit，所以界面 crate 可以换。

| 层 | Crate | 做什么 |
|---|---|---|
| L0 | `figcraft-geom` | 仿射变换，圆角、平滑角、多边形、星形、圆弧的路径 |
| L0 | `figcraft-color` | 颜色（HSB/HSL、LCH/OKLCH、hex/CSS、Display P3）、颜料、混合模式 |
| L1 | `figcraft-doc` | 文档：页、持久化节点表、属性、组件、样式、变量、原型、评论、版本 |
| L2 | `figcraft-layout` | 自动布局（横、竖、换行、网格；适应 / 填充 / 固定）、约束、布局网格 |
| L2 | `figcraft-text` | 字体（可选嵌入 craft-fonts，加上系统扫描）、整形、双向文本、行布局 |
| L2 | `figcraft-collab` | 协作核心，不含网络：变更集、顺序、校验、客户端对齐 |
| L2 | `figcraft-plugin` | 插件清单和 WebAssembly 沙箱（有内存上限，不能碰文件、网络或时钟） |
| L2 | `figcraft-pathops` | 布尔运算、描边轮廓、箭头 |
| L3 | `figcraft-render` | 用 tiny-skia 把场景画成像素：混合、效果、蒙版、阴影、模糊、玻璃、文字 |
| L3 | `figcraft-format` | 自己的 `.figcraft`（`document.json` 加图片的 zip），以及 REST 文件 JSON 导入 |
| L4 | `figcraft-svg` | SVG 导入和导出 |
| L4 | `figcraft-pdf` | PDF 导出 |
| L4 | `figcraft-vectordrawable` | Android Vector Drawable（XML）导出 |
| L5 | `figcraft-engine` | 会话、撤销、命令登记、工具、吸附和测量、组件同步、原型、开发模式代码、菜单和快捷键 |
| L6 | `figcraft-ui-egui` | 界面：画布、工具栏、图层、设计 / 原型 / 检查面板、菜单、对话框、评论、无障碍、控制通道 |
| L6 | `figcraft-mcp` | MCP 服务 |
| L6 | `figcraft-collab-net` | 实时会话的网络层。唯一允许依赖网络库的 crate |
| 应用 | `figcraft`、`figcraft-cli`、`figcraft-web`、`figcraft-server` | 桌面版、命令行、浏览器版、协作服务器 |

## 文档

一份 `Document` 有页，以及一张持久化的节点表：画框、编组、分区、矩形、椭圆、多边形、星形、直线、矢量网络、带样式的文本、布尔组、组件、组件集、实例、切片。还有样式、变量集合、评论、版本，以及按哈希存放的图片。实例记着主组件的 id 和每一层的覆盖。快照便宜（`Arc` 和结构共享），所以撤销不限步数，一次手势一步。

## 命令

用户能看见的行为都是 `crates/engine/src/cmd/*` 里的一条命令，有 id、标签、参数说明和是否可撤销。菜单、快捷键、操作面板、命令行（`fig run`）、JSON 控制通道（`--control <端口>`）和 MCP 分发的是同一批 id。插件在清单允许的权限里也调它们。命令自己不弹对话框，只请求界面；程序调用会丢掉这些请求。

## 渲染和文件

画布、导出和浏览器的像素都来自同一个 CPU 渲染器。桌面画布在工作线程上按设备像素比画，平移或编辑时只重画碰到的区域。选区、手柄、参考线画在上面。

`.figcraft` 是自己的格式（原子保存、自动保存；崩溃或强制退出后，桌面版会用恢复副本打开未保存的文件）。导入包括 REST 文件 JSON、SVG、位图和剪贴板。导出包括 PNG、JPG、WebP、SVG、PDF 和代码。`.fig` 能尽力打开，写 `.fig` 仍是实验性的。详见 [file-format-parity.md](../file-format-parity.md)。

## 不崩溃

工作区 lint 禁止在发行代码里使用 `unwrap`、`expect`、`panic!`，并且禁止 `unsafe`。`figcraft_engine::guard` 把一次命令里的 panic 变成错误并回滚文档。桌面版每一帧也有保护。浏览器版抓不住 panic（wasm32 会中止），panic 钩子会把未保存的文件留在本地存储里，下一页还能打开。
