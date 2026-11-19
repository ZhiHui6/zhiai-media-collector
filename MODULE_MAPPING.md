# MODULE_MAPPING.md

## 仓库结构

| 路径 | 角色 | 说明 |
|---|---|---|
| Cargo.toml | workspace | members = crates/core、src-tauri |
| crates/core/ | Rust 核心库 | UI 无关的抓取/提取/下载/存储 |
| src-tauri/ | Tauri 2 应用 | 状态、命令、事件桥、图片协议、窗口 |
| src/ | Vue 3 前端 | Vite + TS，B 方案沉浸工作台 |
| dist/ | 前端构建产物 | Vite 输出，Tauri `frontendDist` |
| release/ | 应用产物 | build.bat 复制的 exe |
| package.json / vite.config.ts / tsconfig.json / index.html | 前端工程 | npm 脚本与构建配置 |

## Rust 核心（crates/core）

| 代码文件路径 | 功能描述 |
|---|---|
| crates/core/src/lib.rs | 模块导出（download/error/extract/fetch/imaging/link/model/storage） |
| crates/core/src/model.rs | 共享类型：ImageKind/ImageItem/ParseBatch/ParseOutcome/QueueCommand/UiEvent/Settings/HistoryEntry |
| crates/core/src/error.rs | 类型化错误与中文文案、可重试判定 |
| crates/core/src/link.rs | 粘贴文本 → URL 列表、文章/合集判定 |
| crates/core/src/fetch.rs | UA/Referer、超时、验证页识别 |
| crates/core/src/extract/* | 规范化、页面判定、图文/相册/合集提取、兜底扫描、分类 |
| crates/core/src/download/mod.rs | EventSink：`Sender<UiEvent>` + wake 回调（UI 无关） |
| crates/core/src/download/queue.rs | 单 actor：解析、并发调度、暂停/取消/重试、统计（`pub async fn run`） |
| crates/core/src/download/task.rs | 流式下载 → 嗅探 → 转换 → 原子落盘 → 历史/缩略图 |
| crates/core/src/download/preview.rs | 解析预览（轻量变体）与大图原图加载 |
| crates/core/src/imaging.rs | 格式嗅探、JPG/PNG/WebP 转换、GIF/动画保护、缩略图 |
| crates/core/src/storage.rs | settings/history 读写、原子写、损坏回退、旧目录迁移 |
| crates/core/examples/probe_page.rs | 页面结构侦察探针 |
| crates/core/examples/make_icon.rs | 生成 src-tauri/icons 全套图标（绿底 Z 标） |

## Tauri 壳（src-tauri）

| 代码文件路径 | 功能描述 |
|---|---|
| src-tauri/src/main.rs | Windows 子系统入口，调用 `zhiai_app::run` |
| src-tauri/src/lib.rs | Builder：media 协议、setup（迁移/加载/actor/事件桥/主题）、命令注册 |
| src-tauri/src/state.rs | AppState：命令通道、items/paths/history/settings、MediaCache |
| src-tauri/src/commands.rs | 全部 `#[tauri::command]` 薄包装、`get_bootstrap`、`set_download_root`（校验/创建/落盘）、explorer 打开与定位 |
| src-tauri/src/bridge.rs | UiEvent → JSON payload → `app.emit`；历史 1s 防抖落盘 |
| src-tauri/src/media.rs | RGBA→PNG 编码与 512 条有界 LRU |
| src-tauri/tauri.conf.json | 产品名/标识/窗口/构建命令/frontendDist |
| src-tauri/capabilities/default.json | `core:default` 能力 |
| src-tauri/icons/ | 由 make_icon 生成（icon.ico + PNG 套件） |
| src-tauri/build.rs | `tauri_build::build()` |

## Vue 前端（src）

| 代码文件路径 | 功能描述 |
|---|---|
| src/main.ts | 挂载 App、注册 `v-tip`、全局禁用原生右键菜单 |
| src/App.vue | 左导航 + 沉浸工作台布局、头部/输入/筛选/合集提示、全局快捷键 |
| src/store.ts | 响应式镜像状态、核心事件归并、选择/下载/设置/预览动作 |
| src/api.ts | invoke 包装、`core://event` 订阅、`media://` URL 生成 |
| src/types.ts | 与 core/model.rs 对应的 TS 契约、标签/颜色/格式化 |
| src/tip.ts | 自定义延迟 tooltip 指令（替代原生 title） |
| src/components/VirtualGrid.vue | 行虚拟化网格（176×172 卡片，overscan 2） |
| src/components/ImageCard.vue | 图片卡片：单击选中、双击大图、右键菜单、骨架屏、进度 |
| src/components/QueueBar.vue | 进度条 + 悬浮命令条 + 可展开队列 |
| src/components/SettingsSheet.vue | 右侧浮层设置（WeUI cells、开关、滑杆带范围与数值框） |
| src/components/HistorySheet.vue | 右侧浮层下载历史 |
| src/components/PreviewSheet.vue | 大图预览浮层与单张操作 |
| src/components/ContextMenu.vue | 风格化右键菜单 |
| src/components/Icon.vue | 1.7 描边图标集 |
| src/styles.css | WeUI 亮/暗 token、布局、动效、减少动态偏好 |

## 开发工具脚本

| 脚本 | 用途 |
|---|---|
| dev.bat | 启动 Vite 开发服务器并 `cargo run -p zhiai_media_collector`（debug 读 devUrl） |
| build.bat | `npm run build` → release + custom-protocol → 复制 exe 到 release\\ |
| check.bat | fmt --check → check → clippy -D warnings → test → 前端 typecheck+build |
| probe.bat | `cargo run -p zhiai_core --example probe_page`，输出 probe-output.txt |
