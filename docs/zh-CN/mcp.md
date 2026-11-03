# FigCraft 的 MCP 服务

[English](../mcp.md)

FigCraft 带一个 [Model Context Protocol](https://modelcontextprotocol.io) 服务，让 AI 代理用它做设计：创建和修改图层、运行任意菜单命令、用 JSON 查看结果，并看成 PNG。它在标准输入输出上说换行分隔的 JSON-RPC 2.0（协议 `2025-06-18`；也接受 `2025-03-26` 和 `2024-11-05` 的客户端）。

进来的一行在换行之前最多 **32 MiB**，空白也算。读的时候就检查：超长或没有结尾的一帧会在第一个超出的字节停住，返回 JSON-RPC 错误 `-32600`（`message too large`，id 为空），并关掉这次标准输入输出会话，不再读后面的内容，也不再处理后面的请求。长度以内的行，JSON-RPC 行为和原来一样。

服务在 `crates/mcp`（`figcraft-mcp`），由命令行启动：

| 命令 | 后端 |
|---|---|
| `figcraft-cli mcp` 或 `fig mcp` | **无窗口**：进程里一个引擎会话，一份未命名文档。没有窗口，用 CPU 渲染器画。 |
| `figcraft-cli mcp --attach <端口>` | **接到正在运行的应用**：通过控制通道驱动已经打开的 FigCraft（启动时加 `figcraft --control <端口>`）。你能在窗口里实时看到每一次改动。 |

`--attach` 也可以写成 `主机:端口`。控制通道只监听 127.0.0.1。应用对每个请求最多等 60 秒。客户端一次发很大一批时，用 `--timeout 秒数`（1–600）加长：`figcraft-cli mcp --attach 7979 --timeout 300`。无窗口模式没有这个上限。

代理接到应用上工作时，应用会像实时会话里显示一个人那样显示它：头像用它自己的颜色，名字和评论署名一样（「Claude (agent)」，见下面的评论），出现在右侧面板顶部的人员那一行。画布会用它的名字勾出它上一次编辑改过的图层，或它最后选中的图层。两者都在它最后一条命令之后 8 秒消失，而且只有它正在改的那份文件会显示。这只是显示：没有撤销步，不会保存，不会发给实时会话里的其他人，也不改变代理能做的事。

接到应用上的调用在传输失败后**不会**自动重放：应用可能已经执行了请求，只是回复丢了。错误会说明这次操作可能已经完成；重试修改之前先检查文档。下一次明确的调用会重新连接。如果连不上，错误会说明请求没有发出去。

应用的回复在换行之前最多读 **512 MiB**（最大的窗口截图是 256 MiB 的 base64）。更长的回复会在超出的第一个字节停住，并当作回复丢失：连接关掉，操作可能已经完成，下一次调用再连。更大的导出请给 `export` 一个 `path`，或把缩放降低。

应用在空闲时关掉的连接，会在写下一次请求之前被发现并换掉，所以两次调用之间停一会儿不是错误。

窗口最小化或被别的窗口挡住时，应用仍会应答（见 `docs/control-protocol.md`）：编辑、检查（包括 `inspect_ui`）、指针、按键、文字输入和渲染都照常（最小化时，每个发到应用的请求大约最多 0.1 秒）。只有截取窗口本身（`screenshot` 带 `window`）才需要窗口在屏幕上，否则立刻报错。不带 `window` 的 `screenshot` 改去渲染文档。

## 限制能读写哪些文件夹

下面这些根目录只属于**无窗口**这边：`figcraft-cli mcp` 自己跑的那个会话，没有窗口。用 `--attach` 接上的服务是在应用里执行命令，所以由应用来限制，用的是启动应用时的旗标，不是传给命令行的旗标。

```sh
figcraft-cli mcp --automation-read-root /work/project --automation-write-root /work/project/out
```

```json
{"mcpServers": {"figcraft": {"command": "/path/to/figcraft-cli",
  "args": ["mcp", "--automation-read-root", "/work/project", "--automation-write-root", "/work/project/out"]}}}
```

这两个旗标（其他 Craft 应用也认）把代理限制在你给的文件夹里。每个都可以写多次，写成 `--旗标 目录` 或 `--旗标=目录`：

- 工具调用读的每个文件必须在**读根或写根**里；写的每个文件必须在**写根**里。别的地方会用普通工具错误拒绝，代理能读到并改正，例如 `automation path rejected: /etc/x.png is outside the folders that may be written: /work/project/out`。什么都不会写出去，服务继续运行。
- 读和写是分开的。只有 `--automation-read-root` 时不能往磁盘写（`export` 和导出命令不带 `path` 时仍返回字节）。两个旗标都不给时，什么都不改变。
- 根必须是已经存在的文件夹。少一个，或给的是文件，服务在启动时就停。`initialize` 的 `instructions` 会写出这些文件夹，代理就知道能在哪里工作。
- **管到什么：** 一次调用做的所有事。引擎只通过一个模块碰文件系统（`crates/engine/src/automation/file_access.rs`），`crates/engine/clippy.toml` 拒绝引擎里其他地方的 `std::fs`，所以以后新加的命令也在范围内。包括 `open_file`（以及它可能点名的 `variables` 文件）、`save_file`、`export {path}`，以及经 `run_command` / `batch` 点名文件的每条命令：`file.open`、`file.openRecent`（根以外的最近文件不会被列出、打开、移除或点名；`index` 只数列出的文件）、`file.import`、`file.placeImage`、`image.place`、`file.save`、`file.saveAs`、`file.revert`、`file.recover`、`file.export`、`file.exportSvg`、`file.exportLayers`（以及它创建的文件夹）、`file.exportFramesPdf`、导入 REST API 文件时旁边读到的图片和变量 JSON、`library.add` 和打开的文档链接到的库文件、`variable.import` / `variable.export`、`codeConnect.import` / `codeConnect.export`、`component.playground {path}`、`plugin.import {path}`、开发插件再次从磁盘读的模块、`plugin.directory.publish {path}` 及其资源、`collab.recoverFile {path}`，以及无窗口的带 `path` 的 `ui.render` / `ui.screenshot`。
- **路径怎么解析：** 相对路径先按服务的工作目录变成绝对路径，再顺着链接走。拿来比较和打开的是真实路径。文件还不存在时，解析最近的已有文件夹，剩下的必须是普通名字。所以还不存在的文件夹下面的 `..` 会被拒绝，通向无处的链接也会被拒绝（顺着它写会在它指向的地方创建目标）。根以外的路径，不管那里有什么，拒绝都一样，所以拒绝本身不会告诉你外面有什么。文件夹按整段名字比较：`/work2/x` 不在 `/work` 里。Windows 上大小写以磁盘为准（`C:\WORK\x` 在 `C:\work` 里）。`\\?\` 和 UNC（`\\server\share\…`）和盘符路径一样解析。新名字如果是设备名（`CON`、`NUL`、`COM1`…）或流（`a.png:x`）会被拒绝。
- **网络：** 被限制的调用到不了 FigCraft 或插件服务器，所以不能经它们把文档送到别处。托管、加入、发布和分享（`collab.host`、`collab.join`、`collab.publish`、`collab.share` …）、服务器上的团队库（`library.check` 会拒绝；`library.updates` / `library.list` 对每一项报告这次拒绝，而不是上次取到的版本）以及插件目录，都会以 `automation roots are set: commands don't reach the network …` 拒绝。用这些旗标启动的应用连自己的窗口分享也关掉。一个例外：根在网络共享上（UNC，如 `\\server\share\…`，或已挂载的网络盘）仍然是根，在里面读写信会到达那台文件服务器，保存也会把文档送到那里。只给那些你愿意让代理的文件去的根。
- 不管的：应用自己的文件夹，没有任何工具会点名它们（偏好设置、数据恢复副本、从插件目录安装的插件、系统字体）。
- `--attach` 再带这两个旗标之一是用法错误：命令在应用里跑，只有应用能限制它们。用同样的旗标启动应用：`figcraft --control 7979 --automation-read-root 目录 --automation-write-root 目录`（见[控制协议](../control-protocol.md#confining-file-access)），再照常 attach。接上之后的 `screenshot` 从应用拿到 PNG 数据，不会在任何地方写临时文件。
- 路径是先检查再打开的。另一个程序在中间把文件夹换成链接，仍可能抢赢这次竞态。根把代理限制在交给它的文件上，管不了这台电脑上的其他软件。如果这很要紧，用一个改不了这些根文件夹的用户来跑服务。

## 字体

无窗口服务排文字用的字体和应用一样：编进 FigCraft 的字体，加上已安装的系统字体。第一次排文字时建一次索引。从不碰文字的会话不用付这个代价。所以文字尺寸、自动布局和导出，和用户在应用里看到的一致。要在每台机器上得到同样的渲染（例如 CI），用 `figcraft-cli mcp --no-system-fonts` 或设置 `FIGCRAFT_NO_SYSTEM_FONTS=1`：文字只用内置字体。接到应用上时，用的是应用自己的字体。浏览器版没有系统字体。

## 配置客户端

先编译命令行（`cargo build --release -p figcraft-cli`），用 `target/release/figcraft-cli` 的绝对路径。Windows 安装包把 `fig` 放进 PATH 之后，`command` 也可以写成 `fig`。

Linux AppImage 里带了命令行：`FigCraft.AppImage cli <参数>` 就是 `figcraft-cli <参数>`，所以下面凡是 `/path/to/figcraft-cli mcp` 的地方，都可以写成 `/path/to/FigCraft.AppImage cli mcp`。

### Claude Code

```sh
# 无窗口
claude mcp add figcraft -- /path/to/figcraft-cli mcp
# 驱动已经打开的应用（先用 figcraft --control 7979 启动）
claude mcp add figcraft-app -- /path/to/figcraft-cli mcp --attach 7979
```

或写在项目的 `.mcp.json` 里：

```json
{
  "mcpServers": {
    "figcraft": { "command": "/path/to/figcraft-cli", "args": ["mcp"] }
  }
}
```

Windows 上如果 `fig` 已在 PATH 里：

```json
{
  "mcpServers": {
    "figcraft": { "command": "fig", "args": ["mcp"] },
    "figcraft-app": { "command": "fig", "args": ["mcp", "--attach", "7979"] }
  }
}
```

### Claude Desktop

`claude_desktop_config.json`（设置 > Developer > Edit Config）：

```json
{
  "mcpServers": {
    "figcraft": { "command": "/path/to/figcraft-cli", "args": ["mcp"] },
    "figcraft-app": { "command": "/path/to/figcraft-cli", "args": ["mcp", "--attach", "7979"] }
  }
}
```

## 代理必须知道的约定

- 坐标是**页面单位**（缩放 100% 时 1 单位 = 1 像素），x 向右，**y 向下**。
- `create_node` 和 `set_properties` 的 `x`/`y` 是**相对父画框**的。没有父画框时就是页面，那时它们就是页面坐标。编组和布尔组没有自己的坐标系：编组里的图层放在编组所在的画框（或页面）里。`inspect_document` 用同样方式报告 `x`/`y`，另外给出页面坐标的 `absoluteBounds`。在 `props` 里，`fill` 和 `stroke` 也接受 `"none"`（大小写都行）或 `null` 表示没有颜料，和空的 `fills`/`strokes` 列表一样，所以一次调用就能做出只有轮廓的形状。
- 画框、编组、分区和蒙版：用 `pointer` 工具在图层周围画一个画框或分区，会把完全在里面的图层收进去。分区只能在页面上或分区里（别处的移动、拖拽、粘贴和新分区会被拒绝，或落到页面上）。改变分区大小时，里面的东西留在原地。按住 `cmd` 改变画框大小也一样（忽略约束）。蒙版藏住的图层在那里点不到。蒙版组的盒子和导出是蒙版自己的（再加上蒙版下面的图层）。
- 自动布局画框的子图层由布局定位：设 `layout`、`gap`、`padding`、`mainAlign`、`crossAlign`、`sizingH`/`sizingV`，不要设 x/y。同一次调用里，`layout` 先于调节它的键生效，所以 `{"layout": {"mode": "vertical"}, "gap": 8}` 会保留间距。`layout` 为 `null` 或 `"none"` 时，旁边的自动布局键会被拒绝。更多自动布局属性：
  - `sizingH` / `sizingV`：`fixed`（固定）、`hug`（适应内容；自动布局画框和文本）或 `fill`（填充；自动布局流里的子图层）。子图层不能在父图层适应的那根轴上填充：设成 `fill` 会把父图层那根轴改成 `fixed`；父图层设成 `hug` 会把正在填充它的子图层改成 `fixed`。移出自动布局的图层（或它所在画框失去自动布局）保持尺寸，记为 `fixed`。填充的子图层像 CSS flexbox 那样分空间，受最小/最大尺寸限制。
  - `minWidth` / `maxWidth` / `minHeight` / `maxHeight`（像素，`null` 去掉）限制适应、填充和固定尺寸。适应的文本比最大宽度还宽时会换行。
  - `absolute: true` 把子图层拿出流（它留在原地，画框改变大小时按它的 `constraints` 走；填充变成固定）。
  - `mainAlign` 可以是 `min`、`center`、`max`，或自动间距：`spaceBetween`（两端没有空隙）、`spaceEvenly`（每个空隙和两端一样）、`spaceAround`（空隙是两端的两倍）。自动间距不会小于 0。
  - `padding` 不会让画框比它更小：固定或适应的画框（或最大尺寸更小的画框）会放大到能放下内边距。
  - `strokesIncluded`（新的自动布局默认 `true`）：画框自己的内侧描边像内边距一样占空间（按边用 `strokeSides`）。居中和外侧描边从不占。
  - `reverseZ`（画布叠放）：`true` 时第一个子图层画在上面、也先被点到；`false`（默认）是最后一个。图层顺序不变。
  - `crossAlign: "baseline"` 在水平布局里对齐第一行文字的基线（其他图层对齐到顶；嵌套的自动布局画框用它第一个子图层的基线）。
  - `wrap` 配 `crossGap`（别名 `rowGap`）是换行后各行（水平）或各列（垂直布局从上到下折成新列）之间的间距。
  - `object.convertToGroup` / `object.convertToFrame` 把画框就地变成编组，或变回来（图层留在原地。编组没有自动布局、填充、描边、圆角、裁剪或网格）。
  - `object.convertToSection` 把页面上或分区里的画框就地变成分区；`object.convertToFrame` 把分区变回画框（保留填充，不裁剪）。
  - `contentAlign`（会换行的布局）：`auto`（默认）按 `crossAlign` 所说的起点、中间或末尾，把各行以 `crossGap` 排开（全是填充子图层的行会拉伸）。`spaceBetween` 把空闲空间放在行与行之间（只有一行，或没有空闲空间时，和 `auto` 一样）。
  - 网格（`layout: "grid"`）：`gridColumns`、`gridRows`（最少行数，行会增长）、`columnGap` / `rowGap`。轨道尺寸 `columnTracks` / `rowTracks` 写成 `[120, "1fr", "auto"]`（像素固定、`fr` 按份填充、`auto` 适应），或用命令 `layout.setTrack {axis, index, sizing, value?}`。子图层上，`gridColumn` / `gridRow` 是从 0 开始的锚格（`null` 表示自动放置），`gridColumnSpan` / `gridRowSpan` 是跨度，`gridAlignH` / `gridAlignV`（`min`、`center`、`max`；`auto` 或 `null` 用画框的对齐）把图层在格子里对齐。`fr` 轨道像 CSS 的 `fr`：轨道不会小于它的内容（固定尺寸的子图层比它那一份更大时保留这个尺寸，其余 `fr` 轨道分剩下的）。`inspect` 会报告网格子图层算出来的 `gridCell`。
- 图层顺序：`node.reparent {ids?, parent, index?}` 把图层移进 `parent`（在画布上保持位置，或在它的自动布局流里占位）。`node.reorder {id, index}` 在同一个父图层里移动一个。`index` 从父图层的子图层底部数起（自动布局里是流中的第一个），是移动之后（第一个）被移动图层所在的位置，无论是同一个父图层还是新的。多个图层按它们的叠放顺序一起落下。没有 `index`，或 `index` 超过末尾，就放到最上面（流中的最后一个）。
- **每个菜单动作都是一条命令**（`list_commands`）。多数命令在给了 `ids` 时作用于这些 id，否则作用于当前选择。新建的图层成为选择。
- 颜色是 `"#RRGGBB"` 或 `"#RRGGBBAA"`。在填充和描边里面，`color` 也可以是十六进制字符串，渐变可以写成 `{"type": "linearGradient", "stops": ["#7048E8", "#1098AD"]}`。引擎按它的颜料 JSON 来读，所以 `run_command`、`figcraft-cli run` 和控制通道也接受。经 MCP 工具传的效果也可以用十六进制 `color` 字符串。
- **混合模式**：图层上的 `blend`（`node.set`）或颜料上的 `blend`（`fill.set` / `stroke.set`）是菜单标签或 CSS 关键字（`"Normal"`、`"pass through"`、`"multiply"`、`"plusDarker"` …；`normal` 就是 Normal）。编组和画框一开始是穿透：子图层和后面的东西混合，裁剪也穿透（SVG 导出把裁剪画框的裁剪放在它装的内容上，因为被裁剪的编组会把它隔离）。Normal 先让子图层彼此混合（SVG 的 `isolation:isolate`，PDF 的隔离组）。Plus darker 在渲染和像素导出里是精确的。PDF 没有这种模式（用 multiply）。SVG 写出 CSS 关键字 `plus-darker`。检查图层时，颜料的混合不是 normal 就会报告 `blend`。
- **投影**藏在图层盖住的东西后面，包括它的填充（半透明和 0% 的也算）和描边，除非效果有 `showBehind: true`。编组，以及像素被图层模糊或纹理挪动过的图层，改藏在它们的像素后面。SVG 导出以同样方式挖掉它们，滤镜区域和效果伸到的范围一样大。
- **色彩配置**：颜色值在文件的配置里，sRGB（默认）或 Display P3（`inspect_document` 报告 `colorProfile`）。`file.colorProfile {profile: srgb|displayP3, convert?}` 更改它。`convert`（默认 true）转换每个颜色值，让颜色看起来不变（颜色计算的两端保持精确，中间的颜色会稍微移动）。`false` 保留数值并重新解释。图片从它们自己的色彩配置转换进文件的空间：嵌入的 ICC（PNG、JPG、WebP、TIFF；RGB 或灰）、PNG 的 `sRGB` / `gAMA` / `cHRM` 块，否则当 sRGB（CMYK 或损坏的配置也一样）。`image.place` 把配置的名字报告为 `imageProfile`（`null`：没有能用的，当作 sRGB）。PNG 导出会打标签（`sRGB` 块，或 Display P3 的 ICC 再加上 `cICP`）。Display P3 的 JPG 和 WebP 嵌入 ICC。SVG 和 PDF 把 Display P3 文件转换成 sRGB 再导出（色域外的颜色被裁掉）。应用的画布显示 sRGB（屏幕上 P3 在 sRGB 之外的颜色会被裁掉）。`view.proofSrgb {on?}`（视图 ▸ 软打样 sRGB）给这些像素画条纹，这样 `screenshot {window: true}` 能看出它们在哪（下一次画布渲染之后）。取色器对这种颜色会说 Outside sRGB。
- **图片调整**：图片颜料带 `filters`（曝光、对比度、饱和度、色温、色调、高光、阴影；各自 -1 到 1）。`image.adjust {ids?, target?: fill|stroke, index?, preset?, filters?}` 应用内置预设（`vivid`、`muted`、`warm`、`cool`、`blackAndWhite`、`fade`、`reset`）或文件里保存的预设，再叠上任何 `filters` 值。`image.replace {ids?, target?: fill|stroke, index?, data | path}` 把另一张图片放进颜料（默认每个图层最上面的图片颜料），保留填充模式、裁剪、位置和调整。`index` 处不是图片的颜料会变成图片，铺满图层。`image.savePreset {name, filters?}` 把当前（或给定）的调整存进文件。`image.deletePreset {name}` 删一个。`image.presets` 列出两种。`image.crop {ids?, index?}`（一个图层）把它最上面可见的图片填充（或填充 `index`）切换到裁剪，图片留在原地，返回 `{id, index}`。应用也会在画布上进入裁剪模式。裁剪本身是颜料的 `crop` 变换（`fill.set`）。
- 文本属性（`fontFamily`、`fontStyle`、`fontSize`、`lineHeight`、`letterSpacing`、`textDecoration`、`textCase`、`textFills`、`fontVariations`…）作用于整个图层，或在给了 `start`..`end` 时作用于这些字符（`text.style {id, start, end, …}` 做同样的事）。样式范围以 `styledRuns` 返回。可变字体：`fontVariations`（别名 `variationSettings`）用用户单位设轴，例如 `{"wght": 550, "wdth": 85}`（`null` 回到样式名隐含的值）。`text.setVariations {variations, replace?}` 是合并而不是替换。`text.fonts {family, style?, axes: true}` 列出一种字体的轴和具名实例（具名实例也是合法的 `fontStyle`）。`text.missingFonts` 列出文档点了名但没安装的字体族（`{families: [{family, layers, source}]}`）。它们的文字用后备字体画。在桌面版，`font.download {family}` 从字体库后台安装 `source` 不是 null 的字体族（Google Fonts、Noto）：固定、校验过校验和的文件，放进各 Craft 应用共用的字体文件夹。只有用户允许下载字体时才行，而且只有用户能在应用的「缺失字体」窗口里允许；否则拒绝。再调用一次看进度（`{state: downloading, done, total}`）。装好之后，这次调用会让打开文档的文本适应这套字体（`{state: installed, refitted}`）。`font.downloads` 列出下载，以及是否允许下载（`{allowed, downloads: [{family, state, error?}]}`）。字体每个进程只索引一次。装好一种之后，`text.rescanFonts` 会捡起来（`{added: [字体族]}`）。`text.loadFont {path | data (base64)}` 加入一个字体文件但不安装（`{families}`）。文本框保持测量时的尺寸：两者都会让打开文档里这些新字体族的自动尺寸文本重新适应（`refitted: [ids]`）。`text.refit {ids?}` 对任意文本做这件事，例如字体缺失时保存的文件。OpenType 特性：`features {"liga": false, "tnum": true}`。装饰：`textDecorationStyle`（`solid`、`dotted`、`wavy`）、`textDecorationThickness` 和 `textDecorationOffset`（`auto`、像素，或字号的 `"10%"`；偏移从基线向下到下划线的上沿）、`textDecorationSkipInk`（下划线在降部周围断开）和 `textDecorationColor`（一种颜色，或 `auto` 用文字的颜色）也是字符属性。
- **列表**按段落：`listStyle`（`none`、`bulleted`、`numbered`）和 `indentation`（嵌套层级，0–8）作用于字符 `start`..`end` 碰到的段落（没有范围就是全部）。`get_node` 对图层报告一个 `listStyle`，或按段落报告 `lineTypes` 和 `lineIndentations`，编号列表从大于 1 开始时还有 `listStartOffsets`（来自 `.fig` 文件；1 显示成 "2."）。编辑文本时，`text.bulletList`、`text.numberList` 和 `text.listStyle` 作用于选中的段落。图层属性：`listSpacing`（列表项之间的像素，代替 `paragraphSpacing`）、`hangingList`（标记在盒子外面）和 `hangingPunctuation`。编辑时，`key` 的 Tab / Shift+Tab 把一项嵌深或移出；空项上按 Enter，以及在项的开头按 Backspace，会退出列表；在段落开头输入 `- `、`* ` 或 `1. ` 会开始一个列表。
- **文本样式**链的是字符，不只是图层：`style.apply {style, ids?, start?, end?}` 用文本样式链上字符 `start`..`end`（正在编辑文本时是选中的字符，否则是整个图层）。`style.detach {which: "text", start?, end?}` 解开它们（值留下）。图层的 `styles.text` 是它所有字符共用的样式（不一致时没有）。`get_node` 把链上的字符列为 `textStyleRanges`（`[start, end, 样式 id]`）。在已链接的字符上改文本样式所设的东西（字体、字号、行高、字距、装饰、大小写、轴值）会解开它们。`style.update` 移动每一个已链接的字符。`style.create {type: "text", name}` 在编辑时取选中的字符。`value` 可以用 `node.set` 的写法（`{"fontSize": 32, "lineHeight": "120%", "letterSpacing": "2%"}`）。
- **样式跟图层保持一致**：编辑已套样式的填充、描边、效果或布局网格列表（`node.set`、`fill.set`、`fill.add`、绑定的变量…）会解开那个样式（`styles.fill` 等变成 `null`），这样以后的 `style.update` 不会盖掉这次编辑。`style.apply` 会替换绑在样式所设内容上的变量。文本样式只上到文本图层。同一次调用里，样式链接（`fillStyle`、`textStyle`…）优先于它设置的值。要偏离，放到下一次调用里，那次会解开它。名字按 `/` 分成的每一段文件夹名去掉首尾空白。`fill` 或 `stroke` 也可以绑变量：`node.create {type: "rectangle", props: {fill: {"$var": "Theme/surface"}}}` 在一个撤销步里创建并绑定，`node.set` 接受同样的包装。`$var` 先认精确的变量 id，否则认没有歧义的精确名字或 `集合/名字`（名字里可以有 `/`）。未知或有歧义的名字是错误。包装在字面值和样式链接之后应用，并且即使颜色没变也会清掉那块颜料的样式链接。属性里只有 `fill` 和 `stroke` 接受变量；其他字段用 `variable.bind`。`style.move {styles, before?, folder?}` 重排（有 `folder` 时改名进那个文件夹）。`style.duplicate {style}` 在紧下面加「<名字> copy」。`style.renameFolder {folder, name}` 重命名文件夹。
- **变量**：`variable.bind` 拒绝字段不接受的变量类型（颜色绑到 `width`）。作用范围只过滤选择器。`variable.delete` / `variable.deleteCollection` 解开使用它们的图层（图层保留最后的值），并把指向它们的别名换成它们的值。明确改掉已绑定字段的值的编辑（`node.set`、`transform.resize`、`object.opacityKeys`、`text.style`、`text.set`、字号、字重和字距快捷键、画布拖拽）会分离该字段的变量；这次编辑没动的字段保持绑定。`variable.addMode {from}` 复制一个模式。`variable.moveMode {collection, mode, to}` 重排（索引 0 是默认模式，设为自动的图层跟随它）。`variable.update {codeSyntax: {WEB?, iOS?, ANDROID?}}` 设置代码名（`""` 清除）。生成的 CSS 用 Web 的，SwiftUI 用 iOS 的，Compose 用 Android 的（没有的变量在那里保留它的值）。
- **实时预览**：`text.preview {command, params}` 在画布上显示 `style.apply`、`library.applyStyle`、`text.setVariations` 或 `text.style` 会做成什么样，没有撤销步（代替更早的一次预览）。`text.endPreview` 把文档放回去。任何其他命令会先结束预览，所以真正运行那条命令时，它作为一个撤销步提交。
- 每次修改是一个撤销步（`batch` 把好几次合成一步）。失败的命令让文档保持原样。
- 程序调用从不打开对话框。
- 核对你的工作：数字用 `inspect_document` / `get_node`，像素用 `screenshot`。

不改变选择就找图层，用 `run_command` 调 `node.find`，例如 `{"command": "node.find", "params": {"root": 12, "type": "TEXT", "text": "hello", "limit": 20}}`。它搜索 `root` 的严格后代，没有 root 就搜当前页，按存储的子图层顺序（深度优先，父在子前），包括隐藏和锁定的图层以及实例的子图层。`name`、`text`（仅文本图层）和 `component`（实例按其主组件的名字）是不区分大小写的字面子串。`type` 是规范的图层类型，不区分大小写。所有过滤都必须匹配。结果是 `{"ids": [...], "total": N}`：最多 `limit` 个 id（默认 100，0 到 10000），`total` 数全部匹配；`limit: 0` 只计数。另一页上的 root 不用切换页面也能用。未知的 root 或类型、非法的 limit 是错误。在 batch 里，后面的步可以用 `{"$step": 0, "field": "ids/0"}` 指向第一个匹配。

## 工具

| 工具 | 做什么 |
|---|---|
| `list_commands {filter?}` | 每条命令：id、标签、菜单、快捷键、是否可撤销、参数说明。 |
| `run_command {command, params}` | 运行任意命令。例如 `align.left`、`layout.add`、`component.create`、`instance.place`（把实例放到页面上的一个点，落入它下面的画框或自动布局槽）、`component.exposeInstance` / `instance.exposed`（实例上的嵌套实例属性）、`component.makeSlot` / `instance.slotInsert` / `instance.resetSlot`（插槽，见下）、`inspect.code`（一个图层的 CSS、整个画框的 HTML + CSS、React、Leptos、Vue、SwiftUI、Jetpack Compose、ArkUI 或 Flutter；组件把属性当参数，实例是调用，见下面的开发模式）、`inspect.properties`（布局、自动布局、排版、填充、描边和效果，带样式和变量名；`colorFormat`：hex、rgb、css、hsl 或 hsb）、`inspect.assets`（选择下面带导出设置的图层、图标和图片，各自有 `kind`）。`inspect.code` 对 CSS、HTML、React、Leptos 和 Vue 接受 `unit: rem` 和 `rootFontSize`。还有 `style.create`、`variable.bind`、`variable.update`（改名、描述、作用范围）、带 `{alias}` 或颜色计算（`{alpha}`、`{mix}`）的 `variable.set`、`variable.renameMode` / `variable.removeMode`、`variable.deleteCollection`、`proto.interaction.add`、`codeConnect.set`（组件链到真实代码，见开发模式）。 |
| `batch {commands: [{command, params}], label?}` | 几条命令合成一个撤销步（`command.batch`）。不管有多少步，文档只留一份副本（一个 `edit.undo` 步把这个 batch 到目前为止的编辑一起撤销）。对打开的文档及其文档状态，要么全部成功，要么全部不做。失败的内部命令会从日志里去掉。偏好设置、剪贴板、工具状态和界面请求不回滚。一步可以用更早一步的返回：用 `"as": "card"` 命名，后面一步参数里任何地方的 `{"$ref": "card"}` 变成那步结果里的 `id`（`{"$step": n}` 按从 0 开始的位置做同样的事）。`"field"` 取另一个键或斜杠路径，如 `ids/0`（见下面的步骤引用）。 |
| `create_node {type, x, y, width, height, parent, index, name, text, path, props}` | 创建画框、矩形、椭圆、多边形、星形、直线、箭头、文本、矢量、分区、组件或切片。返回 `{id, node}`。 |
| `set_properties {ids, props}` | 对图层做 `node.set`（几何、填充、描边、效果、圆角、自动布局、尺寸、约束、蒙版（`isMask`、`maskType: alpha\|vector\|luminance`）、交付状态（`devStatus: none\|readyForDev\|completed`；再标一次会清除「已更改」）、分区的 `sectionContentsHidden`、文本…）。返回更新后的图层。 |
| `inspect_document {depth, allPages, fields?, outline?}` | 文档的 JSON（它的 `id`、图层树、样式、选择、历史）。`fields` 选择图层属性，`outline: true` 给出紧凑的树（见下）。作用于文档的工具也接受 `document`（见下面的「点名文档」）。 |
| `get_node {id, depth, fields?, outline?}` | 一个图层的 JSON。接受同样的 `fields` 和 `outline`。图层或页面有用 `set_properties {props: {pluginData}}` 写过的图层数据时，带 `pluginData`（`{键: 值}`，值是字符串）。 |
| `select {ids}` | 替换选择（`[]` 取消选择）。 |
| `export {ids, format, scale \| size, path?, bleed?, cropMarks?, bleedMarks?, cmyk?, contentsOnly?, includeBoundingBox?, svgOutlineText?, svgIdAttribute?, svgSimplifyStroke?, vdCompat?}` | png、jpg、svg、webp、pdf、xml（Android Vector Drawable：`svgIdAttribute` 写出 `android:name`。`vdCompat` 为 `bestEffort`（默认：近似并警告）、`exact`（省略并警告）或 `ignore`（不警告：重叠图层的不透明度丢掉，奇偶填充保留为 `fillType="evenOdd"`，菱形渐变省略）。结果用 `warnings` 列出改过或省略的东西），或 `fig`：整份文件写成 `.fig`，实验性（见下面导出 `.fig`）。`size` 是 `2x`、`512w` 或 `512h`。图片盖住图层画出来的范围（阴影朝它们的偏移、描边、未裁剪的子图层、蒙版组的蒙版；隐藏图层不占地方；切片是它那一块加上下面的一切，或在 `contentsOnly` 打开时只包括它所在的画框或编组）。`contentsOnly: false` 还画出那块区域里显示的每一个其他图层。`includeBoundingBox` 导出图层自己的盒子。SVG：`svgOutlineText: false` 保留 `<text>`，`svgIdAttribute` 用图层名写出唯一 id，`svgSimplifyStroke: false` 把每一笔描边写成轮廓。同样的选项进导出设置（`export.add` / `export.update`）。PDF：画框各占一页（尺寸用点），其他东西合成一页。印刷选项：`bleed`（点，或 `"3mm"`、`"0.125in"` 这样的长度）、裁切标记和出血标记、CMYK（见下面的导出）。`file.export`、`file.exportLayers`、`file.exportFramesPdf` 也接受，并由 `export.add` / `export.update` 按每条导出设置保存。`run_command` 的 `file.exportFramesPdf {page?, path?}` 写出一页的每个顶层画框。没有 `path` 时，图片以 MCP 图片内容返回，SVG 以文本返回，XML 以文本再跟上 `{format, bytes, warnings}`。图层自己的导出设置：用 `run_command` 调 `export.add` / `export.update {index, size?, suffix?, format?}` / `export.remove`，以及 `file.exportLayers {ids?, dir?, assets?}`（每个图层按每条设置导出一次，命名 `<图层><后缀>.<格式>`，图层名里的斜杠会建文件夹；`assets: true` 改为导出检查页的资源：图标为 SVG，图片为 PNG）。 |
| `screenshot {ids?, scale?, maxSize?, window?}` | 页面（带背景）或图层的 PNG，缩放到不超过 `maxSize`（默认 1600）。`window: true` 在接上时截取最前面的应用窗口。窗口 id（见下面的窗口）截取那个窗口。 |
| `open_file {path, variables?, reload?}` / `save_file {path?}` | 打开 `.figcraft`、`.json`（FigCraft JSON 或带变量的 REST API 文件 JSON，见下）、`.fig` 或 FigJam 的 `.jam`（尽力，见下）、`.sketch`（Sketch 48 及以后，尽力）、`.svg`（见下面导入 SVG）或一张图片。保存为 `.figcraft`。`open_file` 返回 `opened`（新打开的文件有 `name`、`pages`、`layers`、`images` 的 `summary`，以及 `warnings` 或 `repairs`。已经打开的文件给出它的 `index`，这个标签上次读或保存之后磁盘上的文件变了则 `stale: true`。`reload: true` 再读进这个标签，标签有未保存更改时拒绝）以及文档的 `document` 大纲。 |
| `undo` / `redo` | 在历史里走一步。 |
| `pointer {kind, x, y, to?, steps?, tool?, shift, alt, cmd, pressure?, pressureTo?}` | 用当前工具在页面坐标里做鼠标手势：`click`、`double`、`drag`、`down`、`move`、`up`、`hover`。拖着的图层会落入指针下面的画框或自动布局槽，但编组、布尔组或实例里面的图层（插槽以外）只移动。`pressure`（0–1）是笔压。拖拽从 `pressure` 到 `pressureTo`（见笔刷和笔压）。 |
| `key {key, shift, alt, cmd}` | 像在画布上打的快捷键或键（每个默认快捷键、工具键、Escape、Delete、方向键、不透明度的数字键、没选中东西时平移视图的方向键；编辑文本时先是文本快捷键，然后是文本编辑键）。键名如 `A`、`0`、`Enter`、`ArrowLeft`、`=`、`/`，或 egui 的（`Plus`、`Questionmark`）。`cmd` 在 macOS 上是 Cmd，其他地方是 Ctrl。 |
| `type_text {text, id?}` | 打进正在编辑的文本，或替换文本图层 `id` 的内容。 |
| `inspect_ui {}` | 工具、视图和状态（接上时还有面板、耗时和应用的 `windows`）。 |
| `wait_for_changes {after?, timeoutMs?, topics?, limit?}` | 等待接上的应用里的新评论事件。有待处理事件就立刻返回，否则最多等 `timeoutMs`（默认 20 秒，最大 25 秒）。把返回的 `cursor` 再传回来继续看。无窗口立刻返回。 |

### 紧凑的图层树

`inspect_document` 和 `get_node` 接受 `fields`，一个精确、区分大小写的顶层节点键数组，应用到每个图层和每一页。例如 `fields: ["name", "type", "width", "height"]` 会去掉颜料和文本属性。`fields: []` 只留 id 和结构。`id`、`children`、深度边界的 `childCount` 和传输的 `truncated` 标记总是保留（`truncated` 不是可选字段）。未知的键是错误，并列出合法的键。`fields` 不是字符串数组，或 `outline` 不是布尔值，也是错误。文档元数据不变。

`outline: true` 选择 `id`、`name`、`type`、`characters`、`mainComponentName` 和 `visible`，再加上结构。`characters` 在 120 个字符（Unicode 标量）后截断，只有更长时才在后面加 ASCII 的 `...`。显式选择 `visible`，或通过 outline 选择它，即使图层和页面可见也报告布尔值。没有这些选项时输出保持稀疏。`outline: true` 不能和 `fields` 一起用；`outline: false` 可以。两个选项都保留原有的深度和响应大小限制。

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"inspect_document","arguments":{"depth":6,"outline":true}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_node","arguments":{"id":3,"depth":2,"fields":["name","width","height"]}}}
```

### 起步模板

代理可以从模板开始，不必从空白页开始。`run_command {command: "template.list"}` 返回起步模板，每个是 `{id, name, description, kind, size}`（`kind`：`app`、`website`、`kit` 或 `media`）：`mobile-app`、`landing-page`、`dashboard`、`design-system`、`wireframe-kit`、`social-posts`、`slides` 和 `icon-set`。`run_command {command: "file.newFromTemplate", params: {template, name?}}` 把它做成一份新的、未保存的文档，设为当前文档，返回 `{index, template}`。已经打开的文档不动。模板在代码里用代理也会用的那些命令搭成（带自动布局的画框、组件和实例、颜色和文本样式、带浅色和深色模式的变量、原型链接），所以里面的一切都能改。内容是合成的。搭一份大约 10 毫秒到 190 毫秒。不给 `template` 时，命令返回 `{templates}`（在应用里还会显示主屏幕的模板页）。`help.gettingStarted {show?}` 显示主屏幕的入门提示（`show: false` 关掉）。无窗口时只设置偏好。

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"file.newFromTemplate","params":{"template":"dashboard"}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"export","arguments":{"format":"png","path":"dashboard.png"}}}
```

### 用数据填充副本

`data.fill {id, rows, gap?, formats?, dir?, format?, scale?}` 按每一行把一个画框复制一次（轮播、一组帖子、每项一张卡片），一个撤销步。一行的每个键点名副本里的图层。字符串（或数字）设置它们的文本。`{text?, fill?, visible?}` 设置更多。`name` 给副本起名（默认「<画框> <n>」）。副本向右排成一行，间隔 `gap`（默认 100）。有 `dir` 时，编辑落地后每个副本导出到那里，文件名 `<名字>.<格式>`（默认 png）。返回 `{ids, missing（没有图层叫这个名字的键）, files?, errors?}`。`errors` 把失败的导出列为 `[{id, path, error}]`。副本留下，写成功的文件在 `files` 里。

每行长度会变的文案可以自己定尺寸：`{"Headline": {"text": "…", "fit": {"maxLines": 2}}}` 在文本设好之后对这些图层跑 `text.fit`。

`formats: [{name?, width, height}]` 让每一行按每种尺寸做一次：副本被改变大小，让自动布局重排，命名为 `<名字>-<格式名>`（默认 `<宽>x<高>`），放在这一行上一个尺寸的下面。文本 `fit` 在改变大小之后跑。用途：一次调用把 1080×1350 的帖子变成信息流、快拍（1080×1920）和方形（1080×1080），轮播的每一张都有。画框必须是自动布局（固定位置的子图层留在原地）。

`text.fit {ids?, maxLines?, minSize?, orphans?}` 给固定宽度的文本图层（自动高度或固定尺寸）一个最大的字号，小到 `minSize`（默认当前的 60%）为止，使它不超过 `maxLines` 行，也不超过盒子的高度。每个样式段按同一个倍数缩放。只有一个词的最后一行（孤行）会再缩小最多 8% 来避免，否则用不断行空格把最后两个词留在一起。硬换行之后只有一个词的段落不是孤行。文本从不变大。已经合适的文本保持精确尺寸。尺寸四舍五入到十分之一，且不低于 `minSize`。返回 `{fitted: [{id, fontSize, lines, fits, orphan}]}`。用途：轮播一张写着 "Lighthouse 100. Loads in under 1s."，96px 时断成三行，"1s." 单独一行；用 `fit: {maxLines: 3}` 就变成两行比较均衡的文字。

```json
{"command": "data.fill", "params": {"id": 80, "dir": "out/carousel", "scale": 1, "rows": [
  {"name": "01-cover", "Headline": "Your website and AI chat assistant in 7 days", "Badge": {"visible": true}},
  {"name": "02-proof", "Headline": "Lighthouse 100, loads in under a second", "Badge": {"visible": false}}
]}}
```

### 白板

白板是另一种文件，用来一起想事情：`file.newBoard {name?}` 打开一份（页面是浅色的；`inspect_document` 说 `kind: "board"`）。白板对象是普通图层，带一个 `get_node` 会显示的 `board` 项（`{type: sticky|shape|connector|label|stamp|ink, …}`），所以通常的命令能移动、改变大小、上色、复制、删除和撤销它们。在设计文件上，白板命令会拒绝。在白板上，`proto.*`、`dev.*`、`view.prototype` 和 `view.devMode` 会拒绝。下面每一条都是一个撤销步，并返回它创建的 id：

- `board.sticky {x, y, text?, color?, author?, parent?}` → `{id, text}`：便利贴（方形画框，高度随笔记增长；笔记变长时文字变小）。`color` 是白板颜色（`yellow orange red pink violet blue green gray`）或十六进制。`author` 默认是你的名字（`pref.userName`，或实时会话给你的名字；`false` 表示没有）。
- `board.nextSticky {id?, side?: right|below|left|above, edit?}` → `{id, text}`：在选中的（或正在编辑的）便利贴旁边放一张同样颜色的。`edit: true` 时应用继续在里面打字（应用里 `Shift+Cmd/Ctrl+Enter` 两件事一起做）。在形状上，它做 `board.addConnected` 做的事：下一个形状，用连接线连上（`{id, text, connector}`）。
- `board.addConnected {id?, side?: right|below|left|above, edit?}` → `{id, text, connector}`：在选中的（或正在编辑的）那一个的那一侧，放下一个同样种类、尺寸和颜色的便利贴或形状，距离 80 单位，用连接线连上，方式和连接线工具一样。一个撤销步。`edit: true` 时应用继续在里面打字（应用里选中便利贴或形状旁边的「+」两件事一起做）。
- `board.shape {shape, x, y, width?, height?, text?, color?, parent?}` → `{id, text}`：带居中、会换行文字的形状。`shape` 可以是 `rectangle roundedRectangle ellipse diamond triangle triangleDown parallelogramRight parallelogramLeft trapezoid pentagon hexagon octagon star plus arrowLeft arrowRight chevron speechBubble database document predefinedProcess manualInput internalStorage`。
- `board.connector {from, to, fromSide?, toSide?, fromAt?, toAt?, style?: straight|elbow|curved, startCap?, endCap?, label?}` → `{id, label?}`：`from`/`to` 是图层 id 或页面点 `{x, y}`。边是 `auto`（朝向另一端的那一边）或 `top right bottom left`。`fromAt`/`toAt` 是图层盒子的 `[x, y]` 分数，把端点放在那一部分（它从 `side` 离开）。端帽是描边端帽（默认末端是箭头）。肘形在短路径会穿过所连对象时绕开它们。曲线沿边离开和到达。所连的东西移动或改变大小时，连接线自己重新布线，实时会话里别人移动它时也一样。被删掉的一端留在原地，变成一个点。`board.label {id, text}` 设置它的标签（空则去掉）。
- `board.reattach {id?, end: from|to, target, side?, at?}` → `{id}`：把连接线的一端（默认选中的那条；它的标签代表它）移到 `target`。`target` 是图层 id，或端点保持自由的页面点 `{x, y}`。两端都会落到同一图层上，或目标是连接线时，拒绝。在应用里拖选中连接线的端点手柄做同样的事。按住 Cmd/Ctrl 时，端点落到指针在对象上的位置（`at`，和 `board.connector` 一样）。手工改过形状的路径会回到自己布线。
- `board.reshape {id?, portion, x, y}` → `{id}`：把连接线路径的一段移到那个页面点，和在应用里拖路径手柄一样。`portion` 是肘形的一段（第一段或最后一段会多出一段，好留在它的端点上），或曲线的中段（`portion` 0）。两端离开的边从此保留，形状跟着所连的东西走。直线连接线、它没有的那一段、以及超过 64 段的路径会被拒绝。`board.setStyle {style}` 改成另一种样式会回到自己布线。
- `board.stamp {x, y, stamp: thumbsUp|heart|plusOne|star|question|check}` → `{id}`：居中在那里的圆形徽章，悬停时显示你的名字。
- `board.ink {path | points, highlighter?, color?, select? (默认 true)}` → `{id}`：记号笔（或又宽又透的荧光笔）笔画。
- `board.setColor {ids?, color}`：便利贴和形状把它当填充，连接线和墨迹把它当线条。`board.setStyle {ids?, shape?, fontSize?, style?: straight|elbow|curved, startCap?, endCap?}`：形状的轮廓和文字大小，连接线的布线和箭头。
- `board.templates` → `[{id, name, description, width, height}]` 列出内置模板：`retrospective brainstorm weeklyPlanner flowChart kanban`。`board.template {name, x?, y?}` → `{name, ids}` 放一个（左上角在 `x`、`y`；不给则在视图中间），是普通的分区、便利贴、形状、连接线和文本，被选中，一个撤销步。
- `board.table {rows?, columns?, cells?, x?, y?}` → `{id, rows, columns, cells}`：表格（默认 3×3，最多 50×20。`cells` 是按行的 `[["Name", "Role"], …]`，不给 `rows`/`columns` 时由它决定尺寸）。它是自动布局画框，行画框里放单元格画框，每个单元格一个文本图层。答案里的 `cells` 按行放这些文本图层的 id，所以 `node.set {id, props: {characters}}` 改一个单元格。单元格的行随文本长高。`board.tableEdit {id?, action: addRow|addColumn|removeRow|removeColumn, index?}` → 同样的答案。`index` 是新行或新列放在哪，或去掉哪一个（默认最后一个）。`id` 可以是表格，或表格里的任意图层。
- 给 `pointer` 的工具：`tool.sticky {color?}`、`tool.shape {shape?}`、`tool.connector {style?: straight|elbow|curved}`（从一个对象拖到另一个）、`tool.marker {color?}`、`tool.highlighter {color?}`、`tool.eraser`（在笔画上拖）、`tool.stamp {stamp?}`。文本和分区是通常的工具。

```json
{"command": "file.newBoard", "params": {"name": "Retro"}}
{"command": "board.sticky", "params": {"x": 0, "y": 0, "text": "Went well", "color": "green"}}
{"command": "board.sticky", "params": {"x": 240, "y": 0, "text": "To improve", "color": "pink"}}
{"command": "board.connector", "params": {"from": 2, "to": 4, "style": "elbow", "label": "leads to"}}
{"command": "board.stamp", "params": {"x": 200, "y": 0, "stamp": "plusOne"}}
```

`from` 和 `to` 是两次 `board.sticky` 返回的 `id`（这里在没有作者名的新白板上是 `{"id": 2, "text": 3}` 和 `{"id": 4, "text": 5}`。带作者名的便利贴会多用一个 id）。在 `batch` 里用步骤引用（`{"from": {"$step": 1}, "to": {"$step": 2}}`，第 1 步和第 2 步返回的 `id`）就不用自己数。

在白板上开会：倒计时和投票。两者都是白板的一部分，所以实时会话里每个人看到的一样，而且都不是撤销步（撤销让它们保持原样）。在设计文件上它们会拒绝。

- `board.timer {action: start|pause|resume|add|stop, minutes?}` → `{state, left, text, ends}`：每块白板一个倒计时。`start` 取 `minutes`（默认 5），`add` 加上它们（默认 1），从 1 秒到总共 1440 分钟。`pause` 保留剩余时间，`resume` 再倒计时。计时器保存结束时间（1970 以来的毫秒），暂停时保存剩余时间，每个客户端用自己的钟倒计时。`board.timerState {}` 读取它。`state` 是 `off`、`running`、`paused` 或 `done`（时间到了：有人停止之前显示 0:00）。`left` 是毫秒。`text` 是 `m:ss`（或 `h:mm:ss`）。
- `board.vote {action: start|cast|retract|end|clear|delete, …}` → 投票，格式和 `board.voteResults` 一样。一次只有一个投票。`start {votes?（每人，1–99，默认 3）, on?: all|stickies|selection（默认 all）, ids?（配合 selection；默认选中的图层）}`（已结束的投票可以替换；还在进行的会被拒绝）。`cast {id}` 把你的一票投到一个对象上（同一个对象多票可以；你的票用完就拒绝）。`retract {id}` 收回一票。`end` 显示每个人的票，并把结果留在文件里（保留最近 20 次）。`clear` 把投票和它的结果从白板上拿掉（已保留的结果还在）。`delete {index}` 去掉已保留的结果（0 是最旧的）。`voter`（任何动作）让脚本和测试以别人的身份投票。默认你是你的实时会话客户端，文件没分享时是你的名字（`pref.userName`）。在应用里，投票进行时点一个对象就是投票，点你的徽章（或 Alt 点击）收回这一票。
- `board.voteResults {voter?}` → `{state: off|running|ended, votesPerPerson, on, by, left, mine: [{id, votes}], voters, results?, sessions}`（off 时是 `{state, sessions}`）。进行中你只看到自己的票（`mine`、`left`）以及有多少人投了。结束后 `results` 列出 `[{id, name, votes, rank, voters}]`，票多的在前（平票共用名次）。`name` 是便利贴的文字或图层的名字。`voters` 是谁投了它：`[{voter, name, votes}]`。`voter` 是选票用来键它的（实时会话客户端是 `peer <id>`，否则是名字）。`name` 是他们的称呼（已不在实时会话里的人是 `null`）。`sessions` 列出每一次已结束投票的结果，最新的在前：`[{index（在保留结果里，0 最旧；结果开始保留之前结束的投票是 null）, onBoard, ended（1970 以来的毫秒）, votesPerPerson, on, by, results}]`。投在已删除对象上的票不算。

```json
{"command": "board.timer", "params": {"action": "start", "minutes": 10}}
{"command": "board.vote", "params": {"action": "start", "votes": 2, "on": "stickies"}}
{"command": "board.vote", "params": {"action": "cast", "id": 2}}
{"command": "board.vote", "params": {"action": "end"}}
{"command": "board.voteResults", "params": {}}
```

### 分区、交付和方向键

用 `run_command` 调 `object.wrapInSection {ids?, name?}`（包进新分区：页面上或分区里的图层，周围留一点空）、`section.toggleContents {ids?, hidden?}`（分区藏起内容时只显示名字和轮廓；图层不画也不被点到，图层面板把它们变暗）、`object.resizeToFit`。标成 `devStatus: readyForDev` 的画框或分区，里面的图层被编辑、添加或删除时会变成「已更改」（`get_node` 里 `devChanged: true`）。`dev.resolveChange {ids?, reason?}` 把它回到待开发，并留下原因（`devNote`）。分区里的画框是顶层画框（流程可以从那里开始；`proto.flow.add`）。「前往」的目的地是分区时，演示里显示这个分区上次显示的画框，否则显示它的第一个。

`transform.nudge {dx, dy}`（方向键）移动自动布局的子图层时是挪一个位置，而不是微移：沿着流，到换行布局的上一行/下一行，到相邻的网格格。在流的尽头，或跨过一行的轴时，如果周围的画框是那样流的，就进到相邻的嵌套自动布局画框（一行行的列表：向下把一项移进下面那一行）。

### 键盘快捷键

默认快捷键在一张表里（`figcraft_engine::keymap`），分组和公开的快捷键参考一样（基本、工具、视图、缩放、文本、形状、选择、编辑、变换、排列、组件、原型），带 macOS 键，不同的地方带 Windows/Linux 键（例如用作蒙版在 macOS 是 ⌃⌘M，其他地方是 Ctrl+Alt+M；置于顶层是 ⌥⌘] 和 Ctrl+Shift+]）。`list_commands` 报告每条命令在服务所在平台上的第一个键。`key` 按画布的方式解析键。键后面的命令：

