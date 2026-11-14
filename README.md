# 墨卷 · Mojuan

纯 Rust 写的 Markdown 阅读器 / 编辑器。**一个 8MB 的 exe，双击就能用**——不装运行时、不带 WebView2、不需要安装包。

- **原生渲染**：eframe/egui 立即模式 GUI，冷启动快、内存占用远低于 Electron/WebView 方案
- **阅读优先的排版**：中文字号、栏宽、行距可调；朱砂暖黑 / 米白纸两套主题
- **完整的编辑体验**：实时预览、分屏、源码内联着色、`Ctrl+B/I`、导出 HTML
- **工作区**：打开文件夹、文件树、全文搜索、大纲跳转、最近打开、新建/重命名/删除（进回收站）
- **自带更新**：启动时静默检查 GitHub Releases，有小浮窗提醒；一键下载后原地替换并重启

## 截图

![暗色分屏](docs/split-dark.png)

![亮色阅读](docs/read-light.png)

![首启欢迎页](docs/welcome.png)

## 直接运行

下载 `mojuan.exe`，双击即可。首次启动点「打开文件夹」，或者「更多 → 打开示例文档」看效果。

程序是绿色版：配置写在 exe 同目录的 `data\state.json`；如果 exe 所在目录不可写（比如放进了 Program Files），会自动改用 `%APPDATA%\Mojuan`。

## 从源码构建

需要 Rust 1.80+（开发用 1.96）。Windows 上 MSVC 和 MinGW 工具链都能编译。

```bash
cargo build --release
# 产物：target/release/mojuan.exe
```

> **注意**：如果工程放在含中文的路径下（例如 `D:\缓存\...`），MinGW 链接 proc-macro 时会报
> "cannot find ... .rcgu.o"。把编译产物指到纯 ASCII 路径即可：
> ```bash
> set CARGO_TARGET_DIR=D:\cache\mojuan-target
> cargo build --release
> ```
> MSVC 工具链没有这个问题。

依赖清单在 `Cargo.toml` / `Cargo.lock`，Windows 资源（图标、版本信息）由 `crates/app/build.rs` 在编译时嵌入。

## 快捷键

| 操作 | 快捷键 |
| --- | --- |
| 打开文件 | `Ctrl+O` |
| 打开文件夹（工作区） | `Ctrl+Shift+O` |
| 快速打开 / 命令面板 | `Ctrl+P` |
| 保存 | `Ctrl+S` |
| 新建文件 | `Ctrl+N` |
| 切换 阅读 / 分屏 / 编辑 | `Ctrl+E` |
| 加粗 / 斜体 | `Ctrl+B` / `Ctrl+I` |
| 侧栏搜索 | `Ctrl+F` |
| 关闭当前标签 | `Ctrl+W` |
| 字号增减 | `Ctrl+=` / `Ctrl+-` |
| 检查更新 | `Ctrl+U` |
| 首选项 | `Ctrl+,` |

## 检查更新是怎么工作的

1. 启动 2 秒后静默检查一次（可在首选项关闭），也可以手动「更多 → 检查更新」或 `Ctrl+U`；
2. 请求 `https://api.github.com/repos/{owner}/{repo}/releases/latest`（走系统 WinHTTP，不引入 TLS 库）；
3. 版本号比较（`0.2.0 > 0.1.0`，`jq-1.8.2` 这类带前缀的 tag 也能正确解析）；
4. 自动检查发现新版时**不弹窗**，只在右下角亮一个小浮窗「有新版本 vX.Y.Z」，
   点它才打开详情，点 × 就不再打扰；手动检查才直接弹窗；
5. 点「下载并更新」把新 exe 下到同目录的 `mojuan.update.exe`，然后在进程内完成替换：
   旧 exe 改名 `mojuan.old.exe` 让位 → 新 exe 顶上 → 拉起新进程 → 退出；
   下次启动时自动删掉 `mojuan.old.exe`。全程不写 .bat，也不碰注册表。

更新源默认读 `crates/app/src/update.rs::DEFAULT_REPO`，也可以在首选项里手填 `owner/repo`，
或用环境变量 `MOJUAN_REPO=owner/repo` 临时覆盖（方便测试）。

