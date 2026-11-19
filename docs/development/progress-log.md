# 开发进度记录

## 2026-11-08

### 下载目录自由设置
- 原实现只有文本修改 + 无父窗口的 rfd 选择器，路径无校验、失败无反馈。
- 新增 `set_download_root(path)`：`trim` → 去包裹引号 → 相对路径 `std::path::absolute` → `create_dir_all` → 写回 settings 并同步 actor；返回规范化路径，失败返回中文原因。
- 前端设置页：独立「保存目录」区（浏览… / 打开 / 输入框 + 应用），Enter 直接应用；应用按钮在未修改时禁用；行内小字显示「实际保存到 保存目录\文章标题\」，错误时转红并显示原因；省略内容悬停可见完整路径。
- 目录选择改用 `tauri-plugin-dialog`（模态、带父窗口），capabilities 增加 `dialog:default`；移除自建 `pick_directory` 命令与 rfd 直接依赖。
- 校验：`cargo fmt --check`、`clippy -D warnings`、`cargo test`（29 项）、`npm run build` 通过。

### Tauri 2 + Vue 3 迁移（方案 B：沉浸工作台）
- 决策：直接移除 egui；前端 Vue 3 + Vite + TS；色彩沿用 WeUI 官方 token，布局与质感按 Web 能力重做（深色优先）。
- 仓库：Cargo workspace = `crates/core` + `src-tauri`；Vue 工程在根级 `src/`。核心模块整体迁入 `crates/core/src`，`examples/probe_page.rs` 随迁，`examples/make_icon.rs` 新增。
- 解耦：`EventSink { tx: Sender<UiEvent>, wake: Arc<dyn Fn() + Send + Sync> }`；`queue::run` 改为 pub async，由 `tauri::async_runtime::spawn` 运行；移除需要 `tokio::runtime::Runtime` 的 `spawn`。
- 契约：`ParseBatch/ParseOutcome/QueueStats` 补 serde（PageKind 原已有）；前端 `src/types.ts` 手写镜像，字段保持 Rust snake_case；事件用 `type` 判别。
- 壳层线程模型：Tauri 主线程只做窗口；actor 在 tauri 的 tokio 运行时；事件桥独立 std::thread，`recv_timeout(250ms)`，PNG 编码在桥线程完成（image crate，png feature）。
- 图片通道：RGBA → PNG → 512 条 LRU；`register_uri_scheme_protocol("media")` 返回 `image/png`；前端 `convertFileSrc('thumb/…' | 'full/…', 'media')`。避免原始 RGBA 走 JSON IPC。
- 命令与副作用：rfd 目录选择走 `spawn_blocking`；打开目录/定位文件用 explorer；窗口主题 `set_theme` 与设置项联动；历史在桥线程 1s 防抖保存。
- 前端实现：`store.ts` 单例响应式镜像；`VirtualGrid` 行虚拟化；卡片单击选中/双击预览/右键菜单；`v-tip` 400ms 延迟 tooltip（替代 title）；Esc 关闭浮层；Ctrl+A 在非输入聚焦时全选；`prefers-reduced-motion` 全量降级。
- 图标：程序化 1024px 绘制圆角绿底 + 白色 Z，Lanczos 缩放并手写 ICO 容器（PNG 条目 16–256），输出到 `src-tauri/icons/`。
- 静态校验：`cargo fmt --check`、`cargo check --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（29 项）、`npm run build`（vue-tsc + vite）全部通过；debug 可执行文件链接成功。
- 未做：GUI 实机运行（按用户偏好不擅自启动）；`cargo tauri build` 安装包（需安装 tauri-cli）；旧 `weixin-images` 目录改名。

### 产品改名
- 定名「智爱媒体素材采集器」：品牌 + 媒体中立 + 功能中立，后续接入视频号/小红书/抖音无需再改名。
- 标识符：crate/exe `zhiai_media_collector`、eframe 应用 id 同名、窗口标题中文名；`build.bat` 产物路径、`examples/probe_page.rs` 引用、Cargo.lock 同步。
- 数据迁移：`storage::migrate_legacy_config()` 在 `DownloaderApp::new` 最先调用，旧 `%APPDATA%\wechat_image_downloader` 整体 rename 到新目录；目标已存在直接跳过，rename 失败则建目录并逐个复制 settings.json/history.json。已下载文件与用户自定义 `download_root` 不迁移、不改写。
- 默认下载目录 `Pictures\WeChatImages` → `Pictures\ZhiaiCollect`（仅影响全新安装；老用户 settings.json 里的旧路径继续生效）。
- 设置页顶部新增品牌行（智爱标记 + 名称 + 单行副标题 + 版本号）。副标题只描述已实现的公众号采集，未宣称视频号/小红书/抖音。
- 项目目录名 `weixin-images` 未改动（文件系统操作待用户单独确认）。
- 校验：`cargo check --all-targets` 通过（Cargo.lock 已刷新为新 crate 名）。

### 微信 WeUI 视觉与布局重构（方案 A）
- 色彩权威取自 Tencent/weui 源码 `src/style/base/theme/vars/{light,dark}.less`，亮暗两套 token 全部写入 `style::palette`，通过 `ctx.all_styles_mut` 按 `visuals.dark_mode` 分别套用；主题切换（跟随系统/亮/暗）与原生标题栏同步保持可用。
- 扁平化落实：8px 圆角、`Shadow::NONE`（窗口与浮层）、发丝分割线、无 hover 扩张（`expansion = 0`）、悬停只用中性亮度变化，强调绿仅用于主操作、选中态与完成状态。
- 布局：`Panel::left` 固定 56px 导航 + `CentralPanel` 整页路由（`Page::Images/History/Settings`）；图片页内用 `Panel::bottom` 承载可折叠队列条。设置/历史不再常驻占位。
- 控件统一：自绘 `button`（主/次、32/26px、可选描边图标）、`chip`、`switch`（仅控件本身可点）、`slider_cell`（范围 + 固定宽数值框 + 单位）、`number_cell`、`cells/cell_row/cell_note`（WeUI 分组单元格）、`note`（单行省略，截断才悬停全文）。
- 图标：新增 `ui/icons.rs`，16×16 归一化坐标、1.7 描边、线段端点补圆头、容器内绝对居中；覆盖导航/解析/清空/暂停/继续/重试/清除/复制/下载/目录/折叠/勾选。
- 交互：卡片单击选中、双击大图、右键 `context_menu`；Enter 解析（单行输入失焦触发）、Ctrl+A 全选（输入框聚焦时让位给文本全选）、Esc 关闭预览；队列折叠状态 `Option<bool>`，None 为自动（有排队/进行中即展开），手动切换后记住。
- 大图预览：`QueueCommand::LoadFull` → 优先读已落盘文件，否则抓原图（64MB 上限）→ `imaging::thumbnail(1600)` → `FullImageReady`；失败回 `FullImageFailed` 并在窗口内显示中文原因。
- 校验：`cargo fmt --check`、`cargo check --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（29 项）全部通过；GUI 实机观感待运行确认。