- `object.opacityKeys {digits, ids?}`：不透明度数字键。一位数是百分之几十（`"0"` 是 100%）。两位数是精确百分比（`"45"` 是 45%，`"05"` 是 5%），和在画布上快速连打两位一样。
- `view.zoomAt {x, y, out?, width?, height?}`：缩放工具（画布上按住 Z；Alt 缩小）。一个点放大（或缩小）一步，并让那个页面点留在原地。一个区域填满视图。
- `view.designPrototype`（`Shift+E`）、`view.devMode`（`Shift+D`）和 `view.leftSidebar`（`Shift+Cmd+\`）在应用里切换右侧面板的页，或藏起左侧栏（无窗口时什么都不做）。
- `doc.nextTab` / `doc.previousTab`（`Ctrl+Tab` / `Ctrl+Shift+Tab`）：下一个或上一个打开的文件。
- 白板工具（仅白板）：`tool.sticky`（`Q`）、`tool.shape`（`B`）、`tool.connector`（`X`）、`tool.marker`（`M`）、`tool.highlighter`（`U`）、`tool.eraser`（`E`）、`tool.stamp`（`W`），以及 `board.nextSticky`（`Shift+Cmd/Ctrl+Enter`，在便利贴里打字时也行）。这些键没有设计文件的快捷键占用。在设计文件上它们会拒绝。
- `view.pan {dx, dy, big?}`：没选中东西时的方向键（`key ArrowRight` 做同样的事）每次把视图平移 20 个屏幕像素，`big`（Shift）是 200。
- `view.multiplayerCursors {on?}`（`Alt+Cmd+\`）：在实时会话里显示或隐藏别人的光标。
- `view.outlinesHiddenLayers {on?}` 和 `view.outlinesObjectBounds {on?}`：轮廓模式（`view.outlines`、`view.outlinesSelection`）除了形状还画什么：隐藏图层，以及轮廓不是盒子的每个图层的盒子（编组、文本、椭圆、矢量）。轮廓模式里画出来的东西可以用 `pointer` 点到。这是文件的视图设置，没有撤销步。
- `view.panelWidths {left?, right?}`：应用侧栏的宽度，单位像素（保持在它们的限制内；两个都不给则都回到默认）。无窗口时什么都不做。
- `component.hideWhenPublishing {ids?, on?}`：让组件或组件集（这些 `ids`，或给出的图层，它们自己是或在里面；默认选择）不进入文件发布的库，或让它们回来（`on: false`；不给 `on` 则切换）。标志在名字上：隐藏时在前面加 `.`，显示时去掉开头的 `.` 和 `_`。返回 `{ids, hidden}`。一个撤销步。
- 新文件和页：`file.new` 把页面颜色设为 `#F5F5F5`。只有显示深色主题的应用会把它设为 `#1E1E1E`。无窗口会话（命令行、这个服务）没有主题。`page.add` 给新页当前页的颜色，在哪里运行都一样。用 `text.create` 或 `node.create` 做的文本在这里总是黑的。在应用里（控制通道、`--attach`），深色页面上是白的，除非 `props` 点了 `fill`，和到处的文本工具（`pointer` 配 `tool.text`）一样。
- `transform.rotationOrigin {on?}`（`Alt+R`）：显示或隐藏选择的旋转原点手柄（不给 `on` 则切换）。`transform.setRotationOrigin {x, y}` 移动原点（页面坐标）。`{reset: true}` 放回中心。两者都返回 `{editing, moved, x, y}`。角上的旋转手柄、通过 `node.set` 对选择设置的 `rotation`、旋转 90°/180°，以及不带 `cx`/`cy` 的 `transform.rotate`，都绕它转。它属于这次选择，跟着图层移动和转动，选择一变就丢掉。它不保存。
- `tool.place {tool?, x?, y?}`：选了形状或画框工具时 Enter 做的事（`key Enter` 一样）：做出用这个工具点一下会做出的东西（100×100；线长 100；分区 400×300），中心在 `x`、`y`（默认视图中间），选中它，返回 `{ids}`。一个撤销步。
- `text.listIndent` / `text.listOutdent`（编辑文本时 `Cmd+]` / `Cmd+[`）：选中的列表项深一层或退出一层，和 Tab、Shift+Tab 一样。
- `view.nextRegion` / `view.previousRegion`（`F6` / `Shift+F6`，编辑文本时也有效）：键盘在应用的区域之间移动（工具栏、图层、画布、属性）。在应用里，然后 Tab 和 Shift+Tab 在该区域的控件之间移动，Enter 或空格激活一个，方向键沿工具栏走、在图层行之间走，Escape 回到画布（无窗口时什么都不做）。
- `object.rename {ids?}` 不带 `name`（`Cmd+R` 或 `F2`）：在应用的图层面板里就地重命名，图层行有键盘焦点时也可以。重命名结束时焦点回到那一行（那里按 Escape 取消）。多个图层时打开重命名图层对话框。无窗口时什么都不改，也不留撤销步。
- `object.batchRename {ids?, find?, regex?, replace?, name?, start?}`：一步重命名多个图层。`find`（纯文本，或 `regex: true` 时的正则）的每个匹配被 `replace` 替换。`name` 替换整个名字。两者都接受代码：`$&` 当前名字（匹配时是匹配本身），`` $` `` 和 `$'` 是匹配前后的文本，`$1`…`$9` 是分组，`$n` 是从 `start`（默认 1）往上数的数字，`$N` 是往下数到它的数字，`$nnn` 补到三位，`$$` 是一个美元符号。图层按叠放顺序编号，底部先。返回 `{names}`。
- 页面：`page.add {name?}`（以 `-` 开头的名字加一条分隔线：空页，在页面列表里显示成一条线，从不打开，也不装图层）、`page.list`（每页的 `divider`）、`page.rename`、`page.move {id, index}`、`page.duplicate`（「Copy of …」，在源后面，带它的颜色、流程（从复制的画框开始）、原型设置和参考线）、`page.delete`（最后一个不是分隔线的页留下）。
- `component.list`：每个组件的 `folder`（资源面板的文件夹：它所在的页、装着它的画框，然后是名字里斜杠分成的部分）和 `label`（磁贴上的名字）。
- `view.contextMenu`（`Shift+F10`）：选择的上下文菜单。图层面板有键盘焦点时（F6 之后或点过一行）在它的图层行上，否则在画布上、选择的下面。菜单拿走键盘焦点，关掉时还回去（无窗口时什么都不做）。
- `select.area {x, y, width, height, add?, deep?}`：选出从 `x, y` 框住那个盒子（页面单位）会选出的东西：顶层图层，画框只有被盒子盖住才算，盒子起始所在的顶层画框的子图层。`add` 像按住 Shift 框选那样切换，`deep` 像 Cmd/Ctrl 那样取嵌套图层。返回 `{selection}`。没有盒子时（应用里 `F8`）在选择周围（或视图中间）为键盘画一个：方向键移动它，Shift+方向键移动远角（一次 10 个屏幕像素），Enter 选择（Shift+Enter 添加，Cmd/Ctrl+Enter 取嵌套图层），Escape 或点击丢掉它。
- `pref.reduceMotion {on?}`：减少动态（关掉界面自己的动画；系统的无障碍设置也会打开它）。原型过渡仍然播放。
- `pref.uiScale {scale?}`：整个界面画多大，相对系统的显示缩放。1（默认）在每块屏幕上是系统自己的大小，1.25 表示 125%，不是 125。范围 0.75 到 3（超出停在最近的一端；`"auto"` 是 1）。省略 `scale` 则读取。画布缩放是另一回事。窗口对超过 100% 的尺寸太小时，先画小一点，直到窗口更大。选择会保留。在浏览器里界面始终跟随浏览器的缩放。
- `pref.autoSave {on?}`：已有路径的文件在更改几秒后自己保存（默认关。桌面版的计时器做这件事，所以脚本仍要调用 `file.save`）。
- `pref.startOnHome {on?}`：桌面版启动时停在主屏幕，而不是新文件（默认关。命令行点名的文件仍会打开。`--control` 无论如何从新文件开始，这样控制通道的调用者有一份文档）。
- `pref.language {language: system|en|bs|cs|ru|es|es-419|fr|de|pt-BR|ja|ka|ko|sv|vi|zh-Hans|…}`：应用的界面语言，立刻生效（`"system"`：操作系统或浏览器的语言；没有目录的语言显示英文）。命令 id、标签和每个结果仍是英文。

