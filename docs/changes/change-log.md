# 变更记录

## 2026-11-08

- 下载目录改为真正可自由设置：设置页支持直接输入或粘贴任意路径（自动去除引号与首尾空白、相对路径转绝对路径、目录不存在自动创建），新增「浏览…」调用官方 dialog 插件（带父窗口、模态不跑到窗后）与「打开」按钮；应用后立即落盘并同步 queue actor。新增 `set_download_root` 命令替换原 `pick_directory`，失败时在行内显示中文原因，并提示实际保存位置「保存目录\文章标题\」。涉及 src-tauri/src/commands.rs、src-tauri/src/lib.rs、src-tauri/Cargo.toml、src/components/SettingsSheet.vue、src/api.ts、src/store.ts、src/styles.css。
- 架构迁移到 Tauri 2 + Vue 3：Cargo workspace 拆为 `crates/core`（UI 无关核心，crate `zhiai_core`）与 `src-tauri`（Tauri 壳）；前端迁到根级 Vue 3 + Vite + TS。移除 egui/eframe/rfd 前端与 src/app.rs、src/ui/*、src/main.rs。
- 核心解耦：`download::EventSink` 改持 `Sender<UiEvent>` + wake 回调，不再引用 egui；queue actor 暴露 `pub async fn run`，由 `tauri::async_runtime` 托管。
- Tauri 壳：AppState（items/paths/history/settings/PNG 缓存）、17 个命令、std mpsc → `app.emit("core://event")` 事件桥（历史 1s 防抖落盘）、`media://thumb|full/<id>` 自定义协议（512 条有界 LRU）、窗口主题同步、rfd 目录选择、explorer 定位。
- 前端（B 沉浸工作台）：深色优先 + WeUI token、行虚拟化网格（176×172）、悬浮命令条 + 可展开队列、右侧浮层设置/历史、全屏预览、风格化右键菜单与延迟 tooltip、骨架屏、焦点环、减少动态降级；禁用原生右键菜单。
- 打包与脚本：`crates/core/examples/make_icon.rs` 生成绿底 Z 标全套图标；build.bat 改为 `npm run build` + release custom-protocol；check.bat 增加前端 typecheck+build；新增 .gitignore。
- PRD 与架构文档同步：非功能需求从「纯 Rust 单 exe」改为「Tauri + WebView2 + 安装包」，网格性能条款改为前端虚拟化 + PNG LRU + media 协议。

- 产品改名为「智爱媒体素材采集器」，去掉平台词与媒体类型词，为视频号/小红书/抖音等后续平台预留命名空间（这些平台尚未实现，界面文案不提前宣称）。crate 与 exe 改为 `zhiai_media_collector`，窗口标题与 eframe 应用 id 同步；配置目录改为 `%APPDATA%\zhiai_media_collector\` 并在启动时一次性迁移旧目录（目标已存在则跳过，rename 失败退回逐文件复制），默认下载目录改为 `Pictures\ZhiaiCollect`，用户已持久化的路径不动。涉及 Cargo.toml、src/main.rs、src/storage.rs、src/model.rs、src/app.rs、examples/probe_page.rs、build.bat、PRD 与架构文档。
- 设置页新增品牌行：智爱标记 + 产品名 + 单行副标题「公众号图片批量采集，更多平台陆续接入」+ 版本号（读取 CARGO_PKG_VERSION）。原生标题栏只承载文字名，未做自绘标题栏标记。涉及 src/ui/settings_panel.rs。

- 界面改为微信 WeUI 视觉规范：亮/暗色 token 直接取自 Tencent/weui（BRAND #07C160、BG-0 #EDEDED/#111、BG-2 #FFF/#191919、FG-0/1/2、FG-3 发丝分割线、LINK #576B95/#7D90A9、RED #FA5151），8px 圆角、无阴影、单一强调色。涉及 src/ui/style.rs。
- 布局重构为「左导航 + 整页」：56px 导航（图片 / 历史 / 设置），设置与历史从侧栏和独立窗口改为主区整页 WeUI cells（行高 40、标签左 / 控件右）；移除常驻右侧设置面板与 220px 常驻队列。涉及 src/app.rs、src/ui/nav_panel.rs、src/ui/settings_panel.rs、src/ui/history_panel.rs。
- 图片页：输入压成单行 + Enter 解析，绿色主操作「下载所选 N」固定工具栏右端；卡片单击选中、双击大图预览、右键风格化菜单（预览 / 复制链接 / 打开所在目录 / 下载此张 / 重试 / 取消）；Ctrl+A 全选、Esc 关闭预览。涉及 src/ui/input_panel.rs、src/ui/grid_panel.rs、src/ui/preview.rs。
- 队列空闲时收成一行状态条，有排队或进行中时自动展开，可手动折叠并记住选择。涉及 src/ui/queue_panel.rs、src/app.rs。
- 新增描边图标体系（1.7 描边、圆头、绝对居中）与统一控件：主/次按钮、筛选 chip、只读状态点 + 完整词、开关（仅开关可点）、滑杆带范围 + 固定宽数值框 + 单位、单行省略说明（截断才悬停全文）。涉及 src/ui/icons.rs、src/ui/style.rs。
- 新增大图预览链路：LoadFull 命令优先读已下载文件、否则抓原图，解码上限 1600px，FullImageReady/FullImageFailed 事件回传。涉及 src/model.rs、src/download/preview.rs、src/download/queue.rs、src/app.rs。
- 修复解析结果无预览：解析出新增图片后立即后台抓取缩略图（mmbiz 用 /640、qlogo 用 /132 轻量变体，失败回退原图），全局 4 并发、单图 12MB/15s 空闲超时；下载完成后仍以处理后的图片覆盖。涉及 src/download/preview.rs、src/download/queue.rs、src/app.rs、src/model.rs。
- 修复缩略图纹理重复登记：预览与下载各产生一次缩略图时不再重复入 LRU，避免 512 上限被提前耗尽导致已加载纹理被误淘汰。

## 2026-11-07

- 新增外观主题：跟随系统 / 亮色 / 暗色，默认暗色；切换即时生效、持久化并同步原生标题栏。涉及 src/model.rs、src/ui/style.rs、src/ui/settings_panel.rs、src/app.rs。
- 状态色与类型徽标改为中间调，保证亮暗背景下的可读性。
- 新增开发调试脚本：dev.bat、check.bat、build.bat、probe.bat。
- 全项目执行 cargo fmt 统一格式。