## 发布新版本（维护者）

```bash
# 1. 改版本号：Cargo.toml -> [workspace.package] version = "0.2.0"
git commit -am "release v0.2.0"
git tag v0.2.0
git push origin main --tags
```

GitHub Actions（`.github/workflows/build.yml`）会自动编译，并在 tag 上创建 Release、挂上 `mojuan.exe`。
程序下一次检查更新时就能看到这个版本。

## 代码结构

```
crates/
  core/                 纯逻辑，不依赖 GUI（可单独测试）
    src/doc.rs          Markdown → 结构化模型（标题/段落/代码/表格/列表/图片）
    src/highlight.rs    代码语义高亮（产出与主题无关的 token）
    src/files.rs        目录、UTF-8/GBK 读写、新建/重命名/删除（回收站）
    src/search.rs       文件名 + 全文检索 + 模糊匹配
    src/store.rs        设置、最近记录（JSON 持久化）
    src/http.rs         零依赖 HTTPS GET（WinHTTP 绑定）
    src/update.rs       Release 查询、版本比较、下载
    examples/           dump（解析结构）、net（网络）、upd（更新）三个调试工具
  app/                  egui 界面层
    src/app.rs          状态与业务动作
    src/update.rs       更新流程编排（线程、状态机、替换重启）
    src/theme.rs        调色板与样式
    src/fonts.rs        系统字体加载（雅黑/雅黑粗体/Cascadia/Emoji 兜底）
    src/icons.rs        手绘矢量图标
    src/img.rs          图片解码 + 纹理缓存
    src/art.rs          程序图标（代码生成）
    src/win.rs          DWM：让系统标题栏跟随暗色
    src/ui/             顶栏/侧栏/标签/编辑器/预览/状态栏/弹层/更新弹窗
    assets/             图标 ico + 内置示例文档
```

## 设计取舍

1. **不用 WebView**：省掉 100MB+ 的 Chromium 依赖，也不会出现"装完打不开"的运行时问题；代价是排版全靠自己画。
2. **自绘编辑器**：egui 的 `TextEdit` 负责光标/选区/IME/撤销，我们只提供 layouter 做 Markdown 内联着色。
3. **自绘表格**：先量出每个单元格的文字高度，再统一画背景、文字、行列线——这样才不会出现"高度为 0 被裁掉"的经典坑。
4. **配色**：暗色暖黑 `#121110` + 朱砂 `#C4614A`；亮色米白纸。代码高亮是语义 token，两套主题各配一份颜色。
5. **字体**：不打包字体（省 5-10MB），读系统雅黑 + 雅黑粗体 + Cascadia Mono + Emoji 兜底，逐级降级。
   egui 没有合成粗体，所以必须单独加载粗体字面，否则标题没有层次。

## 体积与性能（本机 Windows 11 实测）

| 项目 | 数值 |
| --- | --- |
| 单文件 exe | **8.0 MB** |
| 运行时依赖 | 无（不需要 WebView2 / .NET / VC++ 运行库） |
| 冷启动 | ≈1.0 s（其中约 50MB 是系统字体读入） |
| 内存：小文档 | 114 MB WorkingSet |
| 内存：含图片文档 | 154 MB WorkingSet |
| 输入 → 预览刷新 | 90ms 防抖，重排通常 <30ms |

内存的大头是字体（雅黑 19.7MB + 粗体 16.9MB + Emoji 12.9MB）。
去掉 `fonts.rs` 里的 Emoji 兜底可省约 13MB，代价是表格里的 ✅ 变方块。

## 已知限制

- 阅读区暂不支持鼠标框选文字（链接可点、代码块有「复制」按钮）；需要复制请切到编辑模式
- 不支持数学公式、Mermaid；远程图片只显示可点击提示
- 超大文档（>50 万字符）编辑区不着色；预览一次全量布局，没有虚拟滚动
- 自动更新要求 Release 里挂了 `.exe` 资源；没有 exe 的版本只能跳发布页手动下载
- 只在 Windows 11 + MinGW 实机验证；Linux 有 CI 配置但未实机跑过

## License

MIT，见 [LICENSE](LICENSE)。