### 编辑矢量

`vector.create {path | points}` 用几何做出图层的盒子。`width`/`height` 把几何缩放到要求的尺寸（两个都给时各边独立；只给一个时另一边保持比例）。什么都画不出来的路径或点集会被拒绝，而不是做出看不见的 0×0 图层。几何在某根轴上没有尺寸时，给那根轴一个尺寸也会拒绝（0 除外，例如水平线的高度）。它有尺寸的轴上给 0 或更小也会拒绝，那会把它压扁。

`node.set {path}`（或 `set_properties`）用新的 SVG 路径数据替换矢量的几何，并保留图层：它的 id、名字、在图层列表里的位置、颜料、效果、样式、变量绑定和约束。路径按 `vector.create` 的方式读，同一次调用里的 `width`/`height` 以同样方式缩放。路径自己的盒子放到图层当前的 x/y（旋转和翻转保留）。同一次调用里有 `x`/`y` 时，放到 `x` 加它的左沿、`y` 加它的上沿，和 `vector.create` 放置的方式一样。点自己的设置（每点圆角、端帽、连接）跟着旧几何走。多个图层时，不是矢量的图层跳过并警告。只有一个图层、或没有一个是矢量时，调用被拒绝（先用 `vector.edit.begin` 或压平把形状转成矢量）。实例里面的图层也被拒绝：它的几何来自主组件，所以去改主组件，或分离实例。撤销把旧路径带回来。

矢量编辑模式用 `run_command` 驱动（手势用 `pointer`/`key`）：`vector.edit.begin {id? | ids?}`（默认选择；把形状转成矢量；多个图层同时编辑，第一个是活动的；选中形状时 `key` Enter 做同样的事）、`vector.edit.state`（活动图层的页面空间叠层、它的局部网络、每个点的手柄镜像、宽度点、`layers`（每个正在编辑的图层，活动的在前）、`others`（其他图层选中的点和段）和 `handles`（选中的贝塞尔手柄，写成 `[segment, start]`））、`vector.edit.tool {tool: move|pen|bend|paint|lasso|cut|eraser|width}`、`vector.edit.select {vertices?, segments?, handles?: [[segment, start], ...], id?（一个正在编辑的图层，变成活动的）, add?}`、`vector.edit.selectAll`（每个正在编辑的图层的点）、`vector.edit.moveVertices`、`vector.edit.nudge`、`vector.edit.position {x?, y?}`、`vector.edit.setHandle`、`vector.edit.mirroring {mode: none|angle|angleAndLength}`、`vector.edit.smooth {smooth?}`、`vector.edit.bend`、`vector.edit.addVertex`、`vector.edit.connect {a, b}`、`vector.edit.pen {x, y, dx?, dy?}`（从钢笔打开的那一端继续；点一个点就连接或分叉）、`vector.edit.penEnd`、`vector.edit.paint {x, y} | {points}`（油漆桶：填充或取消填充页面点下面的闭合区域；有 `points` 时，其余点下面的区域跟第一个走，像拖一下）、`vector.edit.cut {segment, t} | {vertex} | {from: [x, y], to: [x, y]}`（最后一种切开页面线段穿过的每条路径；分开的部分移到它们自己的、在上面的图层，列在 `layers` 里）、`vector.edit.erase {points, radius?}`（橡皮擦）、`vector.edit.lasso {points}`、`vector.edit.delete`、`vector.deleteHeal`、`vector.join` / `vector.smoothJoin`（编辑时连接选中的点）、`vector.edit.cornerRadius`（把选中的尖角变圆，旁边是直段或曲线：曲线在弧与它相遇的地方被切开，其余形状保留；会画出来并导出。`node.set {cornerRadius}` 对矢量把每个点都变圆）、`vector.edit.strokeCap {cap, vertices?}` 和 `vector.edit.strokeJoin {join, vertices?}`（描边在这些点结束处的端帽，以及在那里转弯处的连接，`null` 用图层的；会画、会导出、会描轮廓）、`vector.edit.moveHandles {dx, dy, handles?}`（按页面偏移移动选中的手柄）、`vector.edit.end`。点的索引指 `vector.edit.state` 里 `network.vertices`。局部坐标是图层自己的。