### 解析预览
- 原缩略图只在下载完成后生成，解析后的网格卡片仅显示类型占位。
- 新增 download/preview.rs：解析新增项即抓取轻量变体（mmbiz /640、qlogo /132），解码失败或无该变体时回退原图；4 并发、12MB 上限、15s 空闲超时，失败静默保留占位。
- queue actor 新增 `Preview` 命令与信号量；app 仅对去重后真正新增的图片下发，避免重复抓取。
- install_thumbnail 去重入 LRU，防止预览+下载两次缩略图使 512 上限提前触发淘汰。
- `cargo check --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（29 项）通过；GUI 实测待运行验证。

## 2026-11-07

### M0 项目骨架
- `cargo init --vcs none`，edition 2024，建立 lib + bin 双目标（examples 侦察探针需要库目标）。
- 依赖问题与修复：
  - reqwest 0.13 移除了 `rustls-tls` feature → 改用 `rustls`（aws-lc-rs + 平台证书验证）。
  - egui/eframe 0.37 要求 rustc 1.95（本机 1.94）→ 固定 0.35（MSRV 1.92），不升级全局工具链。
  - webp 0.3.1 默认 `img` feature 使用 image ^0.25，与主依赖类型统一。
- 首次 `cargo check` 验证 libwebp-sys 0.9.6（cc 编译）与 aws-lc-sys 均可在本机 MSVC 构建。

### M1 链接与规范化
- link.rs：多行/混合文本抽取、host 精确校验（修复子串误判）、文章/合集判定。
- normalize.rs：JSON/HTML 反转义、尺寸段→0、去 query、host+path 去重键、格式提示、媒体 hash。

### M2 抓取与文章提取
- fetch.rs：统一 UA/Referer、30s 超时、验证页与 401/403 识别。
- article.rs：og:image/msg_cdn_url 封面、#js_content 正文（data-src/data-croporisrc/src + 背景图）、qlogo 头像、二维码区（含 /mp/qrcode 端点）、标题与账号名。
- fallback.rs：mmbiz+qlogo 全响应扫描（含转义、尺寸变体去重）。
- extract 编排：页面类型判定 + 结构化优先 + 兜底补齐 + 全局去重。

### M3 下载核心
- queue.rs：单 actor；并发 active/limit、软暂停、取消 abort、重试退避、完成通道、统计事件。
- task.rs：流式下载（20s 空闲超时、64MB 上限、100ms 进度节流）、嗅探→转换→原子写、历史与缩略图。
- imaging.rs：guess_format、JPG 透明底合成白底、WebP 有损（libwebp）、GIF/动画 WebP 保护、缩略图。
- storage.rs：原子写、损坏隔离、Windows 非法字符与保留名清洗。

### M4 UI 集成
- egui 0.35 实际 API：`App::ui(&mut Ui)` + `Panel::top/bottom/right`（不再是 `update(&Context)` 与 `TopBottomPanel`）。
- 面板：输入、虚拟滚动网格（行虚拟化 + 纹理 LRU 512）、队列、设置、历史窗口。
- 中文字体：msyh.ttc → msyh.ttf → simhei.ttf → simsun.ttc 依次注入（FontData 支持集合 index）。
- 跨批次 URL 去重，避免重复下载。

### M5 图片消息与合集
- album.rs：__biz/album_id（URL 或 HTML 变量）→ getalbum 分页（count=10、continue_flag、500ms 延迟、50 页上限）→ HTML 链接兜底；UI 确认后逐篇解析。
- examples/probe_page.rs：结构侦察探针（标记检测、可识别图片、结构变量行）。
- 图片消息结构化提取待用户提供真实样本后按实测结构补充；当前由兜底扫描完整覆盖下载。

### 验证状态
- `cargo check` 通过。
- `cargo clippy --all-targets -- -D warnings` 通过（含测试与示例）。
- `cargo test`：29 个单元测试全部通过（修复 1 处测试夹具的 RIFF 布局错误，实现本身正确）。
- GUI 冒烟（Windows 桌面实测）：
  - 发现并修复面板布局问题：设置面板内下载目录输入框使用 `desired_width(f32::INFINITY)`，导致右侧面板按内容定宽并逐帧向左膨胀；底部面板按内容塌缩为 72px。
  - 修复：输入框改为有限宽度并可悬停查看完整路径；右侧面板 `min_size 280 / max_size 480`，底部面板 `min_size 180 / max_size 400`；筛选 chips 与操作按钮改为两行自适应布局。
  - 复验：设置面板位于右侧、全部筛选与操作按钮可见、队列面板正常、中文（msyh.ttc）渲染正常。
- 待办：真实链接网络实测（解析/下载/转换/合集）需用户提供样本；图片消息结构化提取待样本侦察。

### 开发脚本
- 新增 dev.bat / check.bat / build.bat / probe.bat，并在全项目执行 `cargo fmt` 统一格式。
- 批处理写入中文提示时，cmd 在 GBK 代码页下解析 UTF-8 字节会破坏语法（check.bat 曾出现命令片段错误）；已将 dev/check/build 改为 ASCII 提示，probe.bat 保留 `chcp 65001` 以正确回显 UTF-8 侦察结果。
- check.bat 实测退出码 0，29 个测试全部通过。

### 外观主题
- 新增 ThemeMode（跟随系统/亮色/暗色），默认暗色；设置持久化，启动与切换时同步更新 egui 主题与原生标题栏（ViewportCommand::SetTheme）。
- 状态色与类型徽标统一调整为中间调，兼顾亮暗背景对比度。
- 实测三种模式：暗色、亮色、跟随系统（本机系统为暗色）渲染均正常，亮色下标题栏同步为浅色。