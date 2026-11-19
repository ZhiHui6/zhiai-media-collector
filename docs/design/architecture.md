# 系统架构

## 总览

单进程桌面应用，由三部分组成：

- **Rust 核心 `crates/core`（`zhiai_core`）**：链接识别、页面抓取、图片提取、下载队列、图像处理、持久化。完全不依赖任何 UI 框架。
- **Tauri 2 壳 `src-tauri`**：应用状态、IPC 命令、事件桥、`media://` 图片协议、窗口与主题；依赖 `tauri` 与 `zhiai_core`。
- **Vue 3 前端 `src/`**：Vite + TypeScript，经 `@tauri-apps/api` 的 `invoke` / `listen` 与壳层通信。

```
┌─ WebView2（Vue 3）─────────────────────────────┐
│ 组件状态（store）                               │
│   invoke(commands)            listen("core://event")
│      │                              ▲            │
├──────┼──────────────────────────────┼────────────┤
│ Tauri 壳（Rust）                     │            │
│  命令包装 → command_tx          app.emit(payload)│
│                                    ▲             │
│  事件桥线程 ← std mpsc ← EventSink │             │
│  media://thumb|full/<id> ← PNG LRU 缓存          │
├─────────────────────────────────────────────────┤
│ tokio（tauri::async_runtime）                    │
│  queue actor（单状态权威）→ fetch/extract/task    │
│  imaging / storage / model                       │
└─────────────────────────────────────────────────┘
```

## 关键决策

1. **核心 UI 无关**：`download::EventSink` 只持 `Sender<UiEvent>` 与 `Arc<dyn Fn() + Send + Sync>` 唤醒回调；egui 版曾注入 `ctx.request_repaint`，Tauri 版注入空操作，由事件桥线程负责投递。
2. **单 actor 状态权威**：解析与下载命令统一进 `download::queue::run`，前端只持有镜像状态；并发用 active 计数 vs limit。
3. **命令与事件都做类型化包装**：Tauri 命令是薄包装（`parse_text`、`enqueue` 等）→ 构造 `QueueCommand`；`UiEvent` 在事件桥里转成精简 JSON payload，不把原始 `Vec<u8>` 走 IPC。
4. **图片不经过 JSON**：核心产出 RGBA → 壳层编码 PNG → 有界 LRU（512 条）→ 注册 `media` 自定义协议，按 `thumb/<id>`、`full/<id>` 返回字节。事件只发 `{ id, width, height }`，前端用 `convertFileSrc` 拼 URL，数百图网格不卡。
5. **主循环不 block_on**：阻塞操作（PNG 编码、目录选择）走 `spawn_blocking` / 专用线程；事件桥用 `recv_timeout(250ms)` 兼顾投递与历史落盘。
6. **配置目录沿用核心存储**：`%APPDATA%\zhiai_media_collector\`，启动时 `storage::migrate_legacy_config()` 把旧 `wechat_image_downloader` 目录一次性迁入；不依赖 Tauri 的 app_config_dir，避免二次迁移。

## 依赖方向（无环）

```
src-tauri → { tauri, zhiai_core, image(PNG), rfd }
frontend  → 仅 IPC 契约（src/types.ts 手写镜像 core/model.rs）
zhiai_core → { reqwest, scraper, image, webp, tokio, serde, storage 依赖 }
model.rs 不依赖任何 UI；error.rs 用 thiserror 映射中文文案
```

## 持久化

- `settings.json`：`update_settings` 命令即时写入；前端滑块 300ms 防抖。
- `history.json`：事件桥在 `HistoryAppended` 后 1s 防抖保存，通道断开前强制保存；上限 5000 条。
- 原子写（.tmp + rename），损坏文件改名 .bad 后回退默认值。

## 命令与事件

**命令**：`get_bootstrap`、`parse_text`、`parse_album`、`enqueue`、`open_preview`、`cancel_item`、`retry_item`、`retry_all_failed`、`pause_all`、`resume_all`、`set_concurrency`、`clear_finished`、`update_settings`、`set_download_root`、`reveal_path`、`open_directory`、`apply_theme`。

**事件**：单一频道 `core://event`，按 `type` 判别：`parseStarted` / `batchParsed` / `itemState` / `itemProgress` / `itemCompleted` / `thumbReady` / `fullReady` / `fullFailed` / `queueStats` / `history`。

## 技术栈

- Rust 1.94（MSVC）、Tauri 2.12（wry/tao、WebView2）、tokio 1、reqwest 0.13（rustls + 平台证书验证）、scraper 0.29、image 0.25、webp 0.3、serde/serde_json、regex、url、thiserror、dirs、chrono、tauri-plugin-dialog 2（目录选择）。
- 前端：Vue 3.5、Vite 6、TypeScript 5、`@tauri-apps/api` 2。
- 运行时依赖：WebView2 Evergreen（Windows 10 1803+ 随系统提供）。
- 打包：NSIS / MSI 安装包；图标由 `crates/core/examples/make_icon.rs` 生成到 `src-tauri/icons/`。