一次多个图层：编辑模式里不止一个图层时，按另一个图层的点、手柄或段会让它变成活动图层。Shift 点击以及框选或套索可以在其中任何一个里选点。拖一个选中的点、方向键、Delete、`vector.edit.nudge`、`vector.edit.moveVertices`、`vector.edit.delete`、`vector.deleteHeal`、`vector.edit.mirroring`、`vector.edit.smooth`、`vector.edit.cornerRadius`、`vector.edit.strokeCap` 和 `vector.edit.strokeJoin` 在一个撤销步里碰到每个图层选中的点（`vertices` 列表只点名活动图层的点）。钢笔、弯曲、油漆桶、剪切、橡皮擦、连接、定位和变换盒在活动图层里工作。不止一个图层有选中的点时，盒子隐藏。Shift 点击贝塞尔手柄来选中多个；拖一个就把它们全拖走。

编辑模式之外的钢笔：点路径的第一个点会闭合它。点它自己的另一个点（不是最后一个）会连到那里并结束路径。选中一条打开的矢量时，点它的一个端点会在编辑模式里继续那条路径。`vector.deleteHeal` 让愈合后的曲线穿过被删掉的点所在的位置，并保留邻居的切线。用 `pointer` 时，油漆桶和剪切也可以拖（跨区域来上色，跨路径来切开），各是一个撤销步。在点盒子的一个角上，对选中的点按 `shift`，点击会把它移出选择，拖拽会按比例缩放。

选中点的变换盒：`vector.edit.bounds`（`{x, y, width, height, corners, points}`：盒子在矢量的轴上，`x`/`y` 用父坐标，和位置字段一样）、`vector.edit.resize {width?, height?, center?}`（除非 `center`，否则保持左上）、`vector.edit.rotate {angle, cx?, cy?}`（绕盒子中心或一个页面支点逆时针的度数）、`vector.edit.transform {matrix: [a, b, c, d, e, f], vertices?}`（任意页面空间仿射）。有点被选中时，`object.flipH` / `object.flipV` 和 90°/180° 旋转作用于这些点。在画布上（`pointer`），拖点、手柄和钢笔点击会吸附到其他点和段的中点（这条矢量的和其他矢量的）、通常的图层目标以及像素网格。拖的时候按住 `cmd` 关掉对象吸附。

虚线端帽：`node.set {dashCap}` 对虚线描边每一段的两端取 `none`、`round` 或 `square`。`null` 清除它（虚线然后用 `startCap` 和 `endCap`，这是默认）。

变宽描边：`stroke.widthProfile {ids?, preset?: uniform|wedge|taper|eye|rounded|pinch, points?: [[t, scale], ...]}`（也可以 `node.set {widthProfile}`。`t` 沿每条子路径按长度从 0 到 1，`scale` 乘描边粗细，中间的宽度平滑插值。没有预设或点就是均匀）。编辑时，变宽工具（`width`，⇧W）拖宽度点：`vector.edit.widthPoint {t, width?（像素）| scale?}` 加一个，`{index, t?, width? | scale?}` 移动或改变它的大小，`{action: "remove", index}` 删掉它。渲染、PNG/SVG/PDF 导出和 `vector.outlineStroke` 都跟着这个轮廓。

单侧宽度：宽度点可以在路径的一侧比另一侧更宽（沿路径看，y 向下，分左和右）。点是 `[t, left, right]`（或 `{t, left, right}`），每一侧的比例是两侧都那么宽时 `scale` 会是的值，所以 `[t, s]` 等于 `[t, s, s]`。`get_node` 这样报告单侧的点。`vector.edit.widthPoint {index, side: left|right, width? | scale?}` 改一侧（`width`：从中心线到那条边的距离；带 `t` 时加一个点，另一侧保持原样）。不带 `side` 的宽度让点重新对称。用 `pointer` 时，拖手柄（或路径）时 `alt: true` 只改那一侧。`vector.edit.state` 列出 `widthPoints`，带两个手柄位置（`sides`：右、左）和 `oneSided`。

### 原型

交互是 `proto.interaction.add` / `set` / `remove` / `clear`。前往动作可以用 `preserveScroll: true` 保留滚动位置。`{type: "scrollTo", dest, offset?: [x, y]}` 让滚动它的最内层画框停在离左上角那么远的地方，例如固定页眉下面。`delayMs` 让鼠标进入、离开、按下或抬起在动作之前等待。`{type: "openOverlay", dest, position: "manual", offset?: [x, y]}` 把叠层放在打开它的图层左上角那么远的地方；没有偏移就紧贴在那个图层下面。`anchor: "triggerInside" | "triggerOutside"` 把其他位置量在打开叠层的图层上，而不是屏幕上，所以 `{position: "bottomLeft", anchor: "triggerOutside"}` 把下拉菜单开在按钮正下方，左沿对齐。Figma 没有这个设置，`.fig` 导出把它写成手动偏移。按键触发接受一个键或组合（`{type: "keyPress", key: "Shift+K"}`；Ctrl、Alt、Shift 和 Cmd，全都按住且没有别的），或手柄按钮（`key: "Gamepad A"`；`Gamepad` 后面跟 `A`、`B`、`X`、`Y`、`LB`、`RB`、`LT`、`RT`、`View`、`Menu`、`LS`、`RS`、`Up`、`Down`、`Left`、`Right` 或 `Home`：标准布局，面键按位置）。播放器在 Windows 和浏览器里从手柄读取，控制通道用 `ui.input {events: [{gamepad: "A"}]}` 按下。小组件在播放器里对点击作出反应，用的是它自己的那份文件副本（见 [plugins.md](../plugins.md)）。在播放器里，带移动、推入或滑入动画的「拖动时」触发只在往那个方向拖时开始，动画跟着指针直到松开。弹簧缓动一直播放到稳定（时长由弹簧决定，不是 `durationMs`）。叠层的延迟之后计时器在它打开时开始。组件的实例播放它们正在显示的变体上的交互，除非它们自己对那个触发有交互。固定图层画在在它们下面滚动的内容之上。变量动作：`{type: "setVariable", variable, value}` 设一个值，或用 `expr` 在运行时计算表达式。`{type: "setVariableMode", collection, mode}` 为原型切换一个集合的模式（id 或名字；屏幕里面图层上选的模式仍然优先）。`{type: "conditional", blocks: [{condition?, actions}]}` 运行第一个条件成立的块（没有条件的块是「否则」）。表达式是 `{value: {type: number|string|boolean|color|alias, value}}`（`alias` 读一个变量）或 `{op: {op, args}}`，运算有 `add`（涉及字符串时拼接文本）、`subtract`、`multiply`、`divide`、`equals`、`notEqual`、`less`、`lessOrEqual`、`greater`、`greaterOrEqual`、`and`、`or`、`not` 和 `negate`。`{inMode: {variable, mode}}` 读一个变量在它集合的某一个模式里的值，不管当前选的是哪个模式。`{mode: {collection}}` 给出交互运行处一个集合所处模式的名字（文本；它跟随设置变量模式的动作）。变量、集合和模式可以按名字给，存储时用 id。`proto.evaluate {expression | expr, id?}` 计算表达式而不改变任何东西，变量用图层 `id` 的交互会运行的那些模式（默认：默认模式）→ `{value, type, text}`。错了就失败并说明错在哪，和交互命令一样。表达式也可以写成文本：在设置变量动作上写 `expression: "count + 1"`，或在条件块上写 `expression: "count >= 3 and not done"`，代替 `expr` / `condition`（变量按名字，任何名字用 `{name}`；数字、`"text"`、`true` / `false`、`#RRGGBB`；`or` / `||`、`and` / `&&`、`not` / `!`、`==`（或 `=`）、`!=`、`<`、`<=`、`>`、`>=`、`+`（拼接文本）、`-`、`*`、`/` 和括号；`accent[Dark]` 是变量在一个模式里的值，`mode(Theme) == "Dark"` 是集合的当前模式，名字两端有括号或空格时加引号）。文本表达式会检查：条件必须给出真或假，设置变量的表达式必须给出变量能装的值，否则命令失败并说明错在哪（`Something is missing at the end`、`No variable \`x\``、`Can't compare a number with text`）。单个动作，包括条件里面的，用 `proto.action.add {id?, index, path?, action}`（→ `{path}`）、`proto.action.set {index, path, action}`、`proto.action.remove {index, path}` 和 `proto.action.move {index, path, to}` 编辑。`index` 是交互，`path` 是动作：`[i]` 是交互的动作 `i`，`[i, b, j]` 是条件动作 `i` 的块 `b` 的动作 `j`，以此类推（对 `add`，这是它去的地方：`[i, b, n]` 追加到有 `n` 个动作的块；默认：最后一个动作之后）。条件的块：`proto.conditional.addBlock {index, path, else?, expression? | condition?}` 加一个否则如果（在否则之前；默认是第一个布尔变量等于它的值），或 `else: true` 时加否则。`proto.conditional.setCondition {index, path, block, expression | condition}` 和 `proto.conditional.removeBlock {index, path, block}`（只剩否则时去掉如果，会去掉整个条件）。每一条都是一个撤销步，并应用到给出的全部 `ids`（有同样交互的图层）。流程是 `proto.flow.add` / `rename` / `remove`。设备和它的旋转、背景是 `proto.settings {device?, background?, rotation?, color?, width?, height?, responsive?}`（`None`、`iPhone 16`、`iPhone 16 Pro Max`、`Android Compact`、`iPad mini`、`MacBook Air`、`Desktop` 或 `Custom size`。`rotation` 是 `portrait` 或 `landscape`，转动手机和平板。`color` 是设备颜色的名字，例如 iPhone 16 的 `Black`、`White`、`Pink`、`Teal`、`Blue`，错误会把它们列出来，`""` 是默认。`width` / `height` 设置自定义尺寸的屏幕，1–10000 点，并选中 `Custom size`，那是没有机身的光屏幕）。在设备上，演示的画框被定成设备屏幕的大小：比屏幕大的画框滚动它的整个尺寸，不管它的 `overflow`；超出画框自己边缘的内容只按它的 `overflow` 允许的方式滚动（否则裁掉）。`fixed` 图层留在屏幕上（钉在它们的约束所说的边上）。`responsive: true`（存在文件里）时，演示的画框改为改变大小到设备屏幕，没有设备时到演示窗口的 100%，它的约束和自动布局像在编辑器里改变大小时那样响应（它适应的轴继续适应内容）。文件里的画框不变。`proto.present {start?}` 全窗口播放原型，`proto.preview {start?}` 在画布上一个可改变大小的窗格里播放，并实时跟随编辑。`proto.closePreview` 关掉它。`proto.presentOptions {scale?, deviceFrame?, hotspotHints?, inspect?, hideUi?}` 设置全窗口演示怎么给原型定尺寸（`fit` 缩小到能放下，`fitWidth` 填满宽度，`fill` 盖住窗口，`actual` 按 100% 显示；溢出的部分可以平移）、是否画设备、在没有反应的地方点击时是否短暂闪一下屏幕的热点（有交互的图层，默认开；也在演示的选项菜单里）、检查面板是否显示指针下图层的尺寸、颜色和排版（也是它那一条上的检查按钮）。选项会留给以后的演示（演示里 Z 循环它们，R 重新开始）。打开指向 `http` 或 `https` 地址的链接时，先显示一条提示，说这个链接会离开原型，有打开链接、取消和「不再显示」（`openInNewTab: false` 的 `openUrl` 动作然后在网页版自己的标签里打开，离开原型；桌面版不管怎样都打开浏览器）。`proto.linkNotice {show?}` 打开（默认）或关掉这条提示，和桌面版的偏好一起保存（网页版保留到页面重新加载）。接上时，播放器的状态在 `inspect_ui` 里（`presenter`、`preview`：屏幕、滚动、`scale`、`zoom`、`pan`、屏幕的 `hotspots` 以及它们是否在闪、`inspected` 图层）。`ui.set {presentScroll: [x, y]}` 滚动它的屏幕（见 [control-protocol.md](../control-protocol.md)）。演示的选项菜单在尺寸选项旁边提供响应式（没有设备时）或响应式 / 固定尺寸（有设备时），就是同一个 `proto.settings {responsive}`。它的流程选择器列出每个流程和描述。

### 本地库

一份 `.figcraft` 文件可以发布成库，给其他文件用（这里说的是库文件；团队放在 `figcraft-server` 上的库见[团队库](#团队库)。浏览器版没有文件系统，所以那里读库文件的命令返回错误）。

- **发布**（在库文件里）：`library.publish {description?, include?, exclude?}` 把文件的组件、组件集、样式和变量集合发布成一个新版本（默认是除了以 `.` 或 `_` 开头的名字之外的一切，或上次发布提供过的那些。`include`/`exclude` 接受组件 id，以及样式或集合的 id 或名字）。集合带着它的变量（每个模式的值、别名、作用范围），隐藏不发布的除外（`variable.update {variable, hiddenFromPublishing: true}`）。已发布资源用到的组件、样式和变量（组件绑定的变量、变量的别名目标）会跟着来，即使没被点名提供。每个资源保留它上次改变时所在的版本。没有任何更改就发布会被拒绝。保存文件（`file.save`），其他文件才能看到新版本。
- **启用**：`library.add {path}` / `library.remove {id}`（id、名字或路径）/ `library.list {assets?}`（版本、待更新、错误；`assets: true` 时每个库的组件、样式、变量集合（带它们的模式）和变量，以及这份文件能发布什么）。库是只读读取的，有大小限制，文件改变时再读。
- **使用**：`library.place {library, component, x?, y?}`（按 id 或名字的组件、组件集或变体；页面点，默认视图中心）把组件连同它用的组件和样式复制进文件，作为只读的主组件，并放一个实例。`library.import {library, component}` 只复制它（然后 `instance.create`、`instance.swap`…）。`library.swapAt {library, component, x, y}` 复制它，并把那个页面点上的实例换成它（和下面的 `instance.swapAt` 一样）。`library.applyStyle {library, style, ids?, target?}` 把样式复制进来（已链接、只读）并应用。库组件的实例和任何实例一样：覆盖、属性、变体、替换、重置、分离。库组件保留它们的变量绑定和显式模式（绑到库变量的副本上）。
- **库变量**：`library.bindVariable {library, variable, field, ids?}` 把已发布的变量复制进来（已链接、只读，带着它别名的变量和它的集合）并像 `variable.bind` 那样绑定。`library.aliasVariable {library, alias, variable, mode?}` 让本地变量的值成为库变量的别名。`library.setVariableMode {library, collection, mode, ids?}` 在画框上设置库集合的模式（不给 id 时是页面；设计面板的变量模式一节在库的名字下列出已启用库的集合，并用这种方式设置）。`library.importVariable {library, variable}` 只复制它（然后 `variable.bind`、`variable.set {value: {alias}}`）。这些副本不需要库文件就能渲染。编辑副本（库组件、样式、变量或集合）会被拒绝。去库文件里改，再发布。
- **扩展集合**：`library.extendCollection {library, collection, name?}` → `{id}` 做这份文件的一个集合（默认名字「<名字> (extended)」），带库集合的模式，以及它的每个变量一个继承变量。对继承变量做 `variable.set` 会在那个模式里覆盖它的值（变量在 `overrides` 里列出）。`variable.resetOverride {variable, mode?}` 回到继承的值（不给 `mode` 时是每个模式）。它自己的变量来自 `variable.create`。模式，以及继承变量的名字，来自库集合（改名、添加或删除它们会被拒绝）。接受库更新会带来新值（覆盖留下）、改名和新变量。库加到被扩展集合上的变量是它自己的一次更新（`library.updates` 列出它时没有 `id`，它还没有副本）。本身就是扩展集合的库集合可以再扩展：每份文件接受它的库的更新时，这条链跟着走。这份文件的集合也可以扩展：`variable.extendCollection {collection, name?}` → `{id}` 对这份文件的集合做同样的事（包括已经扩展的），扩展立刻跟随它的编辑（值、新变量和模式、改名）。变量编辑器在集合的行上提供扩展集合（这份文件的或库的），并标出被覆盖的值，带一个重置按钮。
- **更新**：`library.updates` 列出有更新的已发布版本的组件、样式、变量集合和变量。`library.acceptUpdates {ids?（库 id）}` 把它们拉进来，保留实例覆盖。绑定的图层、别名和显式模式跟随更新后的值。
- **文件之间粘贴**：从另一份打开的文件复制的实例（在那里 `edit.copy`，在这里 `edit.paste`）跟随它们的主组件。库组件加入（或带来）这份文件对同一个库的副本。另一份文件作为库发布的组件从它那里使用（另一份文件已保存时，那个库会被启用）。其他主组件留在后面：实例显示缺失的主组件（`component.restoreMain` 在这里把它变成主组件）。

### 团队库

放在 `figcraft-server` 上的文件（[`server.md`](../server.md#team-libraries)）可以装着一个库，其他文件通过它的一个分享链接来用。每次调用等待服务器（连不上时最多几秒）。

- `library.publishToServer {server?, key?, description?, include?, exclude?}` 把当前文档的库发布成服务器在这份文件下保存的那一个的下一个版本（选择和每个资源的版本同 `library.publish`，跟随服务器的版本）。`server` 是文件的编辑链接，或在文档正处于有编辑权限的服务器实时会话里时省略它。用服务器的地址和密钥（`ws://host:47900/?key=…`，或 `key`）时，文档先作为新文件保存在那里，结果带它的 `file`、`edit` 和 `view` 链接。查看链接会被拒绝。文档本身不变（没有可撤销的）。
- `library.add {path: <分享链接>}` 启用它（查看链接就够）。然后 `library.place`、`library.applyStyle`、`library.bindVariable`… 和库文件一样工作。`library.list` 和 `library.updates` 先向服务器要更新的版本（列表里 `"server": true`）。`library.acceptUpdates` 取它，覆盖保留。应用自己的后台检查只问这台电脑上添加过（`library.add`）或检查过（`library.check {library?}`）的团队库，所以打开一份点了另一个库名字的文件不会联系任何人（#1097）。
- `library.usage {library?, days?}` → `{id, name, assetsKnown, styles [{id, name, type, layers, files}], variables [{id, name, collection, layers, files}], instances, detached, detachesKnown, days, insertions, detachments, actionsKnown, weeks: [{week, insertions, detachments}], files: [{name, updated, instances, detached}], components: [{id, name, instances, files, detached, insertions, detachments}]}`：使用它的文件报告了什么（每次获取报告询问文档的名字，以及每个组件的实例；然后每个组件被分离和插入的实例。`detached` 把每次分离加起来，`detachesKnown` 在不保存它们的服务器上是 false）。`insertions` 和 `detachments` 数与最近 `days`（1–365，默认 30）重叠的那些周（`week` 是那周的星期一，1970 UTC 以来的秒，只有那周里发生过事才列出）。一次插入是放置、粘贴或复制的实例，不是嵌在另一个实例里的。`actionsKnown` 在不保存周的服务器上是 false。`library` 是已启用的团队库。省略它则是当前文档在它的服务器上自己的库。
- `library.swap {from, to}`（任意两个已启用的库，文件或团队）把从 `from` 复制来的每个实例、样式和变量改指向 `to` 里同名的已发布资源（变体按组件集里的名字，样式按名字和类型，变量按集合、名字和类型，模式按名字），保留覆盖。一个撤销步 → `{components, styles, variables, unmatched: [仍来自 from 的名字，包括这份文件里扩展了它的某个集合的集合], removed（from 什么都没留下，所以它被移除了）}`。

### 覆盖和变体

- **实例覆盖什么**（在它的图层上 `node.set` / `text.set`；`get_node` 列出 `overrides`）：填充、描边、效果、不透明度、可见性、图层名、圆角、文本内容和文本属性（字体、字号、间距、尺寸调整、列表…）、自动布局的内边距、间距和对齐、布局网格、导出设置和原型交互。嵌套实例可以替换（`component`）并接受属性值（`props`）。主组件改那个属性时，每一项都留下。用 `component.resetOverride {id, group}` 或 `component.resetInstance` 去掉。
- **它不覆盖什么**：移动、旋转、改变大小或重新约束实例里面的图层，它的尺寸调整（适应 / 填充 / 最小 / 最大），自动布局方向和其他自动布局设置，以及添加、删除或重排图层（插槽以外）都会被拒绝（什么都不改）。改变实例自己的大小（它的图层跟随主组件的约束），或去改主组件。
- **变体**：`instance.setProperties {ids?, properties}` 切换变体时，在图层的名字路径匹配、且两个变体对那个属性有相同值的地方保留覆盖。文本图层保留它们全部的覆盖。默认变体（用组件集做 `instance.create`，资源缩略图）是组件集左上角的那一个。`component.editProperty {name, default}` 把选中的变体移到那里。`{name, index}` 重排变体属性（变体的名字跟着）。对组件集做 `component.properties` 会报告 `conflicts`：共用同一组值的变体。
- **布尔属性**：在主组件上，布尔属性的值是绑到它的图层的可见性。`component.addProperty {type: "boolean", default, layer}`、`component.bindProperty {field: "visible"}` 和 `component.editProperty {name, default}` 显示或隐藏主组件上绑定的图层，以匹配默认值（没有 `default` 时，`addProperty` 取绑定图层的可见性）。在主组件上显示或隐藏绑定的图层会设置默认值，这个属性的其他绑定图层（组件集的每个变体里）跟着。新实例从默认值开始。已有实例保留它们已有的值。文件按保存的样子打开：绑定图层和默认值不一致的主组件保持那样，直到其中之一被编辑。
- `component.pushChanges {id?}` 把实例的覆盖复制进它的主组件（从嵌套实例的图层也行：总是进外层实例的主组件）。

### 替换实例

- `instance.swap {ids?, component}` 在实例里显示另一个组件（名字匹配的图层上的覆盖留下；同名的属性值也留下）。嵌套实例可以单独替换（它们所在实例的一次覆盖）。被替换的嵌套实例把外层主组件在匹配图层上覆盖过的东西留成它自己的，所以以后主组件里那些地方的编辑到不了它。
- `instance.swapAt {component, x, y}` 替换页面点上的实例：在它下面的实例里（最内层优先），第一个显示同一组件集的变体的，否则是最内层（从不选被放下的组件里面的）。只有文本图层保留覆盖。资源面板在按住 Alt（Option）把组件放到实例上时做这件事，拖的时候高亮那个实例。
- 实例替换属性提供**首选值**：`component.addProperty {type: "instanceSwap", default, preferred? [组件或组件集 id]}` 和 `component.editProperty {name, preferred}`。`component.properties` 列出它们，设计面板的替换菜单把它们放在前面。

### 插槽

插槽是主组件里面的一个画框，它的实例可以在那里放自己的任何内容（在那里添加、删除和重排图层，和实例的其余部分不同）。它是类型为 `slot` 的组件属性（`component.properties` 列出它：在主组件上带它的 `layers`，在实例上是 `value: {layer, edited, layers}`）。

- **定义**（在主组件上）：`component.makeSlot {id（主组件里面的一个画框）, name?, preferred? [组件或组件集 id]}`（属性默认用画框的名字。新属性放在最后，`component.properties` 按创建顺序排列，改名保留位置）。`component.editProperty {name, preferred}`。`component.removeSlot {id}`（实例失去它们在里面的自己的内容）。`component.addProperty {type: "slot", layer}` 和 `component.bindProperty {field: "slot"}` 也行（一个插槽每个组件一个画框）。
- **填充**（在实例上）：`instance.slotInsert {id（插槽图层，或带 slot（属性名）的实例）, component, index?, replace?}` 在里面放一个组件的实例。任何结构命令（`shape.*`、`node.reparent`、选中插槽时的 `edit.paste`、`edit.delete`、`node.reorder`、`instance.place` 或拖到它上面）在里面都工作。主组件的默认内容会显示（并跟随主组件），直到插槽第一次被编辑。然后插槽的图层是实例自己的（覆盖组 `slot`）：跨主组件更新、变体切换、复制/粘贴和分离都保留。放在那里的实例独立存在（它们自己的覆盖、属性和分离）。
- **重置**：`instance.resetSlot {id}`（或 `component.resetOverride {id, group: "slot"}`，或 `component.resetInstance`）把主组件的默认内容带回来。

`get_node` 在插槽图层上显示 `"slot": "<属性>"`。

### 布尔组

`bool.union` / `bool.subtract` / `bool.intersect` / `bool.exclude {ids?}` 把图层包进一个新的布尔组，除了：选中一个布尔组时，它们设置它的运算（每个操作数再次跟随它）；图层在布尔组里面时，它们设置那些操作数自己的运算。底部以上的每个操作数用它自己的运算，或组的运算，和它下面的操作数的结果组合（在任意操作数上 `node.set {operandOperation: union|subtract|intersect|exclude | null}`，或在本身不是布尔组的操作数上 `booleanOperation`；在组上 `booleanOperation` 是组的）。`get_node` 在有自己运算的操作数上显示 `operandOperation`。操作数仍是可编辑的图层：`select.children`（Enter）或双击选中它们，`cmd: true` 的点击直接点到它们。移动、改变大小或编辑操作数的点（`vector.edit.*`）在每次指针移动时更新组的结果，一次手势一个撤销步。拖着的操作数留在它的组里。点击和 `select.layersAt` 在布尔组的结果所在处点到它（在被减去的洞里点击会点到后面的东西）。组或它的一个操作数被选中时，改为点到它的操作数，这样每个都能被选中。`bool.flatten` 把结果烤进去。拖的时候只从被编辑的那个操作数往上重新组合（`figcraft_render::boolean`；用 `cargo xtask bench` 计时）。

### 形状生成器

选中形状的轮廓把画布分成区域（被同一组形状覆盖的面积）。`shapeBuilder.regions {ids?}` 列出它们：页面坐标里的 SVG `path`、盖住每一个的 `sources`（最上面的先）、盖住它的 `parts`（`{id, stroke}`：形状的填充，或它描边的轮廓区域；最上面的先）、`bounds` 和里面的一个 `point`。可见描边以它们的轮廓区域参加，所以有描边的线和开放路径会切开它们穿过的区域，描边的一条带是它自己的区域。`shapeBuilder.merge {ids?, regions?, points?}` 用这些区域做一个矢量（按索引，或页面点下面的区域），样式像盖在第一个上面的最上面的填充或描边（描边的区域用描边的颜料填充）。`shapeBuilder.remove {ids?, regions?, points?}` 删掉它们。无论哪种，盖住被选区域的每个形状都失去那块面积（它变成矢量，或什么都不剩时消失）。涉及的形状的描边变成紧挨在它们上面的轮廓矢量（结果里的 `outlines`；开放路径被它的轮廓替换）。每次调用是一个撤销步。工具是 `tool.shapeBuilder`（Shift+M）：用 `pointer` 点击或拖过区域来合并，加 `alt` 来去掉。`shapeBuilder.mode {mode: merge|remove}` 交换普通点击做的事。

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"shapeBuilder.regions","params":{"ids":[2,3]}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"shapeBuilder.merge","params":{"ids":[2,3],"points":[[120,80],[160,80]]}}}}
```

### 路径上的文本和图案填充

- **路径上的文本**：`text.onPath {text?, path?, start?, flip?}`（默认：选中的文本图层和另一个选中的图层）把文本图层放到矢量或形状的路径上。文本拿走路径图层的位置、变换和盒子，路径图层被去掉（`edit.undo` 把它带回来）。文本在路径图层有效果时也拿走那些效果，在其中一个填充显示时拿走它的填充，各自带着样式和变量绑定；否则文本保留自己的。基线从 `start`（它长度的 0 到 1）沿路径走。文本的水平对齐把它放在那里（左对齐从那里开始，居中在那里居中，右对齐在那里结束）。`flip` 让它在另一侧、另一个方向走。开放路径末端以外的字形被隐藏。闭合路径（椭圆、矩形）把它们绕一圈。只跟随第一条子路径。`text.pathOptions {start?, flip?, ids?}` 更改它们。`text.removeFromPath {ids?}` 让它变回普通的自动宽度文本。改变图层大小会缩放路径。文本仍可编辑（`text.edit.*`、`type_text`）。`get_node` 显示 `textPath {d（图层空间里的 SVG 路径数据）, start, flip}`。导出把它画成轮廓（PDF 也是：不可选择）。路径上不画下划线和删除线。用工具 `tool.textOnPath` 时，`pointer` 在形状或矢量的第一条子路径 6 个屏幕像素以内按下，会在上面放一个新的空文本图层，从按下的地方开始，并开始打字。文本也拿走路径图层的约束和效果，以及其中一个显示时的填充。编辑结束时什么都没打，会把路径图层放回去，没有撤销步。锁定的图层和实例里面的图层会跳过。
- **图案填充**：`fill.pattern {source（图层 id）, ids?, index?, tile?（grid|horizontalHex|verticalHex）, scale?, spacing? [x, y], align?（start|center|end）, opacity?}` 让 `index` 处的填充（默认最上面的）成为源图层的重复，大小是 `scale` 乘它自己的尺寸，瓷砖之间是 `spacing`。六边形瓷砖每隔一行（一列）错开半块。`align` 把一块瓷砖放在盒子的左上角、中心或右下角。填充是 `{type: "pattern", source, tile, scale, spacing, align}`，也可以用 `fill.set {paint}` 设置。源改变时它更新。图层不能平铺它自己，或装着它的图层（嵌套超过两层的图案什么都不画）。和源一起做的副本平铺被复制的源。导出：源的矢量的 SVG `<pattern>` 和 PDF 平铺图案（有效果的源是一块瓷砖的图片）。画布按它显示的缩放画一块瓷砖（一边最多 2048 像素）并重复它。

### 导入 SVG

`open_file {path}` 把 SVG 打开成新文档。`run_command {command: "file.import", params: {path | data + name, x?, y?, center?}}` 把它加到当前页，作为一个画框（左上角在 `x, y`，或 `center: true` 时中心在那里，和粘贴一样）。进来后可编辑：编组用它们的 `id` 起名，变换烤进图层，`<rect>`/`<circle>`/`<ellipse>`/`<line>`（以及描这些轮廓的路径）成为带圆角的矩形、椭圆和线，其余是矢量。描边带它们的宽度、虚线、端帽、连接和斜接限制。线性和径向渐变带精确的手柄（`gradientTransform`，`reflect`/`repeat` 展开，焦点作为中心）。图案成为图片填充（填充、适应、裁剪或平铺）。裁剪路径成为矢量蒙版，蒙版成为亮度或 alpha 蒙版。阴影和模糊滤镜（`feDropShadow`、偏移 / 模糊 / 洪水 / 合成链、颜色矩阵阴影、内阴影、`feMorphology` 扩展）成为投影、内阴影和图层模糊。还有不透明度和 `mix-blend-mode`。文本成为文本图层（SVG 点名且已安装的字体、锚点、字距、带样式的 `<tspan>` 段、叠成一层的多行；一个 `<textPath>` 上的文本成为路径上的文本，带它的起始偏移和锚点）。图片成为图片填充，SVG 图片就地导入。CSS、`<use>`/`<symbol>`、单位和 `viewBox`/`preserveAspectRatio` 会被解析。结果的 `warnings` 列出省略了什么（不是阴影或模糊的滤镜、旋转的重复图案、裁剪路径自己的裁剪路径）。链接的文件从不读取，只读 data URI。嵌套超过 512 个元素的文档会被拒绝。

### 导入 `.fig` 文件

`open_file {path}`（一份新的、未命名的文档；从不覆盖保存那个文件）和 `file.import`（页追加）尽力读取 `.fig` 文件。每个文件带着它的数据编码所用的模式，所以数据按通用方式解码，再按字段名映射到下面的 REST 导入：同样的图层、颜料、文本、自动布局、组件、样式、变量和原型会过来，图片填充来自文件自己的 `images/` 文件夹。实例从它们的主组件重建，带文件记录的覆盖和属性值。矢量作为可编辑的矢量网络进来（网络缺失或损坏时用它们的轮廓）。便利贴、带文字的形状、连接线、表格和类似图层作为最接近的画框、文本图层和矢量进来。设置变量、设置变量模式和条件动作会过来。视频填充跳过。结果的 `opened.warnings` 列出近似或跳过了什么。

### 导出 `.fig` 文件（实验性）

`export {format: "fig", path?}`（或 `run_command` 的 `file.export {format: "fig"}`，或任何以 `.fig` 结尾的 `path`）把整份文档写成 `.fig` 文件，依据和导入相同的公开格式笔记：图层树、几何、带图片字节的颜料、描边、效果、带样式段的文本、矢量网络、自动布局、约束、组件、组件集、带覆盖和属性值的实例、带模式的变量集合、变量和绑定、带图层链接的共享样式，以及原型交互。`warnings` 列出省略了什么（评论、开发模式数据、某些绑定和设置之类）。经 FigCraft 自己的导入器往返是测过的，示例文档和小测试文件在其他软件里打开过（`docs/file-format-parity.md` 说明检查了什么）。其他应用可能打不开每一份文件。没有 `path` 时，文件以 base64 `data` 返回。再打开一次来检查：

```text
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"export","arguments":{"format":"fig","path":"/tmp/handoff.fig"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"open_file","arguments":{"path":"/tmp/handoff.fig"}}}
```

### 导入 REST API 文件 JSON

公开 REST API 文件端点返回的 JSON（`GET /v1/files/:key`，最好带 `geometry=paths`，这样矢量带着它们的轮廓）和它的节点端点，像任何文档一样打开：`open_file {path}`（一份新的、未命名的文档；JSON 从不被覆盖保存）或 `run_command {command: "file.import", params: {path}}`（页追加到当前文档，带着它们的流程、原型设置和变量）。结果的 `warnings` 列出跳过或近似了什么（不支持的图层、颜料和效果类型、视频触发、多于一个按钮的手柄触发、除了在一个变量和它的一个模式上查变量模式之外的表达式函数、绑到库变量、文件以外组件的实例，那些作为画框进来）。页、图层树、几何、颜料、描边、效果、圆角、约束、自动布局、布局网格、导出、带样式段的文本、矢量、组件、组件集、实例和组件属性都会映射，原型也是：`interactions`（点击、悬停、按下、拖动、延迟之后、鼠标进入/离开/按下/抬起和按键触发，手柄触发作为手柄按钮；前往、叠层、返回、滚动到、切换到、打开链接、设置变量（值、变量引用和表达式）、设置变量模式和条件（表达式上的如果 / 否则如果 / 否则）动作；溶解、智能动画、移动、推入和滑入过渡，包括它们的缓动、自定义贝塞尔和弹簧曲线）、流程起点、滚动溢出和固定/粘性图层，以及原型设备。图片填充保留它们的 `imageRef`，并显示灰色占位，除非图片文件在 JSON 旁边的 `images` 文件夹里，按 `imageRef` 命名（`images/<imageRef>.png`）。不会从网络获取任何东西。

变量来自本地变量响应（`GET /v1/files/:key/variables/local`）：集合和它们的模式（默认模式在前）、每个模式的值、别名和作用范围。把它作为 `variables`（路径，或 JSON 对象或文本）传给 `open_file`、`file.open` 或 `file.import`，或放在文件旁边作为 `<名字>.variables.json` 或 `variables.json`，或把它的 `meta` 嵌进文件 JSON。图层的 `boundVariables`（尺寸、间距和内边距、圆角、不透明度、描边粗细和每边的描边粗细、可见性、文本、字体属性、单色填充和描边）和 `explicitVariableModes` 然后链到它们，所以用 `variable.setMode` 切换模式会重新给图层上样式（`mode: null` 把画框设回自动，继承父图层的模式）。

同一份 JSON 在 FigCraft 文件之间搬运变量：`variable.export {collections?, format?, path?}` 把这份文件的集合（默认：除库副本以外的全部）写成一份本地变量响应（`format: "json"`，默认；颜色计算写成它们在每个模式里显示的颜色，其他集合用它们的默认模式），或写成 CSS 自定义属性（`format: "css"`：默认模式在 `:root` 上，其他每个模式在 `[data-<集合>="<模式>"]` 上，别名是 `var(--name)`，颜色计算是它们在那个模式里显示的颜色（其他集合用默认模式），数字是 px，除非绑到它们的图层、它们的作用范围或别名它们的变量使它们成为不透明度或字重，所以 `border-radius: var(--radius)` 读出来是 `12px`），或写成 Flutter 的 Dart 常量（`format: "dart"`：每个集合和模式一个 `abstract final class`，默认模式以集合命名，其他的以集合和模式命名，每个变量一个 `static const` 的 `Color`、`double`、`String` 或 `bool`；名字变成彼此分开的驼峰 Dart 标识符，别名解析成它们在那个模式里的值），并返回 `{text}`（或写出 `path` 并返回 `{path}`）。`variable.import {variables}`（对象、JSON 文本，或最多 256 MiB 的普通文件的路径；`variableCollections` 和 `variables` 在 `meta` 下或在顶层）读回一份：集合加入这份文件里同名的集合（值按模式名匹配，新模式和变量被添加，同名但另一种类型的变量跳过并警告；库副本和扩展集合从不是目标），其他的被添加。别名会检查循环。它返回 `{collections, variables, updated, warnings}`，JSON 里没有变量时警告（`no variables found: expected meta.variables …`），或比它的集合在 `variableIds` 里列出的更少时警告。变量编辑器的导入…和导出…按钮通过文件对话框做同样的事。

描边像填充一样接受颜色样式和变量：`style.apply {style, target: "stroke"}` 把颜色样式链到描边（`styles.stroke`；样式的编辑跟着走，`style.detach {which: "stroke"}` 保留颜色）。`variable.bind` 单独绑定 `strokeWeight` 和每一边的粗细（`strokeTopWeight`、`strokeRightWeight`、`strokeBottomWeight`、`strokeLeftWeight`；用 `node.set` 绑定或设置一边会打开分别描边，其他边用那个粗细）到作用范围是描边粗细的数字变量。

`variable.bind` 也绑定渐变色标（`fills/<i>/stops/<j>/color`、`strokes/<i>/stops/<j>/color`；更早的色标被去掉时绑定跟着它的色标走，列表重排时跟着它的颜料走）和实例的变体属性（`componentProperties/<属性>` 配字符串变量：实例显示它的值所点名的变体；点名不到变体的值什么都不改）。取色器绑定选中的色标。设计面板的变体下拉接受字符串变量。

### 评论

评论线程活在文件里，钉在页面点上或图层上（`anchor`：节点 id，或 `true` 表示点下面最内层的画框），所以它们跟着它移动。图层被删除时，钉子留在原来的地方，在最近的还在的父图层上。用 `run_command` 驱动：`comment.add {x, y, text, page?, anchor?}`（返回 `{id, link}`）、`comment.reply {id, text}`、`comment.edit {id, text, reply?}` 和 `comment.delete {id, reply?}`（一条消息是线程的第一条，或 `reply`：它的回复的索引；删第一条消息会删整个线程）、`comment.react {id, emoji, reply?, on?}`（切换你的反应）、`comment.resolve {id, resolved?}`、`comment.move {id, x, y, anchor?}`（钉到新点下面的画框，除非 `anchor: false`）。代理用它自己的名字评论，从不用用户的：MCP 客户端运行的每条命令都以那个客户端在 `initialize` 里的名字行事（`clientInfo.title`，否则 `clientInfo.name`），后面加 " (agent)"，例如 "Claude (agent)"，客户端什么都没给时是 "AI agent"。它的消息、反应和已读状态是它自己的，它只能编辑和删除自己的消息，它的消息在 `comment.list` 里带 `"agent": true`（应用在名字旁边显示 **Agent** 标签）。偏好设置里的用户名不变。每个连接设一次，所以没有命令需要 `author`。`author` 参数仍然优先。在应用里，评论用偏好设置里的名字（`pref.userName {name}`，默认 "You"）。只有消息的作者能编辑或删除它。消息里的 `@名字` 提及某人（`comment.people` 列出文件里用过的名字：每个写过或反应过的人，以及消息 @提及的每个名字，任何大小写各一次；以及作为 `you` 的、正在行事的人）。

已读状态按名字保存：新消息对除作者以外的每个人都是未读。`comment.markRead {id? | link?, read?}` 把一个线程（或不给 `id` 时全部）标为已读或未读（不是撤销步）。`comment.list {page? | "all", filter?（all|open|resolved|mine|unread）, sort?（date|page）, query?, frame?, mentions?}` 返回线程，带它们的位置、`frame`/`frameName`、`pageName`、`unread`、`link` 和回复（带 `edited` 时间和 `reactions`）。每条消息，第一条和每条回复，都有 `mentions`：它按应用高亮它们的规则 @提及的名字，每个是 `{name, known, you}`。`known` 表示文件里的人（`comment.people`，或你）有这个名字，按他们的拼法。`you` 表示它点的是你（或 `author`）。`@名字` 算在文本开头，或在一个不是字母或数字的字符之后，所以 `a@b.com` 谁也不提及。未知的名字是 `@` 后面的字母和数字。`mentions: "<名字>"`（任何大小写；开头的 `@` 可以）只列出有一条消息、第一条或一条回复，按那些 `mentions` 报告的方式提及那个名字的线程，并和其他过滤组合：`{"mentions": "Claude", "filter": "unread"}` 是标记了 Claude 的未读线程。文件认识 Ada Lovelace 时，`@Ada Lovelace` 提及 "Ada Lovelace"，不是 "Ada"。文件里谁都没有的名字仍可以是几个词（`"Zoe Smith"` 能找到 `@Zoe Smith`）。链接是 `figcraft://comment/<id>`：`comment.link {id}` 返回一个，`comment.open {id | link, view?}` 去它的页，缩放到它的画框（`fit`，默认；`center` 保持缩放；`keep` 不移动），并在应用里打开它。`comment.next {id?, dir?, …列表参数}`（`mentions` 也行）以同样方式走到列表的下一个或上一个线程。两者都返回 `comment.list` 列出的那个线程，带每条消息的 `mentions`。`comment.zoom {ids}` 放大一组钉子。实时会话里造出的评论 id 是大数字（它们创建者的 id 空间）：按原样传回去。

代理可以把每个打开的线程读成一个资源（`figcraft://comments`），或调用 `comment.list`。`comment.prompt {ids?, page?（默认 "all"）, filter?（默认 open）, sort?, query?, includeResolved?}` 返回 `{count, truncated, text, comments}`。`text` 是一份 Markdown 任务说明，带文档、每个线程的页、画框、锚点、点、消息、回复和链接，以及修复、回复和解决的指示。`comments` 是最多 200 个选中线程的有界元数据（id、页、画框、锚点、状态、链接、作者、预览、回复数和 `mentions`：整个线程 @提及的名字，按 `comment.list` 对每条消息的报告，最多 32 个，名字截到 80 个字符），不是完整线程：那些用 `comment.list`。评论面板的 **复制供 AI 使用** 用它。`fix_comments` 提示跑审查循环：检查锚点图层，做最小的改动（每个线程一个 `batch`），回复改了什么，用截图核对，然后解决。先回复再解决：`comment.reply` 会重新打开线程，所以先解决再回复会让它再次打开。`auto_resolve: "no"` 让代理回复并把线程留给用户。评论是任何 MCP 客户端的反馈队列。

接上时，`wait_for_changes` 对应用的事件日志做长轮询：`comment.added`、`comment.replied`、`comment.edited`、`comment.reacted`、`comment.resolved`、`comment.moved` 和 `comment.deleted` 带着 `{command, commentId, reply}`。事件之后用 `comment.list` 读线程。回滚的 `batch` 不报告它各步的事件。日志保留最新的 1024 个事件。`reset: true` 表示游标超前了（应用重启了），`dropped` 数比保留窗口更旧的事件。无窗口立刻返回。它后面的控制方法是 `events.poll`（见 [control-protocol.md](../control-protocol.md)）。

### 版本历史

文档的具名快照，保存在 `.figcraft` 文件里（文件保存之后它们才持久。新建、改名或删除一个版本会把文件标为已更改）。用 `run_command` 驱动：`file.saveVersion {name?, description?}`（返回 `{id, index, name, description, time}`；未命名的版本叫 "Version N"）、`version.list`（最旧的在前；`time` 是 Unix 秒）、`version.restore {id | index}`（用这个版本替换设计，一个撤销步；文件名和评论留下，版本里没有的图层上的评论留在原地。被替换的设计先留成一次自动保存，除非它的一个版本刚刚保存或恢复过）、`version.rename {id | index, name, description?}`（有标题的自动保存变成具名版本）、`version.delete {id | index}` 和 `version.duplicate {id | index}`（把版本作为新文档标签打开）。`version.preview {id | index}` 用一个版本代替文档显示，只读：编辑、撤销和手势被拒绝，直到 `version.restore` 保留它，或 `version.exitPreview` 回去。保存、关闭或保存一个版本会先结束预览，`version.list` 把正在显示的版本标为 `previewing`。索引会校验（负数或超出范围的索引是错误）。一份文件最多保留 100 个版本。`version.autosave` 在文档自上次版本以来有未保存更改时保存一个自动检查点（`auto: true`，名叫 "Autosave"）。应用对每份打开的文档每 30 分钟跑一次，一份文件保留最新的 20 个自动保存（用户版本从不会为它们被丢掉）。合并分支会先把主文件按当时的样子加进去（自上次版本以来有更改时是一次自动保存），然后是 `Merged “<分支>”`。`file.versionHistory` 返回同一份列表，并在应用里打开面板。版本留在保存它们的人那里：实时会话里不分享。

### 分支

分支是文件的一份副本，用来试改动，对照主文件审查，再合并回去。分支保存在主 `.figcraft` 文件里（保存任一标签都会保存带分支的主文件）。用 `run_command` 驱动：

- `branch.create {name?}` 把文件复制进一个新的分支标签，返回 `{id, name, index}`。分支记住文件现在的样子（它的基线），并在它自己的 id 空间里造新 id，所以两边造出的图层、样式和评论从不会冲突。它的历史以一个 `Branch created` 版本开始。
- `branch.list` 列出所显示文件的分支（或分支标签所属文件的分支）：`[{id, name, time, state: active|merged|archived, index?}]`（`index`：它打开的标签）。
- `branch.open {id}` 在标签里打开一个分支（或切到它）。像任何文档一样编辑它。
- `branch.review {id?}`（默认：正在显示的分支标签）返回自基线以来每一边改了什么，以及冲突：`{branch: {layers, document, truncated}, main: {…}, conflicts: [{key, id?, name, property, branch, main, deletedBy?}]}`。`layers` 是更改过的图层，属性更改的格式像检查（`fill #2563EB → #DC2626`）。`document` 列出样式、变量、评论和文件设置。冲突是同一个属性在两边改成了不同的值（键 `<图层 id>/<属性>`，例如 `4/fills`；填充、描边、效果、布局网格、导出设置、参考线、原型交互、标注和开发模式资源逐项比较，所以在每一边改一个列表的不同项，或在一边的末尾添加项，合并时没有冲突；其他列表，例如代码链接，整份比较；图层的父图层和位置是一个属性，`parent`），或一边删了图层而另一边编辑了它或它里面的东西（`<图层 id>/layer`，带 `deletedBy`），或文档级的条目或设置在两边都改了（`style/<id>`、`variable/<id>`、`comment/<id>`、`doc/<属性>`…）。
- `branch.merge {id?, resolutions?}` 把分支合并进主文件，一个撤销步，归档它（`state: merged`），关掉它的标签并显示主文件。主文件的版本历史得到这次合并的一个版本（`Merged “<名字>”`，在主文件有更改时它当时样子的一次自动保存之后）。`resolutions` 把每个冲突键映射到 `"branch"` 或 `"main"`。还有冲突没有选择时，合并被拒绝，这样任何一边的工作都不会在没有选择的情况下被丢掉。
- `branch.updateFromMain {id?, resolutions?}` 以同样方式把自基线以来主文件的更改带进分支（新基线是主文件现在的样子），并在分支的历史里加一个 `Updated from main` 版本。它不可撤销：分支标签的撤销历史重新开始。
- `branch.archive {id?}` 把分支放到一边而不合并（它留在文件里，仍可打开，但不能合并）。

每个分支保留自己的版本历史：在分支标签上，`file.saveVersion`、`version.autosave` 和 `version.*` 命令（列出、预览、恢复、改名、删除、复制）作用于分支的版本。它们和分支一起存在文件里，合并或归档时留在它那里。

实时会话里分支以同样方式工作，并留在造出它们的人那里（别人看不到）。会话里造出的分支在你自己的客户端空间里造 id，远高于主文件造的 id，所以会话服务器接受它们。分支打开时每个人在主文件上的编辑，在审查里是主文件的更改（对照分支自己的基线），和它们的冲突按离线方式解决。`branch.merge` 是会话文档的一次编辑：每个人都得到它，它是你自己撤销历史的一步。查看链接上会拒绝。会话以外（或更早一次会话里）造出的分支，如果它的新图层、样式、变量或评论落在另一个 id 空间，服务器会拒绝，这种也会拒绝。等文件离开会话再合并那些。会话还在启动时，需要你的客户端 id 的分支命令会等它。

### 协作更改

`collab.diff {against?（undo：上一个可撤销的动作，默认 | saved：自上次保存以来）}` 返回 `{changes, count, bytes}`：实时会话会为那次编辑发送的更改批次（按属性的 `create` / `set` / `delete` 和文档级更改，见 `docs/collab-protocol.md`）以及它的二进制大小。它什么都不改。在实时会话里，撤销是按人的（`edit.undo` / `edit.redo` 只收回你自己的步，见 `docs/collab-protocol.md`），只有 `saved` 适用。

### 窗口

接上的桌面版可以在几个窗口里显示文件，每个窗口有自己的标签。文件、偏好设置、库和剪贴板是共用的。用 `run_command` 调 `window.new {path?, index?}` 打开一个（新文件、`path` 处的文件，或把打开的文件 `index` 移出它的窗口），`window.focus {id}` 把一个带到前面。`inspect_ui` 把它们列为 `windows`（`id`、`main`、`title`、`files`、`active`、`focused`）。工具作用于最前面的窗口（最后打开、聚焦或点击的那个）：先聚焦窗口再改它的文件，并把 id 传给 `screenshot {window}` 来看它。无窗口（一个会话，没有窗口）和浏览器里这两条命令什么都不做。

### 点名文档

工具作用于最前面的文件，所以接上的应用里，用户在任务中途点了另一个标签，会改变下一次编辑落在哪里。`run_command`、`batch`、`create_node`、`set_properties`、`inspect_document`、`get_node`、`select`、`export`、`screenshot`、`save_file`、`undo` 和 `redo` 接受可选的 `document`：文件的 `id`（在 `run_command {command: "doc.list"}` 里，以及 `open_file` 和 `inspect_document` 返回的 `document` 大纲的 `id`）或一个打开文件的路径。调用作用于那份文件，最前面的标签留在前面，除非命令自己打开或切换到另一份文件（`file.open`、`file.new`、`doc.switch`），和没有 `document` 时一样。没打开的文件是错误。`doc.list` 给代理每个打开文件的 `id`、名字和状态，不给它在磁盘上的位置（#1117）。`screenshot {window}` 截取窗口显示的东西，所以它需要文件在前面。在控制通道上，`engine.execute`、`document.inspect`、`node.get` 和 `ui.render` 接受同样的 `document`。其他方法拒绝一个点名了前面标签后面的文件的参数。无窗口会话也接受它。

### 实时会话

人（和代理）一起编辑一份文件，并看到彼此的光标、选择和视口。无窗口会话和接上的应用都可以托管和加入（网页版可以加入但不能托管）。每次调用收进上次以来发生的事，并送出你的编辑（加入的文件作为新文档打开）。

会话按文档：每份打开的文档有它自己的会话或没有，几个可以同时跑（托管一份、加入另一份、发布第三份）。另一份文档是当前的时，它们继续同步。在那里你的在场是空的（没有光标、选择或视图）。接上的代理在应用里自己的在场（见开头）从不是实时会话在场的一部分。`collab.*` 命令和 `collab.peers` 说的是**当前文档的**会话（`doc.switch {index}` 改变是哪一个），`collab.peers` 里的 `sessions` 列出全部。关闭文档只结束它的会话：托管者停止托管它，客人离开它。

- `collab.share {via?, access?, server?, name?, port?, bind?}` 用一个**加入码**分享当前文档，这是做 `collab.publish` 和 `collab.host` 所做之事的短办法。`via: "online"`（默认）把它发布到偏好设置里的服务器（`pref.publishServer {url?}`：除非改过，否则是 FigCraft 服务器；`{url: ""}` 关掉在线分享，`{}` 回到内置的那个；`server` 为这次调用点名另一个，需要密钥时带 `?key=…`），并返回 `status: "connecting"`。一旦 `collab.peers` 说 `"live"`，`code` 就是给别人的码（`K7M-2QX-9TD`），`share` 是 `{via, access, shared, editCode, viewCode, commentCode, editLink, viewLink, commentLink}`：为别人做的链接和码（`published.edit` / `published.view` 和 `link` 是你自己进去的办法；留着它们）。`access: "view"`（或分享着的时候再带它调用一次）让 `code` 成为只能查看的那个，`access: "comment"` 是可以查看和评论的那个（用它加入的人可以添加、回复、反应和解决评论，但不能改设计。比评论链接更旧的服务器上 `commentCode` 是 null，那时再要它会失败）。比加入码更旧的服务器让 `code` 为 null：改给 `share.editLink` / `share.viewLink`。`via: "local"` 为同一网络上的人托管文档，并立刻返回一个更长的 `code`（20 个符号，带着这台机器的 IPv4 地址、端口和会话的秘密；`bind: "local"` 把它限制在这台机器）。每个加入的人都能编辑。**调用它会把文档发布到别人运行的服务器上：只有在被要求分享时才做。** `collab.stop {}` 停止分享。本地：会话对每个人结束（`{ended: true}`）。在线：`{ended: false, shared: false}`：别人的链接和码被收回，他们立刻断开（带着他们的副本），而你留在文件里。文件留在服务器上，只对你开放（`share.shared` 是 false，`code` 是 null）。然后 `collab.share {}` 用新码再次分享同一份文件，`collab.leave {}` 把它留在那里，`collab.manageFile {file, action: "remove"}` 把它从服务器拿掉（文件记在你的在线文件里：见下面的 `collab.files`）。
- `collab.host {port?, bind?}` 开始托管当前文档（那份文档所在的会话结束；其他文档的会话继续）。`bind` 是 `"local"`（这台机器，默认）、`"lan"`（本地网络）或一个接口地址。端口是 47900，被占用时是任意空闲端口，第二份被托管的文档也一样（`port` 选一个，0 是任意）。返回会话，格式同 `collab.peers`，带 `link`（`figcraft://join?host=…&file=…&token=…`）和 `url`（`ws://…/?file=…&token=…`）：两者都带着会话的秘密，所以只分享给邀请的人。`collab.stop {}` 结束当前文档的会话。
- `collab.join {code}` 或 `collab.join {link}`（任一种加入码：空格、短横和大小写无所谓，`O` 读成 `0`，`I` 或 `L` 读成 `1`；或任何形式的链接：应用的邀请、`figcraft-server` 分享链接，或 `ws(s)://…?file=…&token=…` 网址；任一参数接受任一种）立刻返回新会话（`session`，`status: "connecting"`），不碰其他文档的会话。它的文件作为新文档打开，并成为当前文档：轮询 `collab.peers` 直到 `status` 是 `"live"`。在那之前，如果发起它的那份文档自己不在会话里，加入显示在那份文档上，并且不管怎样都在 `sessions` 里。加不进去时它在那里变成 `"failed"`，带一个 `reason`（"No session with that code"、"That code has expired or its sharing was stopped"、"Couldn't reach the server (…)"、你的地址错码太多、错误的令牌、会话已满、被收回的链接、到不了的主机、证书不受信任的 `wss://` 服务器：见 [`server.md`](../server.md) 里的 `FIGCRAFT_CA_FILE`）。`collab.leave {}` 离开当前文档的会话（或结束从它发起、仍在路上或已失败的加入），`collab.leave {session}` 离开那个 id 的会话（来自 `sessions`），不管哪份文档是当前的。同步过的文档作为你的副本留着打开。
- `collab.publish {server, name?, key?}` 把当前文档放到 `figcraft-server`（[`server.md`](../server.md)）上，并从那里同步（作为那份文档的会话；其他的继续）。`server` 是 `figcraft-server key` 打印的地址（`ws://host:47900/?key=…`；或把 `key` 分开给），`name` 是文件名（默认：文档的）。它立刻返回 `status: "connecting"`。上传完成之前文档只读，然后 `collab.peers` 有 `published: {server, file, name, edit, view}`，带文件的第一个编辑链接和查看链接（它们带着令牌：像密码一样分享）。用查看链接加入的人得到 `access: "view"`：同步文档上的每次编辑都以 "View only: …" 拒绝（服务器也拒绝它们）。用评论链接（`access: "comment"`）时评论命令工作并同步，其他每次编辑以 "Can comment: …" 拒绝。在对公众开放的服务器上（[`server.md`](../server.md)）`server` 不需要密钥（`wss://figcraft.example.com/`）：`published` 然后还有 `owner`（所有者令牌，只显示这一次：留着）和 `expires`（除非有人打开，服务器何时删除文件，1970 以来的秒），`hosting` 说明谁在运行服务器。不开放的服务器会回答它只在有密钥时接受文件（`collab.peers` 里的 `reason`）。
- `collab.serverInfo {server}` → `{open, operator, terms, idleDays, maxFileMb, expires}`：服务器在发布之前怎么说它的托管（`open`：不需要密钥；`idleDays`：不用密钥发布的文件不打开时保留多久，一直保留时是 null）。比开放托管更旧的服务器被报告为只在有密钥时接受文件。
- `collab.files {refresh?}` → `{files: [{server, host, file, name, created, opened, shared, code, viewCode, state, expires, session, document, hasOwnerToken}], problems: [{server, host, state, reason}], max}`：**你的在线文件**，这次会话（或接上时，带已保存偏好的应用）不用服务器密钥放到服务器上的文件，最近打开的在前。`file` 是其他命令接受的 id。`shared` 说别人现在能不能进来，带可以发出去的 `code` / `viewCode`。`session` 和 `document` 点名文件所在的会话和打开的文档，或为 null。没有 `refresh` 时不问任何服务器，`state` 是上次听到的（问过之前是 `"unknown"`）。`refresh: true` 向每个服务器问一次它的文件后来怎样了：`"online"`，或 `"gone"`（被移除，或未打开而过期）。别处改过的名字和别处收回的链接会被接过来。到不了的服务器，或比这个请求更旧的服务器，列在 `problems` 里（`state: "offline"` 带 `reason`，或 `"unsupported"`），它的文件保持原样。**所有者令牌和你自己的链接从不在结果里**（`hasOwnerToken: true` 代表令牌）。它们和偏好一起保存。无窗口会话只在它运行时保留这份列表：如果文件在它结束之后还要管理，记下 `collab.peers` 里的 `published.owner`。
- `collab.openFile {file, server?}` 作为所有者再次加入你的一份在线文件 → `{session, status: "connecting", document, alreadyOpen, file, server, name}`。它作为新文档打开（轮询 `collab.peers` 直到 `"live"`），在那里 `collab.stop {}` 和 `collab.share {}` 停止和开始它的分享。已经打开的文件改为成为当前文档（`alreadyOpen: true`，`document` 是它的索引）。只有几台服务器上的文件共用一个 id 时才需要 `server`。`state` 是 `"gone"` 的文件会被拒绝。
- `collab.forgetFile {file, server?}` → `{forgotten, file, server, onServer}` 把文件从**只在这台电脑上的**列表里丢掉：文件留在它的服务器上（`onServer`）直到过期，并且不能再从这里管理。要把它从服务器拿掉，改用 `collab.manageFile {file, action: "remove"}`。
- `collab.recoverFile {path, file, server?, session, client, seq}` 是桌面版崩溃恢复对在线文件的副本所跑的（后三个来自副本旁边的记录）：它作为所有者加入文件，保留副本直到文件到达，然后丢掉它（服务器上有一切），把它未确认的更改作为一次同步的编辑加在上面，或在文件旁边把它打开为「<名字> (recovered copy)」。加不进去的文件把副本作为未保存的文档打开。返回 `{session, status: "connecting"}`。
- `collab.manageFile {file, action, server?, owner?, …}` 用所有者令牌管理一份不用密钥发布的文件。`server` 和 `owner` 默认是为你的一份在线文件记录的那些（`collab.files`）。这里没有记录的文件要两个都给。`links` → `{links: [{id, access, label, created}]}`。`createLink {access?: view|edit, label?}` → `{id, access, link}`（一个新的分享链接，只显示一次）。`revoke {link}`（用它加入的人被断开）。`share` → `{shared: true, code, viewCode, editLink, viewLink}`（给别人的新链接和加入码；已经分享时是同样的那些）和 `unshare` → `{shared: false, …}`（记录的链接和码被收回，用它们加入的人被断开；文件在服务器上仍是你的）。`rename {name}`（它在服务器上和你的列表里的名字）和 `remove`（文件从服务器删除，里面的人被断开，它离开你的列表，这里开在它上面的会话结束，它的文档作为副本留着打开）→ `{done}`。错误的令牌，或另一份文件的令牌，会被拒绝。
- `collab.report {reason}`（最多 500 个字符）把你加入的服务器上的文件报告给它的运营者 → `{sent}`。会话的消息说明服务器何时收下了，或为什么没有（每个地址一小时十条）。比这个版本更旧的服务器不接受报告（`hosting` 是 null）。
- `collab.peers {}` → 当前文档的会话：`{available, canHost（网页版是 false）, session（它的 id；文档不在会话里时是 null：那时 status 是 "off"，其余是 null 或空）, status（off|connecting|live|reconnecting|failed）, reason?, attempt?, role（host|guest）, me, name, color, fileName, pages, link, url, address, network, access（edit|view|comment）, readOnly（当前文档现在不能编辑：只能查看、只能评论，或正在发布）, published（你发布的文件：{server, file, name, edit, view, owner, expires}）, code（文档用 collab.share 分享过时给别人的加入码，否则 null）, share（{via: online|local, access, shared, editCode, viewCode, commentCode, editLink, viewLink, commentLink}，或 null；shared: false：你的文件在服务器上，现在没分享）, hosting（你所在文件的服务器怎么说：{open, operator, terms, idleDays, maxFileMb, expires}，或 null）, follow, signals（托管者传递光标聊天和聚光灯）, chat（你的光标聊天）, spotlight（谁在聚光，或 null）, spotlighting（你在）, syncing（文档到了）, document（同步的打开文档的索引）, editing（你正在打字的文本图层：{node, layer, caret, selection}，或 null）, peers: [{id, name, color, editing（他们正在打字的文本图层，同样的形状，或 null）, chat（他们打开的光标聊天消息，或 null）, presence: {cursor: [page, x, y], selection: [ids], viewport: [page, x, y, w, h], editing_text: {node, caret, selection?}}}], sessions: [{session, document（它打开的文档的索引；还在路上的加入是 null）, status, reason, role, fileName, link, peers（里面还有多少别人）}]}`。`sessions` 是应用的每个会话，不管哪份文档是当前的。插入点和选择按字符数。某人打字在他们结束时同步，所以在那之前他们的插入点可能超过你的副本的末尾。在别人正在打字的图层里开始 `text.edit.begin` 会设置消息「<名字> is editing this text too: the version finished last is kept」（两次编辑在每个人结束时作为整份值同步；最后的那个赢）。
- `collab.cursor {x, y}` 把你的光标放到页面位置（两个都省略则隐藏）：代理的指针。你的选择、页面和视图在它们改变时自己送出去。
- `collab.follow {client}` 跟随那个人的视口。`collab.follow {}` 停止（你自己的任何平移或缩放，或 Escape，也会停止）。
- `collab.removePerson {client}` 把那个人从会话里移除：他们被告知会话对他们结束了（他们保留副本，不再连接），会话运行期间他们的地址不能再加入这份文件。只有正在托管的人，或服务器上文件的所有者（带着它的所有者令牌）可以。比这个请求更旧的服务器说它不能。返回 `{removed, name}`。
- `collab.cursorChat {text}` 把 `text` 显示在你的光标旁边，给会话里的每个人（先用 `collab.cursor` 把光标放在某处）：最多 100 个字符，控制字符变成空格，`""` 关掉它。别人在它最后一次更改 5 秒后看到它淡出，所以要保持就再发一次。不带 `text` 时它在应用里打开聊天框（画布上的 `/`），程序调用从不这么做：它返回 `{open}`。别人的消息是 `collab.peers` 里每个对等方的 `chat`。
- `collab.spotlight {on?}`（没有 `on`：切换）请会话里的每个人跟随你的视图和页面。以后加入的人也跟随，任何平移或缩放的人停止跟随（聚光继续；他们可以再跟随）。别人的聚光会接过你的。返回 `{spotlighting}`。两条命令都需要托管者是这个版本或更新的实时会话（`collab.peers` 里的 `signals`）；否则它们说明为什么。
- 你在会话文档上创建的样式、变量、集合、模式和评论从你自己的 id 空间取 id（`S:<n>`、`VariableID:<n>` …，用大数字），所以两个人同时创建从不会互相覆盖。会话以外，id 看起来和以前一样（`S:1`、`VariableID:1`…）。
- 在会话文档上，每条命令（或 `batch`，或手势）是你自己撤销历史的一步：`edit.undo` / `edit.redo` 只收回你的步，留下别人此后改过的。托管者拒绝的编辑（有人删了图层）会回滚，原因是会话的消息。连接正在重连时，编辑继续；连上之后编辑送出去（同一个客户端，补上它错过的）。
- `collab.registerLinks {on?}` → `{on, registered}` 让桌面版成为 `figcraft://` 邀请链接的打开者，或停止（没有 `on`：切换）。只在应用自己注册它们的地方（Windows，按用户）。别处它报告命令不可用。它也设置偏好设置里的复选框。用邀请链接启动应用（`figcraft "figcraft://join?…"`）会加入那个会话。

### 插件

沙箱里的 WebAssembly 插件通过同样的命令行事（清单、ABI 和 Rust SDK 见 [`plugins.md`](../plugins.md)）。用 `run_command` 驱动：`plugin.import {path}`（一个 `figcraft-plugin.json` 或它的文件夹）或 `plugin.import {manifest, wasm}`（清单是对象或 JSON 文本，模块是 base64，只为这次会话）、`plugin.list`、`plugin.remove {id}`，以及 `plugin.run {id, command?, params?, fuel?}`，它返回 `{plugin, command, output, logs, calls, fuel, panel}`。一次运行是一个撤销步，失败时回滚。插件只读文档，除非它的清单要求 `write`，可以被限制在某些命令，并且从不能碰文件、其他文档、撤销历史、对话框或其他插件。无窗口 MCP 会话开始时没有插件：先导入它们。`plugins.runLast` 再跑上一次插件命令。

显示面板的插件（API 2）也可以无窗口使用：`plugin.ui {id?}` 返回插件 `id` 打开的面板，或不给 `id` 时最后打开的那个（`{plugin, command, title, controls, values}`，或 null；几个插件可以同时有面板打开）。`plugin.event {id, control, value?}` 按一下按钮或设置一个字段，和点击一样（插件被回调；一个撤销步；返回像 `plugin.run`，带更新后的 `panel`）。`plugin.close {id}` 结束会话。插件设置按插件和偏好一起保存（无窗口会话里，为这次会话）。

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"plugin.run","params":{"id":"org.figcraft.examples.swatch-grid","command":"panel"}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"plugin.event","params":{"id":"org.figcraft.examples.swatch-grid","control":"palette","value":"Forest"}}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"plugin.event","params":{"id":"org.figcraft.examples.swatch-grid","control":"generate"}}}}
```

小组件（API 3 和 4，插件画在画布上的图层）以同样方式驱动：`widget.insert {plugin, x?, y?}` 在页面上放一个（没有位置时在视图中间）并返回 `{id, plugin, state, targets, output, logs, calls, fuel}`。`widget.click {id, target}` 点击它的一个 `targets`，和在画布上点击一样（插件重画它；一个撤销步；返回同样的东西）。`widget.list` 列出文件里的每个小组件，带它的插件、页、状态和目标。插件 API 4 时，小组件可以有可编辑的文本字段和属性菜单（两者和它一起列出，作为 `fields` 和 `menu`）：`widget.setText {id, field, value}` 设置一个字段，和在里面打字一样（值最多 1000 个字符，进到状态里字段名下面，插件重画；一个撤销步）。`widget.menu {id, item, value?}` 使用一个菜单项（开关的 `value` 是 `true` 或 `false`，默认是现在的相反；颜色的是它的选项之一）。API 5 时，小组件的回调可以打开它的插件的面板：`plugin.ui` 显示它（`command` 是 `widget`），`plugin.event` 使用它，插件在同一个撤销步里重画小组件。小组件的状态是图层数据，所以它和文件一起保存、撤销和同步。

```json
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"widget.insert","params":{"plugin":"org.figcraft.examples.counter"}}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"widget.click","params":{"id":12,"target":"increment"}}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"widget.list","params":{}}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"widget.setText","params":{"id":12,"field":"label","value":"Votes"}}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"widget.menu","params":{"id":12,"item":"big"}}}}
```

来自 `figcraft-server` 插件目录的插件（[`plugins.md`](../plugins.md)）：`plugin.directory.list {server?, query?}` 列出每个插件的最新版本，带它要求的权限（`access`、`commands`）、大小和 SHA-256，以及 `installed` / `update`。`plugin.directory.install {id, version?, confirm: true, sha256?}` 下载它，检查 SHA-256，并为这次会话安装（代理必须传 `confirm: true`，在向用户展示权限之后；没有它什么都不下载）。`plugin.directory.publish {path | plugin, server}` 和 `plugin.directory.unpublish {id, version?, server}` 需要 `server` 里的服务器密钥（`ws://host:port/?key=…`）。

```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"plugin.directory.list","params":{"server":"ws://127.0.0.1:47900/"}}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"plugin.directory.install","params":{"id":"org.figcraft.examples.swatch-grid","confirm":true}}}}
```

### 开发模式

- **页面作为 React + Tailwind：** `inspect.code {id, language: "tailwind"}` 把图层树写成一个 React 函数组件，用 Tailwind CSS v4 工具类，没有样式表。自动布局变成 4px 间距刻度上的 `flex`/`flex-col`/`gap-*`/`p-*`，固定宽度的自动布局画框和文本变成流动的（`w-full max-w-[960px]`；和画框一样宽：`w-full`），画框的固定高度变成最小值，分边的描边变成 `border-b` 之类，实例连同覆盖内联进去（组件代码是 `language: react`），但组件已连接到代码的实例除外（`codeConnect.set`/`codeConnect.import`，React）：那些是它们的片段，例如 `<Button variant="outline">{"See examples"}</Button>`，每个导入在顶部一次。用这种方式映射到 shadcn/ui 的设计系统导出的页面使用项目自己的组件。绑定的变量变成主题工具类（`bg-accent`、`rounded-card`、`text-muted`），由 `variable.export {format: "tailwind"}` 声明：`@import "tailwindcss";` 和一个 `@theme static` 块（每个令牌都发出，包括只通过 `var()` 读到的），按命名空间命名（`--color-*`、`--radius-*`、`--text-*`、`--spacing-*`、`--font-*`；数字的命名空间和单位来自图层把它绑到什么），其他模式在 `[data-<集合>="<模式>"]` 上。带 `mobile: <画框 id>`（同样的图层名，移动布局）时，代码是移动优先：移动画框的类，桌面画框不同的那些在 `lg:` 后面，只在桌面的图层是 `hidden` 加上 `lg:flex`（自动布局画框）或 `lg:block`（其他图层）。
- **组件代码：** `inspect.code {id, language: react|leptos|vue|swiftui|compose|arkui|flutter}` 在组件、组件集或变体上把组件写成可复用单元：带 props 接口的 React（TSX）、带 `#[component]` 函数的 Leptos（`#[prop]` 默认值，`view!` 标记）、单文件 Vue 组件（`<template>`、带 `defineProps` 及其默认值的 `<script setup>`、`<style scoped>`；插槽图层是 Vue 自己的 `<slot>`，所以它不占 prop）、带 `View` 结构体的 SwiftUI、带 `@Composable` 函数的 Compose、带 `@Component` ArkTS 结构体的 ArkUI（`@Prop` 参数，替换和插槽用 `@BuilderParam`）、带 `StatelessWidget` 的 Flutter。它的组件属性是参数，带默认值：变体属性变成字符串联合（React、Vue）或枚举，布尔变成包住它们显示的图层的标志，文本属性是字符串，实例替换是视图（`React.ReactNode`、Leptos `ViewFn`、`AnyView`、可组合 lambda、ArkUI builder、`Widget`），插槽是可选内容。组件集的每个变体共用一个定义。在 React、Vue 和 Leptos 里，其他变体的差异变成 `data-*` 属性上的 CSS 规则。Vue 模板里没有任意表达式，所以 React 写成花括号表达式的东西在那里有自己的写法：文本是 `{{ prop }}`，布尔属性的图层带 `v-if="prop"`，绑定是 `:prop="value"`。在实例上（或含实例的图层树里），每个实例是一次调用，只传它改过的值，例如 `<Button state="Hover" label="Buy" />`、`<Button state=ButtonState::Hover label="Buy" />`、`<Button state="Hover" />`、`Button(state: .hover, label: "Buy")`、`Button(state = ButtonState.Hover, label = "Buy")`、`Button({ state: ButtonState.Hover, label: "Buy" })`、`Button(state: ButtonState.hover, label: "Buy")`。名字被转义并保持唯一（关键字加后缀，`default` → `defaultValue`，Rust 的 `type` → `type_`；Leptos 名字是 snake_case，Dart 枚举成员是 lowerCamel；会遮蔽框架类型的名字也加一个，`Text` → `TextComponent`）。`inspect.componentCode {id?, language?}` 返回各部分：`{component, name, set, properties: [{name, param, type, default, value?（实例）}], definition, usage?（实例）}`。
- **代码连接：** 把组件（变体连接它的组件集）按语言链到它被做成的代码组件（`react`、`leptos`、`vue`、`swiftui`、`compose`、`arkui`、`flutter`）：`codeConnect.set {id?, language?, importPath?, name?, template?}`（可撤销；省略的字段保留原值；默认是生成的名字，以及传递每个属性的一次调用）→ `{component, language, code, unknown}`。模板是文本，从不执行：`{property}`（它的名字或参数名）是为该语言转义过的实例的值（React 和 Leptos 保留标记的花括号，所以 `size={size}` 给出 `size="Large"`，`disabled={disabled}` 给出 `disabled={true}`；Vue 绑定取一个表达式，所以 `:size={size}` 给出 `:size="&quot;Large&quot;"`，`:disabled={disabled}` 给出 `:disabled="true"`——字符串把 JS 引号保留为实体，静态属性（`size={size}`）取纯文本；组件替换是组件本身，`:icon={icon}` 给出 `:icon="Icon"`；需要转义的字符串把转义留在值里面，属性自己的引号变成 `&quot;`；Leptos 用 `use <importPath>::<name>;` 导入），`{property ? "a" : "b"}` 插入两个字符串之一，其他任何 `{…}` 按写的留下（`unknown` 列出不是属性的占位名）。然后对实例或组件做 `inspect.code` 返回导入行和片段，而不是生成的代码（`connected: true`）。生成的图层树里已连接的实例（也在组件的定义里、实例放进它的插槽的内容里，以及已连接模板的实例替换所放的组件里）是它们的片段，每种语言的导入行在代码顶部各一次。`inspect.componentCode` 返回 `connected: {importPath, name, template, import, code}`，片段作为 `usage`。例子：模板 `<Button variant="primary" icon="{Has icon ? "cart" : "none"}">{label}</Button>`，导入路径 `@acme/ui`，给出 `import { Button } from "@acme/ui";`，然后 `<Button variant="primary" icon="none">{"Buy now"}</Button>`。`codeConnect.remove {id?, language?}`（没有语言：全部）。`codeConnect.export {path?}` → `{text, links}`（或写出文件）：`{version: 1, links: [{component, language, importPath, name, template}]}`，已排序，打算作为 `figcraft.connect.json` 活在代码仓库里。`codeConnect.import {json | path}` → `{linked, warnings}` 把每个条目链到每个叫那个名字的组件或组件集（一个撤销步；最多 5000 个链接和 4 MB；模板最多 4000 个字符，名字 100，导入路径 300；无效条目跳过并警告）。`get_node` 把组件的链接列为 `codeLinks`。在应用里，检查页的「连接到代码」一节用代码一节的语言编辑它们，带实时预览和已连接徽章。不需要编辑器扩展：代理和编辑器通过这个服务读同样的片段。清单只要求 `read` 的插件也从检查页运行，带它们的面板（`plugin.ui`）；它们的回调只能读。
- **开发资源：** 任何图层上给开发者的链接（文档、代码、工单），每个是一个 URL 和一个名字。`devResource.add {id?, url, name?}` → `{id, index}`（没有名字：去掉协议、`www.` 和末尾斜杠的 URL；不查找页面标题）、`devResource.edit {id?, index, url?, name?}`、`devResource.remove {id?, index?}`（没有索引：图层自己的全部），都可撤销，以及 `devResource.list {id?}` → `[{name, url, index?（它自己的）, from?（它来自的图层）}]`：实例先列自己的，然后是它的主组件的和那个组件的组件集的（只读）；实例里面的图层显示它的主图层的，不接受自己的。URL 必须是带主机的 `http://` 或 `https://`，没有空格或控制字符，最多 2048 个字符。名字截到 200 个字符。一个图层最多 32 个链接。链接是数据：引擎从不获取或打开它们（应用在被点击时打开一个）。`get_node` 和 `inspect_document` 把它们列为 `devResources`。检查页列出、添加、编辑和删除它们。
- **留在画布上的测量：** 开发者留在画布上给每个人看的距离。`devMode.addMeasurement {from?（默认第一个选中的图层）, to?, side?}` → `{from, index, lines}` 在 `from` 和 `to`（同一页上的图层）之间放一个，沿 `from` 的一边（`side`：`top`、`right`、`bottom` 或 `left`：那一边的长度），或两个都不给时到 `from` 周围的图层。`devMode.removeMeasurement {from?, index?}` → `{removed}`（没有索引：从该图层出发的每一个）。两者都可撤销。`devMode.measurements {page?, id?}` → `[{from, fromName, index, to?, toName?, side?, labels, lines: [{x0, y0, x1, y1, label, dashed}]}]` 列出一页上的那些（默认当前页；`id`：只列出从该图层出发或到该图层的），线条在页面空间里，从图层当前的边界量起，所以它们跟随图层。测量存在它出发的图层上（`get_node` 把它们列为 `measurements: [{to} | {side}]`），和文件一起保存并同步。同样两个图层只量一次（哪个方向都行），一个图层最多出发 64 个。另一图层被删除或移到另一页的那个不列出也不画（撤销会把它带回来）。副本保留边的测量，以及到和它一起复制的图层的那些。在应用里，开发模式用悬停红线画它们，检查页列出它们（什么都没选时，页上的每一个），点击时选中测量的图层，用 − 去掉一个，并从它的添加测量菜单放置新的。
- **标注：** 任何图层上给开发者的笔记，每条可以带可选的钉住属性，开发模式显示它们的当前值（以及后面的样式或变量）。`annotation.add {id?, label?, properties?}`（笔记、钉住的属性，或两者；返回 `{id, index}`）、`annotation.edit {id?, index, label?, properties?}`（`properties` 替换列表）、`annotation.remove {id?, index?}`（没有索引：图层的全部），都可撤销，以及 `annotation.list {ids? | id? | page?（"all"）}` → `{annotations: [{id, name, index, label, properties: [{property, label, value, token?, color?}]}]}`（图层没有那个属性时 `value` 是 null）。可钉的 id：`width`、`height`、`top`、`left`、`rotation`、`opacity`、`cornerRadius`、`direction`、`gap`、`padding`、`alignment`、`fontFamily`、`fontStyle`、`fontSize`、`lineHeight`、`letterSpacing`、`textAlign`、`fill`、`stroke`、`strokeWeight`、`effects`。一个图层最多 64 条标注。笔记最多 5000 个字符。它们和文件一起保存（`get_node` 把它们列为 `annotations`）。实例不继承主组件的标注，并在主组件编辑时保留自己的。在应用里，检查页把它们画成图层旁边的标注框，并在它的标注一节里编辑。
- **标注类别：** 每条标注可以有一个类别，画成它的标注框的颜色。默认是开发、交互、无障碍和内容，文件可以加自己的。`annotation.add` / `annotation.edit` 接受 `category`（id 或名字；`null` 清除），`annotation.list` 返回 `category: {id, name, color}`，并用 `{category: id | 名字 | "none"}` 过滤。`annotation.categories {}` → `{categories: [{id, name, color, count}]}`。可撤销的 `annotation.addCategory {name, color?}` → `{id}`（新的取第一个没用过的颜色）、`annotation.editCategory {id, name?, color?}`（改名、改颜色；id 留下）和 `annotation.removeCategory {id}`（它的标注留下，没有类别）。名字唯一（忽略大小写），最多 64 个字符；最多 64 个类别。文件只在列表和默认不同时才保留列表（旧文件用默认打开）。在应用里，什么都没选时，检查页按类别过滤标注框并编辑类别（颜色、名字、删除、添加）；每条笔记有一个类别菜单。
- **待开发：** 页面或分区顶部的画框，以及分区，带一个交付状态（`node.set {devStatus: none|readyForDev|completed}`；其他图层拒绝状态）。`inspect.readyForDev {status?: readyForDev|changed|completed|any, page?}` 跨页（或在一页上）按页和图层顺序列出它们：`{items: [{id, name, page, status, changed}]}`（`changed`：待开发且此后被编辑过）。检查页设置选中画框的状态，什么都没选时带状态过滤列出它们（待开发、已更改、已完成、任何状态；改过的带已更改徽章；点一个就去那里，或点它的聚焦按钮在聚焦视图里打开）。在应用的开发模式里，画布只检查：按下是选择（以及平移和评论），从不移动、改变大小、绘制或编辑文本和矢量，菜单和快捷键拒绝编辑设计的命令（开发模式自己的标注、状态、开发资源、测量、代码连接和评论仍然工作）。在工具栏里选一个绘制工具会回到设计，图层面板只选择和展开。经 MCP 或控制通道发送的命令不受影响。
- **比较更改：** 标为待开发的图层第一次更改时（它变成「已更改」），文件保留它和它里面一切在那次编辑之前的副本（只在有更改时存在 `.figcraft` 文件里；解决更改、再标一次、另一种状态或删除它会丢掉副本，所以下一次更改和那个状态比较）。`dev.compare {id?, version?, show?}` → `{id, name, changed, snapshot, added, removed, modified, truncated, version: {id, name, time} | null, changes: [{id, name, type, change: added|removed|changed, depth, layers, properties?: [{section, property, before, after, text}]}]}`：图层按 id 匹配，添加和删除的整份列出（`layers` 数里面有什么），更改的带它们的属性，格式像检查（`width 120 → 136`、`fill #FF0000 → #E03030`、`text “Welcome” → “Hello”`、`parent Badge → Screen`、`order 3 → 1`、`component Card → Card 2`、自动布局、排版、描边、效果、圆角、不透明度、可见性、名字），包括嵌套图层和实例内容。`snapshot: false` 表示画框在开始保留副本之前就改了（更旧的文件）。`show: true` 在应用里打开比较视图：之前和现在并排（或用滑块叠在一起），更改的图层勾出轮廓（添加是绿的，更改是橙的，删除是红的），更改列表和解决更改。带 `version`（来自 `version.list` 的 id）时，任何顶层画框或分区，无论是否待开发，改为和那个保存的版本比较（版本里没有的图层作为整体被添加）。比较视图在底部选它（比较对象：标为待开发时，或一个版本；不是待开发的画框和最新版本比较），`ui.set {devCompareVersion}` 设置它。
- **聚焦视图：** `dev.focus {id? | direction: next|previous}` → `{id, name, page, index, count}` 在画布上单独显示一个顶层画框或分区（适应、选中，其他图层既不画也不被点到），需要时切换页面。`direction` 跨页在待开发画框之间走，到头绕回。`dev.unfocus {}` 结束它，Escape（一旦画框或什么都没被选中）、离开页面或删除画框也会。应用显示一条，带画框的名字和状态、比较更改、上一个 / 下一个和退出。`inspect_ui` 报告 `devFocus`。

### 效果

`component.playground {id?, properties? {name: value}, language?, path?, scale?}` 在一份副本上用其他变体、布尔、文本（和实例替换）属性值试一个组件，返回它的属性和值、它的代码，有 `path` 时还有它的 PNG。文档不变。

`style.bind {style, field, variable?}` 在共享样式里面绑定一个变量（没有 `variable`：解开）：颜料样式上的 `fill` 或 `fills/<i>/color` 和 `fills/<i>/stops/<j>/color`，效果样式上的 `effects/<i>/<color|radius|spread|offsetX|offsetY|startRadius>`，网格样式上的 `layoutGrids/<i>/<size|count|gutter|offset>`，文本样式上的 `fontSize`、`lineHeight`、`letterSpacing`、`fontFamily`、`fontStyle`。使用该样式的每个图层在它自己的模式里显示这个字段，并保持链接。`style.list` 给出样式的 `bound` 映射。

`effect.add {ids?, type? | effect?}` 加一个效果（`dropShadow`、`innerShadow`、`layerBlur`、`backgroundBlur`、`noise`、`texture`、`glass`），用它的默认值，或完整的 `effect` 对象。`effect.set {ids?, index, effect}` 替换一个。图层先画它的投影，然后是填充、内阴影、子图层和描边（描边画在内阴影上面），图层模糊、噪点和纹理在结果上面。编组没有自己的轮廓：它的内阴影从它的图层一起画出来的东西周围投射，盖在它们上面，它的背景模糊和玻璃透过它的图层的轮廓显示。

- **模糊**（`layerBlur`、`backgroundBlur`）是均匀的（`{"radius": 8}`）或渐进的：`{"type": "layerBlur", "blurType": "progressive", "startRadius": 0, "radius": 24, "startOffset": [0.5, 0], "endOffset": [0.5, 1]}` 把半径从 `startOffset` 处的 `startRadius` 线性升到 `endOffset` 处的 `radius`，点在图层的单位盒子里（横向和向下 0–1，像渐变的手柄；起点之前和终点之后半径保持）。超过 10000 的半径按 10000 画。在应用里，设计面板的模糊行选均匀或渐进，渐进模糊的设置打开时，它的起点和终点是画布上的手柄。生成的 CSS 省略渐进模糊（CSS 模糊是均匀的）。检查列出这条坡度。
- **导出：** PNG、PDF 和 SVG 画出和画布一样的像素。PDF 和 SVG 让图层保持矢量，并为矢量显示不了的东西嵌入图片：背景模糊和玻璃（来自图层下面的东西：PDF 里是页面上的，SVG 里是导出的图层之间的）、PDF 里的图层模糊、噪点和纹理；SVG 里的噪点颗粒（矢量图层上的一张图片）、纹理和渐进图层模糊（整个图层）。均匀的图层模糊和阴影在 SVG 里仍是 SVG 滤镜。
- **印刷用 PDF：** 出血让每一页在每一边增大那么多，并显示画框那里以外的东西（画框的填充伸到出血边缘，渐变和图片留在原地，它的子图层裁到出血而不是画框，出血以外什么都不画）。只有一个画框的页裁到画框的盒子（它的阴影进到出血里）。裁切标记（在裁切的角上）和出血标记（在出血的角上）是 0.25 点的细线，长 18 点，从出血外面 3 点开始，用套准色。MediaBox 是纸张，BleedBox 是裁切加上出血，TrimBox 是画框。CMYK 用 `K = 1 − max(R, G, B)`、`C = (1 − R − K) / (1 − K)`（M 和 Y 一样）把每种颜色（填充、描边、文本、渐变色标、图片和栅格化的效果）从 sRGB 转换，确定且没有配置文件：文件是没有输出意图的 DeviceCMYK，因为嵌入一个需要 CMYK ICC 配置，FigCraft 不带。软蒙版保持 RGB（它们不印刷），透明组在 RGB 里混合，按 PDF 写出器声明的那样。

### 笔刷和笔压

- `stroke.brush {ids?, preset, spacing?, sizeJitter?, rotationJitter?, scatter?, seed?}` 用笔刷画图层的描边（`preset: null` 回到普通描边；没有描边的图层得到一条黑的）。`node.set {brush}` 接受同样的（预设 id、对象或 `null`），`get_node` 显示 `brush`。拉伸笔刷沿每条子路径弯一个形状：`taper`、`ink`、`ribbon`、`bristle`。散布笔刷沿它每隔 `spacing` 个描边粗细放一份副本（默认 1.5；一条路径最多 5000 份）：`dots`、`stars`、`confetti`、`leaves`，由 `sizeJitter`（0–1）、`rotationJitter`（0–1，最多半圈）和 `scatter`（偏离路径 0–4 个粗细）变化，从 `seed` 可重复。描边粗细设定笔刷的大小，宽度轮廓沿路径缩放它。笔刷在画布、PNG、SVG 和 PDF 里画（作为填充的轮廓），`vector.outlineStroke` 把它们压平。虚线不作用于笔刷。
- **笔压：** 铅笔把压力记进描边的宽度轮廓（按长度最多 32 个点；中等压力 0.5 画出描边粗细，从它的十分之一到两倍）。没有压力（鼠标）时描边保持均匀。`pointer` 接受 `pressure`，拖拽还接受 `pressureTo`。`tool.pencilBrush {preset?, …}` 设置新的铅笔笔画得到的笔刷（`null` 清除；没有 `preset` 则读取）。桌面版在 Windows 上从笔输入读压力；浏览器里从报告力度的触控笔触摸事件读；macOS 和 Linux 的数位板不报告。

### 选择颜色

`selection.colors {ids?}` 返回选择及其后代上的 `{colors: [{paint, style, variable, targets}]}`，重叠的根去重。只列出可见的纯色和渐变颜料。蒙版和布尔操作数省略。样式和变量把本来相同的颜料区分开。每个目标是 `{id, fills, index, range}`。`range` 对节点颜料是 null，对生效的文本范围填充是字符偏移 `[start, end]`。

把返回的目标列表传给：

- `selection.setPaint {targets, paint}` 只替换那些槽（颜料写法和 `fill.set` 一样）。
- `selection.selectColor {targets}` 选中它们各自的所属图层，没有文档撤销步。
- `selection.applyColorStyle {targets, style}` 应用文档的颜料样式。和 `style.apply` 一样，这替换匹配节点的整个填充/描边列表并链接样式。文本范围复制解析后的颜料并保留排版，没有颜料样式链接。
- `selection.bindColorVariable {targets, variable}` 把文档的颜色变量（id 或名字）绑到那些纯色填充和描边槽，和 `variable.bind` 用 `fills/<i>/color` 或 `strokes/<i>/color` 一样。列表上的颜色样式和该槽更早的绑定会被去掉。自己有填充的文本范围取变量在图层模式里的值，没有绑定。渐变和其他变量类型会被拒绝。作用范围只过滤选择颜色菜单提供什么，和 `variable.bind` 一样。
- `selection.detachColor {targets}` 解开那些槽的颜色样式和颜色变量，保留它们的值。

在一次实时交互里让目标保持固定。一种颜色变得和另一种相等，不得给这次手势添加槽。无效或缺失的目标会原子地失败。直接的颜料编辑在颜料值改变时分离绑定（只有不透明度时保留颜色绑定）。文本范围的编辑保留其他字符仍继承的节点绑定。每条会修改的命令是一个撤销步（一次交互可以把它们分组）。

### 颜料、效果和布局网格列表

图层的填充、描边、效果和布局网格是**底部在前**的列表：索引 0 先画，最后一个在最上面（设计面板反过来列，最上面的在前）。

- **重排：** `fill.move`、`stroke.move`、`effect.move` 和 `grid.move {ids?, from, to}` 移动一项（给出的每个图层一个撤销步；超出范围或相等的索引什么都不改，负数或小数被拒绝）。这一项保留一切（图片填充的图片、缩放模式和调整，渐变的色标和手柄），链到列表的颜料、效果或网格样式被解开（列表不再和它匹配）。在应用里，用行的把手（字段左边的空条）拖一行；一条线显示它落在哪里。
- **变量绑定跟着它们的项**，当列表重排或一项被去掉时（`fill.remove`、`stroke.remove`、`effect.remove`、`grid.remove`）。被去掉的项的绑定跟着它走。绑定字段（`variable.bind {field}`）：`fill` / `stroke`（第一块颜料的颜色）或 `fills/<i>/color` / `strokes/<i>/color`，`effects/<i>/<offsetX|offsetY|radius|startRadius|spread|color>`，`layoutGrids/<i>/<size|count|gutter|offset>`（数字变量；数量保持在 1–1000，尺寸和间距从不为负）。移动一块绑定的颜料会改写它的键（`fill` 变成 `fills/2/color`）。
- **实例属性：** `variable.bind {ids: [实例], field: "componentProperties/<名字>", variable}` 用布尔变量驱动布尔属性，用字符串变量驱动文本属性（其他类型拒绝）。值像任何绑定一样跟随变量的模式，绑定跟随属性的改名，删除属性、重置实例或分离它会丢掉绑定。实例的属性面板用它的变量按钮提供同样的东西。

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"fill.move","params":{"ids":[2],"from":0,"to":2}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"variable.bind","params":{"ids":[5],"field":"layoutGrids/0/gutter","variable":"gutter"}}}}
```

### 标尺参考线

`guide.add {vertical, offset, frame? | page?}`（返回 `{index}`）、`guide.move {index, offset, …}`、`guide.remove {index, …}` 和 `guide.clear {…}` 编辑当前页、另一 `page`，或顶层 `frame` 的参考线（页面上或分区里的画框、组件和实例；嵌套画框被拒绝）。画框的参考线在它自己的坐标里：跟着画框移动，只跨过它，和它一起复制，并吸附它里面的图层。画框被移进另一个画框时保留它的参考线，但它们停止显示、吸附和测量，直到它再次成为顶层。在旋转过的画框上，它们沿它自己的轴走，里面的图层沿那些轴吸附到它们（以及它们的兄弟和画框的边），在画框坐标里保持整像素。

`guide.select {index, frame? | page?}` 选中一条参考线（并取消选择图层；`{}` 取消选择它），和在应用里点一条一样。`guide.remove {}` 不带索引时去掉选中的那条（上下文菜单的移除参考线），去掉另一条时保持选中的那条仍被选中。参考线被选中且没有图层被选中时，Delete 去掉它，横穿它的方向键微移它（Shift：大步），Escape 取消选择。在应用里，从标尺拖出并落到画框上的参考线成为那里顶层画框的参考线，沿更靠近标尺的那根画框轴。Alt 拖一条参考线放下一个副本，拖的时候按住 Alt 显示它到它所穿过的画框里面最近边的距离（拖之前选中的画框，否则是它自己的）。

布局网格：固定宽度（左、中、右）且 `count` 为 0 的列或行网格是自动，能放多少放多少（拉伸的网格至少显示一个）。转到离开 0° 的画框上，网格不显示也不吸附。

### 测量、缩放工具、弧手柄和选择

- **测量**（Alt/Option 悬停显示的）：`measure.distances {ids?（默认选择）, to?（一个图层；默认选择的父图层）}` 返回页面坐标里的 `{lines: [{x0, y0, x1, y1, label, dashed}]}`，在边界盒之间（多个图层用它们的并集，旋转过的用它们的边界）：一个盒子在另一个里面时量到每一条边，分开的盒子量空隙（`dashed` 线把空隙线延伸到它旁边经过的盒子），重叠的盒子量匹配的边之间。`measure.padding {id?（自动布局画框或一个子图层）}` 返回 `{frame, lines}`：画框的内边距和相邻子图层之间的空隙（Shift+Alt 悬停显示的）。用 `pointer` 时，`alt: true` 悬停是测量（加 `cmd` 取嵌套图层，`shift` 取内边距），`alt` 拖显示到邻居的实时距离。`inspect_ui` 在 `tool.overlay` 下报告它们（`guides`，以及量到的盒子的 `measure_box`）。
- **按间距复制：** 用 `pointer` 把副本从图层上 Alt 拖开时，也会吸附到那个图层（它的边、中心和相等间距）。`edit.duplicate {ids?, dx?, dy?}` 不带 `dx`/`dy` 时重复上一次复制的页面偏移，条件是它的副本就是它要复制的（并且仍是选择），在它运行时测量：Alt 拖，或复制再移动一个副本，然后每次 `edit.duplicate` 再走出同样远的下一个。自动布局放置的副本不设步长。显式的 `dx`/`dy` 总是优先。
- **缩放工具**（`tool.scale`，K）：用 `pointer` 拖选择手柄时，用一个倍数缩放一切（尺寸和位置，不管约束、描边、效果、圆角、自动布局间距、最小/最大尺寸、布局网格、参考线、平铺图片、文本），每次拖一个撤销步，绕锚点 `tool.scaleAnchor {anchor?: tl|t|tr|l|c|r|bl|b|br}`（或锚点落在正在拖的那一边时，绕对面的手柄；`alt: true` 从中心缩放），拖的时候倍数和支点在 `tool.overlay.scale` 里。不拖时，`transform.scale {ids?, factor, cx?, cy?}` 用数字做同样的事。
- **弧手柄：** 用移动工具时，单个选中的椭圆（屏幕上至少 40 像素，未锁定）显示它的弧手柄，`inspect_ui` 在 `tool.arcHandles` 下列出（页面坐标里的 `{id, sweep: [x, y], start, ratio}`；有弧时才有 `start`，有弧或环时才有 `ratio`）。用 `pointer` 拖它们。**扫过**从右沿里面开始。绕椭圆拖，它打开一条弧：从右沿向上为正，向下为负。回到起点时再次闭合。**起点**转动弧并保持它的扫过。**比例**（中心，或环上绕到一半的内沿）向外拖做出环，拖进空隙显示另一部分。`shift: true` 把角度吸到 15° 一步，比例吸到 5% 一步。每次拖是一个撤销步，并写出 `node.set` 的 `arcStart` / `arcSweep`（度）和 `arcInner`（0–0.99）。设计面板的弧字段（起点 °、扫过 %、比例 %）写同样的键。绑到 `arcSweep` 和 `arcInner` 的数字变量（`variable.bind`）装的是百分比，和字段显示的一样，绑到 `arcStart` 的是度。拖的时候 `tool.overlay.arc` 装着读数（`{handle, label: "Sweep 75%", at}`），`tool.overlay.arc_hover` 装着指针下的手柄。
- **圆角和星形比例手柄：** 选中的矩形或画框（屏幕上至少 48 像素）在每个角里面有一个手柄，多边形或星形在它的顶点里面有一个，星形在它的第一个内点上有一个比例手柄。`inspect_ui` 在 `tool.shapeHandles` 下列出（`{handle: {kind: radius, corner} | {kind: ratio}, at}`）。沿角的平分线拖圆角手柄设置 `cornerRadius`（停在角相遇的地方）。`alt: true` 只设置矩形或画框的那一个角（`topLeftRadius`…）。拖比例手柄设置 `innerRadius`。每次拖一个撤销步。`tool.overlay.shape` 装着读数（`{handle, value, at}`）。`node.set {pointCount}` 把星形保持在 3–60 个点。`miterAngle`（度，设计面板的斜接角）设置斜接限制（`limit = 1 / sin(angle / 2)`）。`get_node` 报告不是斜接的连接（`strokeJoin`），或不是默认约 28.96° 的斜接角（`miterAngle`）。
- **选择：** `select.layersAt {x, y}` 列出页面点下面的每个图层，最前面的先，带它的深度（上下文菜单的选择图层）。`select.set`、`select.children` / `select.parent`（Enter / Shift+Enter）、`select.next` / `select.previous`（Tab / Shift+Tab）。用 `pointer` 时，`cmd: true` 点击选择嵌套图层，从空白画布 `cmd` 拖选择它们碰到的嵌套图层，`shift` 拖切换它们碰到的。在有图层的顶层画框的背景上按下（画框未被选中）会在它里面开始框选，和在画布上一样。空画框被选中并拖走。锁定的图层（可以用 `select.set` 选中）没有手柄，选择的其余部分被拖时它们留在原地。`select.all` / `select.inverse` 跳过隐藏和锁定的图层。
- **变换手势：** 用 `pointer` 时，`shift` 保持比例，`alt` 从中心改变大小（两个一起也行），拖过对面的边会翻转（多个图层镜像并翻转）。编组和多个图层沿每个图层自己的轴缩放，保留它的旋转和翻转。改变画框大小时 `cmd` 忽略它的子图层的约束（它们留在原地），并且和每次拖一样关掉吸附。移动吸附到边、中心和相等间距（空隙显示在 `tool.overlay.guides` 里，带标签）。像素网格打开时，移动和改变大小落到整像素上。

工具失败作为带 `isError: true` 和一条消息的结果返回。协议问题（未知工具、畸形 JSON、错误的参数类型）是 JSON-RPC 错误。没有什么能让服务崩溃。工具或命令不接受的名字（`run_command {args}` 当成 `params`、一步里除 `command`、`params` 和 `as` 以外的键，或它的 `list_commands` 条目没列出的参数，例如 `comment.list {filterr}`）是错误，并列出它接受的名字，从不会被忽略。`ids` 和 `id` 每条命令都接受。

## 资源

| URI | 内容 |
|---|---|
| `figcraft://document` | 当前文档（像深度 8 的 `inspect_document`）。 |
| `figcraft://commands` | 命令目录。 |
| `figcraft://comments` | 每一页上每个打开的评论线程，JSON。 |
| `figcraft://node/{id}`（模板） | 一个图层，子图层到深度 2。 |
| `figcraft://command/{id}`（模板） | 一条命令的说明，例如 `figcraft://command/layout.add`。 |

## 提示

| 提示 | 参数 |
|---|---|
| `mobile_screen` | `screen`（必填）、`device`（默认 "iPhone 16"）、`brand_color` |
| `design_system` | `name`、`primary` |
| `landing_page` | `product`（必填）、`width`（默认 1440）、`accent` |
| `prototype_flow` | `screens`（逗号分隔）、`transition` |
| `fix_comments` | `page`（默认 "all"）、`auto_resolve`（默认 "yes"）、`author`（默认 "AI"） |
| `watch_comments` | `minutes`（默认 5） |

每个返回一份逐步计划，点名要用的具体工具和命令，以及怎么核对。

## 例子

做一个带自动布局的卡片并检查：

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"create_node","arguments":{
  "type":"frame","name":"Card","x":100,"y":100,"width":320,
  "props":{"fill":"#FFFFFF","cornerRadius":16,"layout":"vertical","padding":24,"gap":12,"sizingV":"hug",
           "effects":[{"type":"dropShadow","color":"#00000026","offset":[0,4],"radius":16}]}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_node","arguments":{
  "type":"text","parent":2,"text":"Ada Lovelace","props":{"fontSize":24,"fontStyle":"Bold"}}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_node","arguments":{"id":2,"depth":2}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"screenshot","arguments":{"ids":[2]}}}
```

一个组件，带文本被覆盖的实例（id 是更早的调用返回的；用 `get_node` 读实例的文本图层 id）：

```json
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"component.create","params":{"ids":[2]}}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"run_command","arguments":{"command":"instance.create","params":{"component":2,"x":500,"y":100}}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"type_text","arguments":{"id":7,"text":"Grace Hopper"}}}
```

智能选择的间距（可移动的兄弟图层，排成行、列或对齐的网格）：

```json
{"jsonrpc":"2.0","id":"spacing","method":"tools/call","params":{"name":"run_command","arguments":{"command":"arrange.spacing","params":{"ids":[3,4,5],"horizontal":24}}}}
```

`horizontal` 和 `vertical` 是可选的页面单位间距（至少要一个，绝对值最多 1,000,000）。第一列/第一行固定不动。后面的轨道移动，不改变每条轨道里的尺寸、父图层或偏移。不均匀的间距变成均匀。普通自动布局流里的子图层、锁定/隐藏的图层和实例内部被排除。交互式调用者可以在一次交互里传 `fromGesture: true`，好在拖动过程中继续用它原来的轨道。

对齐和分布合成一个撤销步：

```json
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"batch","arguments":{"label":"Tidy","commands":[
  {"command":"align.top","params":{"ids":[3,4,5]}},
  {"command":"distribute.horizontal","params":{"ids":[3,4,5]}}]}}}
```

默认（`"onError": "abort"`）时，第一次失败回滚整个 batch，成功时按顺序返回原始结果。在 `commands` 旁边写 `"onError": "continue"` 时，失败那一步的文档编辑自己回滚，后面的步仍然运行，结果改为每步一个对象：

```json
[
  {"step": 0, "command": "text.set", "ok": true, "result": {"ok": true}},
  {"step": 1, "command": "text.set", "ok": false, "error": "invalid parameters for `text.set`: step 1: node 2 is not text"},
  {"step": 2, "command": "page.rename", "ok": true, "result": {"ok": true}}
]
```

即使有些步失败，这次工具调用也不是错误（`isError` 是 false）。成功的文档编辑合成一个撤销步。如果每一步都失败，不添加撤销步。未知命令、坏参数和坏引用只让它们那一步失败，但畸形的 `commands` 列表、无效的 `onError`，或两步用了同一个 `as`，仍会在任何一步运行之前拒绝整个 batch。必须等待后台工作的一步会让整个调用等待，和不带 `onError` 时一样。嵌套的 batch 有它自己的 `onError`。

### 步骤引用

`batch` 里的一步可以用更早一步返回的东西。用 `"as"` 给那一步起名（和 `command`、`params` 并列），后面一步参数里任何地方的 `{"$ref": "名字"}`（一个值、`ids` 里的一项、嵌套字段）在后面那步运行之前被替换成那步结果里的 `id`。这样一次调用、一个撤销步就能搭出嵌套的树，不用猜 id，也不用数步：

```json
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"batch","arguments":{"label":"Card","commands":[
  {"command":"node.create","as":"card","params":{"type":"frame","name":"Card","props":{"layout":"vertical","gap":12}}},
  {"command":"node.create","params":{"type":"text","parent":{"$ref":"card"},"text":"Ada Lovelace"}},
  {"command":"node.create","as":"button","params":{"type":"frame","parent":{"$ref":"card"},"name":"Button"}},
  {"command":"node.create","params":{"type":"text","parent":{"$ref":"button"},"text":"Follow"}}]}}}
```

`{"$step": n}` 改为按从 0 开始的位置指一步，能用在同样的地方。两者可以混用。名字是任何非空字符串，在这个 batch 里必须唯一。

`"field"` 取 `id` 以外的东西：结果的一个键（`{"$ref": "card", "field": "name"}`），或进去的一条路径，部分之间用 `/`，列表项用从 0 开始的索引，和 `variable.bind` 命名 `fills/0/color` 的方式一样。`edit.duplicate` 返回 `{"ids": [...]}`，所以它的第一份副本是 `ids/0`：

```json
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"batch","arguments":{"label":"Copy","commands":[
  {"command":"edit.duplicate","as":"copy","params":{"ids":[5]}},
  {"command":"node.set","params":{"ids":[{"$ref":"copy","field":"ids/0"}],"props":{"name":"Card copy"}}}]}}}
```

要编辑副本里面，传 `"map": true`：`edit.duplicate` 然后还返回 `map`，每个原始图层的 id（字符串，因为 JSON 的键是字符串），包括后代，配上它的副本的 id，例如 `{"ids": [8], "map": {"5": 8, "6": 9, "7": 10}}`。嵌套的画框、编组和实例（以及它们的子图层）都在里面。主组件的副本是它的实例，所以它的子图层和实例的配对。于是 `"field": "map/6"` 是图层 6 的副本：

```json
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"batch","arguments":{"label":"Copy","commands":[
  {"command":"edit.duplicate","as":"copy","params":{"ids":[5],"map":true}},
  {"command":"node.set","params":{"ids":[{"$ref":"copy","field":"map/6"}],"props":{"text":"Copy title"}}}]}}}
```

没有 `map` 时，结果仍是以前的 `{"ids": [...]}`。

路径可以以 `/` 开头，像 JSON Pointer（`/ids/0`）。本身含 `/` 的键先按写下的样子匹配，再当作路径读。路径停在它到达的任何值上，所以 `"field": "ids"` 是整份列表。

这些会让整个 batch 失败并回滚，错误点名那一步：两步用了同一个 `as`（任何一步运行之前）；`$ref` 指向没有一步拥有的名字（错误列出已有的名字），或指向同一步或更后面的步；`$step` 指向同一步或更后面的步，或是负数或不是整数；结果里没有的 `field`（错误点名完整路径，对列表还说它有多少项）；`$ref` 或 `$step` 旁边除 `field` 以外的任何键。`"onError": "continue"` 时，除了重复的名字，其余只让它们那一步失败，指向失败了的一步的引用也是（错误把两者都点名）。步保持它们的位置，引用读原始结果。只有装着 `$ref` 或 `$step` 键的对象才是引用：`"$ref card"` 这样的文本按写下的留下。两个键在步骤参数里是保留的：任何地方装着其中一个的对象（插件数据或变量值里也算）都按引用来读。本身是 `command.batch` 的一步按写下的传下去，所以它的步骤里的引用指向它自己的步骤和名字，不是外层 batch 的。外层 batch 可以用路径进到它的结果列表（`{"$ref": "inner", "field": "1/id"}`）。

`crates/mcp/tests/it/agent_tasks.rs` 里的端到端测试是真实任务的实例（自动布局、用步骤引用一次 batch 搭出的卡片、组件和覆盖、渐变和阴影、颜料样式、浅色/深色变量、对齐/分布/编组、布尔减去再压平、导出、撤销/重做、用工具绘制）。
