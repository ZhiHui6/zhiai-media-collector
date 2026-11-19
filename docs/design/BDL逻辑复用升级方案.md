# 智爱媒体素材采集器 · 基于 BDL 的逻辑复用与功能升级方案

| 项    | 内容                                                                     |
| ---- | ---------------------------------------------------------------------- |
| 方案日期 | 2026-11-19                                                             |
| 复用来源 | `C:/Users/Administrator/Desktop/bdl-main`（BDL · Bilibili Download Lab） |
| 目标项目 | `E:/我的开发软件/weixin-images`（智爱媒体素材采集器 `zhiai_media_collector`）           |
| 来源协议 | MIT License（Copyright (c) 2026 YUELI）——可商用、可修改，复用须保留版权与许可声明            |
| 交付形态 | 本 MD 文档 = 升级蓝图；代码落地按第六节分阶段推进                                           |

---

## 〇、结论先行（TL;DR）

**能抄，而且高度匹配。** 两个项目是同一类软件的不同垂类：都是「Rust 核心 + Tauri 壳 + 事件驱动前端」的本地优先下载/采集器。BDL 是 B 站视频下载器，我方是微信图文采集器，**架构骨架几乎一一对应**，因此 BDL 里「与具体平台无关的下载管线工程逻辑」可以低成本平移，「平台强相关逻辑」必须重写或不用。

**最高价值的 5 个可抄点（按投入产出比排序）：**

| # | 可抄逻辑                                                     | 我方现状                                        | 升级收益                      | 改动量 |
| - | -------------------------------------------------------- | ------------------------------------------- | ------------------------- | --- |
| 1 | 统一命令错误边界 + 脱敏（`CommandError{code,message,detail}`）       | 命令返回 `Result<_, String>` / `Value`，无错误码、无脱敏 | 前端可按 code 精确提示，日志不泄Cookie | 小   |
| 2 | 数据驱动失败分类（规则表 + `classify_failure` + 建议动作）                | 验证页只给一句固定文案                                 | 按错误给可执行指引，直达"环境异常"痛点      | 中   |
| 3 | 设置版本迁移（`schema_version` + `normalized()` + `validate()`） | 仅 `serde(default)`，无版本/校验收口                 | 后续加字段不炸老用户配置              | 小   |
| 4 | 任务持久化 + 崩溃恢复（SQLite WAL + 增量 diff + 启动恢复）                | 队列纯内存，退出即丢                                  | 采集任务可恢复，体验质变              | 大   |
| 5 | 命名冲突策略 + 预留集合（`Skip/Append/Overwrite`、`ReservedPaths`）   | 按序号+hash 命名，无磁盘冲突策略                         | 重复采集同一批链接可正确跳过/覆盖         | 中   |

**明确不要抄的：** 账号/扫码登录、凭证安全存储、B 站风控码绕过、FFmpeg 封装/NFO/弹幕/字幕、Android 适配——这些要么被我方 PRD 禁止（"不登录微信、不破解验证"），要么与图片采集无关。详见第五节"不适用清单"。

---

## 一、复用路径说明（为什么是"项目内逻辑复用"而非找现成轮子）

按复用检索 SOP，正确顺序是「生态内 skill/MCP（零开发）→ 全网开源 → 分级筛 → 自研」。本次的特殊性在于：**用户已指定复用来源（bdl-main），且需求是"借鉴逻辑设计"而非"接一个外部能力"**。

- 生态内 skill/MCP：本需求是**本地代码逻辑迁移与适配**，不是接入外部服务/API/数据源，无对应可安装 skill 或 MCP 连接器，故不走生态安装路径。
- 全网开源：用户给的 bdl-main 本身就是一份 MIT 许可、活跃维护、可直接读源码的高质量参考实现，**优于再去搜同类轮子**。
- 因此本次采用 **「指定项目内逻辑复用 → 按适配难度分级 → 给出最小落地步骤」** 路径。这是我方生态原则（优先复用已验证实现）在"已有明确复用源"场景下的正确落点。

**行业背景**：下载/采集类桌面软件的工程难点高度同质——断点续传、限速、重试分类、崩溃恢复、命名冲突、限流熔断、事件节流。成熟做法是把这些做成 UI 无关的核心层纯逻辑 + 可测试契约，UI 壳只消费事件。BDL 正是这一惯例的教科书式实现，我方架构已具备同样的分层前提，所以是"换内容不换骨架"的移植，而非重写。

---

## 二、双方画像与同构性分析

| 维度    | BDL（来源）                                | 智爱采集器（目标）                      | 同构度       |
| ----- | -------------------------------------- | ------------------------------ | --------- |
| 语言/版本 | Rust 2024 / 1.88+                      | Rust（MSVC）/ 1.94               | 高         |
| 桌面壳   | Tauri 2                                | Tauri 2                        | 完全一致      |
| 前端    | Nuxt/Vue + 本地 wrapper                  | Vue 3 + Vite + TS              | 高         |
| 核心层   | `bdl-core`（UI 无关）                      | `zhiai_core`（UI 无关）            | 完全一致      |
| 壳层    | `bdl-tauri`                            | `src-tauri`                    | 一致        |
| 状态模型  | `AppState` 多字段细粒度锁                     | `AppState` 多字段 Mutex           | 一致（我方更精简） |
| 事件同步  | `domain://action` 常量 + 全量快照            | 单频道 `core://event` + `type` 判别 | 中（我方更粗）   |
| 命令层   | 薄包装 + DTO + `CommandError`             | 薄包装，无 DTO/统一错误                 | 中（我方偏弱）   |
| 任务队列  | SQLite 持久化 + worker + 恢复               | 内存单 actor，退出即丢                 | 低（最大短板）   |
| 下载管线  | 断点续传/分段/限速/多URL回退                      | 整块下载（图）+ 流式（视频，无续传）            | 低（主要增强点）  |
| 持久化   | SQLite WAL + 迁移                        | JSON（settings/history）+ 原子写    | 中         |
| 配置迁移  | schema_version + normalized + validate | `serde(default)` + 目录名迁移       | 中         |
| 诊断    | 失败分类规则表 + 脱敏导出                         | 无                              | 低（空白）     |
| 平台    | 桌面 + Android                           | 仅 Windows 桌面                   | 不适用部分     |

**结论**：骨架同构 → 工程逻辑可平移；差异集中在"任务持久化/下载管线/诊断"三块，正是本次升级主战场。我方已有的两项实现甚至优于来源，应保留：**① 指数退避重试（1/2/4s）——BDL 反而没有；② 事件进度 100ms 节流——比 BDL 的 200ms 更积极；③ 原子写盘 + 损坏 `.bad` 回退——已具备。**

---

## 三、可借鉴逻辑设计全景对比表

> 类型列：`逻辑`=项目内可移植的纯逻辑设计。链接列给出来源文件路径（相对 bdl-main 根目录）。推荐度按"成熟度 × 适配收益 ÷ 改动量"评估。已知坑来自源码/README/设计文档。

| 方案（设计点）                                           | 类型 | 链接（来源文件）                                                                        | 维护·口碑      | License | 复用成本 | 已知坑                             | 推荐度    |
| ------------------------------------------------- | -- | ------------------------------------------------------------------------------- | ---------- | ------- | ---- | ------------------------------- | ------ |
| 统一命令错误边界 `CommandError{code,message,detail}` + 脱敏 | 逻辑 | `crates/bdl-tauri/src/commands.rs`                                              | 活跃维护，多版本迭代 | MIT     | 低    | 需自建微信场景 code 表                  | ★★★★★  |
| 数据驱动失败分类 + 建议动作（规则表 JSON 编译期嵌入）                   | 逻辑 | `crates/bdl-core/src/diagnostics.rs`                                            | 活跃，含测试     | MIT     | 中    | 需重写规则表；注意"尝试边界"归属               | ★★★★★  |
| 设置 schema_version + `normalized()` + `validate()` | 逻辑 | `crates/bdl-core/src/settings.rs`                                               | 活跃，含迁移测试   | MIT     | 低    | 迁移逻辑需随版本维护                      | ★★★★★  |
| 任务持久化 SQLite WAL + 增量 diff 写 + 启动崩溃恢复             | 逻辑 | `crates/bdl-core/src/storage.rs`、`crates/bdl-tauri/src/queue_coordinator.rs`    | 活跃，契约测试充分  | MIT     | 高    | 引入 rusqlite 依赖；迁移需谨慎            | ★★★★★  |
| 命名模板 + 净化 + UTF-8 字节预算 + 三种冲突策略 + 预留集合            | 逻辑 | `crates/bdl-core/src/naming.rs`                                                 | 活跃，多测试     | MIT     | 中    | 超大相册下计数上限需评估                    | ★★★★☆  |
| 断点续传（`.part` + ETag/Last-Modified 校验 + Range）     | 逻辑 | `crates/bdl-core/src/fetcher.rs`                                                | 活跃，假服务器测试  | MIT     | 中    | 微信 CDN 可能不返回 Length/ETag，会退化为整下 | ★★★★☆  |
| 令牌桶限速（全局 + 单任务双层、可热更新、可取消）                        | 逻辑 | `crates/bdl-core/src/fetcher.rs` + `tests/bandwidth_limiter.rs`                 | 活跃，虚拟时间测试  | MIT     | 低    | 需与取消令牌配合 select                 | ★★★★☆  |
| 完整性魔数校验（完成文件按 magic bytes 判损坏再决定是否重下）             | 逻辑 | `crates/bdl-core/src/fetcher.rs`                                                | 活跃         | MIT     | 低    | 需为图片定义魔数白名单                     | ★★★★☆  |
| 解析节流 + 风控熔断（进程内闸门 + 尊重 Retry-After）               | 逻辑 | `crates/bdl-core/src/resolver/pacing.rs`、`crates/bdl-tauri/src/parse_pacing.rs` | 活跃         | MIT     | 中    | B 站风控码不适用，改判微信验证页               | ★★★★☆  |
| 事件主题细分 + 统一 `emit()` 包装（失败转 CommandError）         | 逻辑 | `crates/bdl-tauri/src/events.rs`                                                | 活跃         | MIT     | 低    | 我方已有 100ms 节流，别重复造              | ★★★☆☆  |
| 强类型 ID newtype（透明序列化杜绝串号）                         | 逻辑 | `crates/bdl-core/src/ids.rs`                                                    | 活跃         | MIT     | 低    | 我方 ID 简单，收益有限                   | ★★★☆☆  |
| 后台准备作业会话（长任务从 RPC 解耦、可取消可恢复）                      | 逻辑 | `crates/bdl-tauri/src/preparation.rs`                                           | 活跃         | MIT     | 中高   | 合集批量场景才需要                       | ★★★☆☆  |
| 文件删除安全协议（预览令牌 + 文件身份二次校验 + 链接校验）                  | 逻辑 | `crates/bdl-tauri/src/file_removal.rs`                                          | 活跃，契约测试    | MIT     | 中高   | 可裁剪，至少保留 stamp 二次校验             | ★★★☆☆  |
| 故障注入契约测试（TRIGGER 注入 DB 失败、Condvar 造并发窗口）          | 逻辑 | `crates/bdl-tauri/tests/core_contracts.rs`                                      | 活跃         | MIT     | 中    | 需先有 SQLite 层                    | ★★★☆☆  |
| 账号/扫码登录 + 凭证安全存储（keyring/加密文件）                    | 逻辑 | `crates/bdl-tauri/src/secure_store.rs`                                          | 活跃         | MIT     | 高    | **PRD 禁止登录微信——不适用**             | ☆（不采用） |
| B 站风控码绕过 / HTTP 跨进程闸门（-352/-412 等）                | 逻辑 | `crates/bdl-core/src/resolver/http_guard.rs`                                    | 活跃         | MIT     | 高    | **PRD 禁止破解验证——仅借节流思想**          | ☆（不采用） |
| FFmpeg 封装 / NFO / 弹幕 / 字幕                         | 逻辑 | `muxer.rs`/`nfo.rs`/`danmaku.rs`/`subtitles.rs`                                 | 活跃         | MIT     | 高    | 图片采集无关——不适用                     | ☆（不采用） |
| Android 多端适配                                      | 逻辑 | `crates/bdl-mobile-*`                                                           | 活跃         | MIT     | 高    | 我方仅 Windows 桌面——不适用             | ☆（不采用） |

---

## 四、逐项适配设计（可抄点的落地方案）

> 每一项结构：BDL 怎么做 → 我方现状 → 适配设计 → 落地文件 → 验证点 → 已知坑。

### P0-1 统一命令错误边界 + 脱敏

- **BDL 怎么做**：`commands.rs` 定义 `CommandError{code, message, detail}`，`From<BdlError>` 把内部错误映射为稳定前端码（`network_timeout`、`file_access_denied`…），`message` 经 `redact_sensitive()` 给用户，`detail` 留脱敏堆栈；`#[serde(skip_serializing_if)]` 去掉空字段；批量操作用 `BulkQueueResult{updated,removed,failed}` 表达部分成功。
- **我方现状**：`commands.rs` 里 `set_download_root` 返回 `Result<String,String>`、`apply_theme` 返回 `Result<(),String>`，其余多为 `()`；错误直接是中文字符串，前端无法按类型处理，也无脱敏。
- **适配设计**：在 `src-tauri` 新增 `error.rs` 定义 `CollectorsError` 与 `CommandError`，把 `DownloadError/FetchError/StorageError/ImagingError` 统一收敛；新增 `redact()`（对 URL query、可能的 Cookie/token 头打码）；命令返回 `Result<T, CommandError>`。前端 `types.ts` 补 `CommandError`，`api.ts` 统一拦截并按 `code` 映射到现有中文文案。
- **落地文件**：新增 `src-tauri/src/error.rs`；改 `src-tauri/src/commands.rs`、`lib.rs`、`src/api.ts`、`src/types.ts`。
- **验证点**：构造一个带 query 参数的失败 URL，断言前端收到的 `detail` 中参数被打码；命令失败时前端按 code 显示对应提示而非原始串。
- **已知坑**：错误码表要在 Rust 与 TS 两端同步（建议生成一个常量文件或测试比对）。

### P0-2 数据驱动失败分类 + 建议动作

- **BDL 怎么做**：`diagnostics.rs` 用 `include_str!` 编译期嵌入 `diagnostic-rules.json`，`OnceLock` 懒加载；`classify_failure` 按规则表匹配给出 `RecommendedAction`；`explicit_failure` 优先解析消息里的 `code=xxx]` 标记；`latest_failure` 按时间倒序，遇到"重试任务/开始下载"日志立即返回 None，防止上一次错误算到新尝试。
- **我方现状**：`fetch.rs::is_verification_page()` 只能识别验证页，`DownloadError::retryable()` 只区分网络/429/5xx；用户遇到"环境异常"只看到一句固定文案，无分级建议。
- **适配设计**：新增本地规则表（可先用 Rust 常量数组起步，不必上 JSON），覆盖我方高频失败：验证页提示"在浏览器打开链接完成验证"、401/403"链接可能过期或需验证"、429"请求过快，降低并发"、磁盘满"清理下载目录"、文件被占用"关闭预览后重试"。给 `UiEvent::ItemStateChanged` 增加可选 `action` 字段，前端展示"建议：…"。**关键**：按 BDL 经验，重试/新尝试要重置"当前错误"，避免旧建议残留。
- **落地文件**：新增 `crates/core/src/diagnostics.rs`；改 `model.rs`（UiEvent）、`error.rs`、`download/queue.rs`、`src/types.ts`、`src/components/*`。
- **验证点**：单元测试覆盖每条规则匹配 + "新尝试不继承旧错误"；UI 验证验证页场景显示可操作建议。
- **已知坑**：规则匹配顺序（显式 code 优先于关键词），否则会误判。

### P0-3 设置版本迁移

- **BDL 怎么做**：`AppSettings` 全字段 `#[serde(default)]`，`settings_schema_version` 显式版本；`normalized()` 把 v1→v2（如按 proxy_url 推导 proxy_mode、非法值归一、空预设注入内置预设、老扁平配置转新预设，匹配不上存"migrated/历史配置"）；`validate()` 收口交叉字段校验。
- **我方现状**：`Settings` 已有 `#[serde(default)]`（老配置缺字段不炸），且有配置目录名迁移；但无 schema_version，新增字段的语义迁移/废弃字段清理无统一入口。
- **适配设计**：给 `Settings` 加 `schema_version: u32`（默认当前版本）；加 `normalized(self) -> Settings` 与 `validate(self) -> Result<Self, String>`；`load_settings()` 里 `serde → normalized → validate`。把 `set_download_root` 里的路径校验逻辑收进 `validate()`。
- **落地文件**：改 `crates/core/src/model.rs`、`storage.rs`、`src-tauri/src/commands.rs`。
- **验证点**：测试"老版本 JSON（无 schema_version）加载后自动补默认值并规范化"；非法并发值的归一。
- **已知坑**：迁移要幂等，重复加载不产生副作用。

### P0-4 任务持久化 + 崩溃恢复（最大工程项）

- **BDL 怎么做**：`storage.rs` 用 SQLite WAL + 事务，`apply_queue_changes(prev,next)` 按 id 做增量 diff（只删消失的、只更新变化的），`UPDATE ... WHERE id=? AND task_id=?` 要求 changed==1 做并发写保护；`add_column_if_missing` 增量迁移不重建表；`queue_coordinator.rs::prepare_startup_recovery()` 把中断态任务重置为 Waiting/Paused，只重置未完成资源（保留已完成，不全量重下）；`state.rs::QueueEdit` 写时复制，**先落库成功后才提交内存**（内存是 DB 的投影）。
- **我方现状**：`queue.rs::Actor` 用 `HashMap<u64, Entry>` 纯内存，`ItemState` 有 Queued/Downloading/Completed/Failed/Cancelled/Skipped；进程退出全部丢失，重启后前端 `store.views` 也是空的。历史（HistoryEntry）有 JSON 持久化，但队列状态没有。
- **适配设计**（建议拆成两个子阶段）：
  - **4a 轻量版（推荐先做）**：不引入 SQLite，复用现有 JSON 原子写机制，新增 `tasks.json` 持久化队列快照（id/item/state/attempts/path），在状态变更时防抖落盘（仿历史 1s 防抖）；启动时加载并把 Downloading 态重置为 Queued 供用户确认重试。改动小，先解决"退出即丢"。
  - **4b 完整版（可选）**：引入 `rusqlite(bundled)`，建 `tasks` 表，实现事务 + 增量 diff + 增量列迁移 + 故障注入契约测试。收益是并发写安全与大规模任务可靠。
  - 无论哪版，都遵循"**先持久化、后改内存、再发事件**"的不变量。
- **落地文件**：改 `crates/core/src/download/queue.rs`、`model.rs`、`storage.rs`；`src-tauri/src/lib.rs`（启动加载）、`bridge.rs`（防抖落盘）；`src/store.ts`（boot 时回填队列）。
- **验证点**：下载中强杀进程 → 重启后任务以 Queued 出现且可重试；已完成的不重复下载；写盘失败时内存状态不变。
- **已知坑**：ID 稳定性（我方 item.id 是 u64 自增，重启后需保证不冲突，建议改为内容 hash 派生或持久化自增游标）；快照里不要存原始 RGBA。

### P0-5 命名冲突策略 + 预留集合

- **BDL 怎么做**：`naming.rs` 的 `DuplicateNamingStrategy{SkipExisting,AppendSuffix,OverwriteExisting}` + `ReservedPaths`（Windows 下路径 `to_uppercase()` 做大小写不敏感判重）；`render_output_path` 模板渲染 + `sanitize_path_component` 净化 + `utf8_prefix` 按 UTF-8 字符边界截到 255 字节 + 为计数后缀预留字节；`reserve_task_paths` 区分 `live_collision`（队列内）与 `disk_collision`（磁盘已有）。
- **我方现状**：`storage.rs` 已有 `sanitize_component()`（非法字符替换、尾点空格裁剪、Windows 保留名转义、按字符截断）+ `file_name(seq,kind,hash,ext)`；缺少"磁盘已存在同名文件"的处置策略和批量内预留去重。
- **适配设计**：新增 `DuplicatePolicy` 枚举与设置项（默认 `AppendSuffix`，对齐"不覆盖用户文件"的安全默认）；在 `download/task.rs` 落盘前检查目标路径：Skip→不创建任务并标记 Skipped；Append→加 `_1/_2`；Overwrite→直接覆盖。用 `ReservedPaths` 在 `Enqueue` 阶段一次性预留同批路径，避免批内撞车。
- **落地文件**：改 `crates/core/src/storage.rs`、`model.rs`（Settings/ItemState）、`download/task.rs`、`download/queue.rs`、`src/components/SettingsSheet.vue`。
- **验证点**：同一链接重复采集 + 磁盘已存在同名文件，三种策略各自行为正确；中文长标题按字节安全截断；Windows 保留名被转义。
- **已知坑**：我方 `file_name` 已含 hash，同名概率低，但"重复采集同一链接"仍会生成相同路径——这是本项主要价值场景。


### P1-1 令牌桶限速 + 完整性魔数校验

- **BDL 怎么做**：`BandwidthLimiter` 用 `AtomicU64` 存速率、令牌桶可热更新、不足时短睡（≤0.1s）以便响应取消；全局+任务双层 `join!`；`completed_media_needs_redownload` 读文件前 16 字节按魔数判断媒体是否损坏，坏则重下。
- **我方现状**：无限速；`task.rs` 的 `already_present` 只比较文件长度，长度相同但内容损坏会被当成成功复用。
- **适配设计**：给 `Settings` 加 `speed_limit_kbps`（0=不限）；核心加 `BandwidthLimiter`，在 `download()`/`write_video()` 的 chunk 循环里按字节 `acquire()`，与 60s/20s 超时 `select!`；把 `already_present` 升级为"长度 + 魔数"双校验（复用 `imaging.rs` 的格式嗅探，JPEG `FFD8`、PNG `89504E47`、WEBP `RIFF....WEBP`、GIF `GIF8`）。
- **落地文件**：改 `crates/core/src/download/task.rs`、`imaging.rs`、`model.rs`、`src/components/SettingsSheet.vue`、`src/types.ts`。
- **验证点**：虚拟时间测试限速生效；坏文件（长度对、头错）被识别并重下。
- **已知坑**：限速 acquire 必须能被取消打断，否则暂停无响应。

### P1-2 视频断点续传

- **BDL 怎么做**：`fetcher.rs` 用 `.bdlpart` + `.state` 文件（原子写），`resume_offset` 仅在 "总长一致 ∧ 已下长度一致 ∧ ETag/Last-Modified 匹配" 时续传，否则从头下；续传强制 206。
- **我方现状**：`run_video` 流式写 `.part`，失败即 `remove_file` 重来；`MAX_VIDEO_BYTES` 高达 4GB，无续传体验差。
- **适配设计**：仅为视频路径（`ImageKind::Video`）加断点续传：`.part` 保留不删；请求带 `Range: bytes=<len>-`，接受 206 续写、200 则从头；用 `ETag/Last-Modified` 做校验，缺失则退化为从头。图片路径保持整块下载不变（小文件，无需分段）。
- **落地文件**：改 `crates/core/src/download/task.rs`（`run_video`/`write_video`）。
- **验证点**：假服务器测试"中断后续传只请求剩余字节"；服务端内容变更时从头下载。
- **已知坑**：微信视频 CDN 可能忽略 Range，必须优雅退化。

### P1-3 解析节流 + 验证熔断

- **BDL 怎么做**：`pacing.rs`/`parse_pacing.rs` 用单闸门串行化请求，间隔 + 每 N 页长休息，命中频率限制设 `blocked_until` 且**不重试**，尊重 `Retry-After`。
- **我方现状**：合集枚举有 500ms 延迟与 50 页上限（已有基础节流），但批量解析多篇文章无统一节流；命中验证页后仍逐条重试，容易加剧限制。
- **适配设计**：新增 `ResolveGate`（进程内 `tokio::Mutex` 串行 + 可配置间隔），在 `QueueCommand::ParseText` 的批量循环中经过闸门；命中 `is_verification_page()` 时设置短时间熔断（如 60s 内不再发请求），并给前端一个明确状态。**不照抄 B 站风控码**，只复用"闸门 + 熔断 + 尊重 Retry-After"思想。
- **落地文件**：改 `crates/core/src/download/queue.rs`、`fetch.rs`、`model.rs`（Settings 加节流参数）。
- **验证点**：虚拟时间测试批量解析按间隔发出；命中验证页后熔断期不再请求。
- **已知坑**：熔断要有时限与恢复路径，避免永久卡死。

### P2 其余可选项

- **事件主题细分**：仿 `events.rs` 把单频道 `core://event` 拆为 `parse://`、`queue://`、`settings://` 等常量 + 统一 `emit()` 失败转错误。**前提**：我方进度已有 100ms 节流，勿重复实现，只改主题组织。
- **后台准备作业**：合集批量解析若是长任务，仿 `preparation.rs` 注册作业即时返回 + 后台执行 + 可取消，避免 UI 卡住。
- **文件删除安全协议**：`library_delete` 目前直接 `remove_file` 且按路径删。至少补三点：删除前 `FileStamp{size,modified}` 二次校验、拒绝符号链接/目录联接的祖先路径、保护其他任务仍引用的文件。**这与个人文件安全强相关，优先级可视用户数据敏感度上调。**
- **强类型 ID**：仿 `ids.rs` 用透明 newtype 替代裸 `u64`，杜绝 id/path_id 串用（收益有限，可最后做）。

### 不适用清单（明确排除）

| 来源逻辑                                                           | 排除理由                               |
| -------------------------------------------------------------- | ---------------------------------- |
| `secure_store.rs` / `account.rs` / 扫码登录                        | PRD 明确"不登录微信、不抓取历史消息"              |
| `http_guard.rs` 的 B 站风控码（-352/-412/-509/-799）                  | PRD 明确"不破解验证"；只借节流/熔断思想，用微信验证页识别替代 |
| `muxer.rs` / `nfo.rs` / `danmaku.rs` / `subtitles.rs` / FFmpeg | 图片采集不需要音视频封装与弹幕字幕                  |
| `crates/bdl-mobile-*` / Android 适配                             | 我方仅 Windows 桌面                     |
| 分段下载（8 段并发）                                                    | 以图片为主，小文件分段无收益；视频用简单续传即可           |

---

## 五、关键代码骨架（落地参考，非最终代码）

> 以下为设计示意，落地时需结合现有类型调整。

**① 统一错误边界（`src-tauri/src/error.rs`）**

```rust
#[derive(Debug, serde::Serialize)]
pub struct CommandError {
    pub code: String,        // 稳定前端码：network_timeout / verification / disk_full ...
    pub message: String,     // 脱敏后、可直接展示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
type CommandResult<T> = Result<T, CommandError>;

impl From<zhiai_core::error::DownloadError> for CommandError {
    fn from(e: zhiai_core::error::DownloadError) -> Self {
        let code = match &e {
            DownloadError::Verification => "verification",
            DownloadError::Timeout => "network_timeout",
            DownloadError::Io(_) => "file_access",
            _ => "download_failed",
        };
        Self { code: code.into(), message: redact(&e.to_string()), detail: None }
    }
}
```

**② 失败诊断规则表（`crates/core/src/diagnostics.rs`）**

```rust
pub struct FailureRule { pub match_code: &'static str, pub contains: &'static [&'static str], pub action: &'static str }

const RULES: &[FailureRule] = &[
    FailureRule { match_code: "verification", contains: &[], action: "请在浏览器打开链接完成验证后重试" },
    FailureRule { match_code: "", contains: &["429"], action: "请求过快，请在设置中降低并发" },
    FailureRule { match_code: "", contains: &["磁盘空间", "os error 28"], action: "请清理下载目录后重试" },
];

pub fn classify(code: Option<&str>, message: &str) -> Option<&'static str> { /* 先 code 后关键词 */ }
```

**③ 命名冲突 + 预留集合（`crates/core/src/storage.rs`）**

```rust
pub enum DuplicatePolicy { Skip, AppendSuffix, Overwrite }

pub fn resolve_path(dir: &Path, base: &str, ext: &str, policy: DuplicatePolicy, reserved: &mut HashSet<String>) -> Option<PathBuf> {
    let candidate = dir.join(format!("{base}.{ext}"));
    let key = candidate.to_string_lossy().to_uppercase(); // Windows 大小写不敏感
    match policy {
        DuplicatePolicy::Skip if candidate.exists() || reserved.contains(&key) => None,
        DuplicatePolicy::Overwrite => Some(candidate),
        DuplicatePolicy::AppendSuffix => {
            for n in 1..=10000 {
                let p = dir.join(format!("{base}_{n}.{ext}"));
                let k = p.to_string_lossy().to_uppercase();
                if !p.exists() && !reserved.contains(&k) { reserved.insert(k); return Some(p); }
            }
            None
        }
        _ => { reserved.insert(key); Some(candidate) }
    }
}
```

**④ 令牌桶限速骨架（`crates/core/src/download/limiter.rs`）**

```rust
pub struct BandwidthLimiter { rate: AtomicU64, state: Mutex<LimiterState> }
impl BandwidthLimiter {
    pub async fn acquire(&self, bytes: u64) { /* 令牌不足则 sleep(min(wait,100ms))，可被取消打断 */ }
    pub fn set_rate(&self, bytes_per_sec: u64) { /* 热更新并重置桶 */ }
}
// 使用：tokio::join!(global_limiter.acquire(n), task_limiter.acquire(n));
```

---

## 六、分阶段实施路线图

| 阶段            | 内容                                 | 关键交付             | 验证标准                 |
| ------------- | ---------------------------------- | ---------------- | -------------------- |
| **S1（低风险快赢）** | P0-1 错误边界 + P0-3 设置迁移              | 统一 code、脱敏、配置版本化 | 相关单元测试 + 前端按 code 提示 |
| **S2（体验痛点）**  | P0-2 失败诊断分类                        | 规则表 + 建议动作       | 验证页/429/磁盘满各有指引，不串错误 |
| **S3（数据可靠）**  | P0-4a 轻量任务持久化（JSON 快照）+ P0-5 命名策略  | 队列可恢复、冲突可配置      | 强杀重启可恢复；三种冲突策略正确     |
| **S4（下载增强）**  | P1-1 限速+魔数校验、P1-2 视频续传、P1-3 节流熔断   | 带宽治理、续传、防限制      | 假服务器虚拟时间测试通过         |
| **S5（可选深化）**  | P0-4b SQLite 完整版、P2 事件细分/准备作业/删除安全 | 大规模可靠性           | 故障注入契约测试             |

每阶段结束跑我方现有 `check.bat`（fmt --check → check → clippy -D warnings → test → 前端 typecheck+build），确保不破坏既有功能。

---

## 七、风险红线与合规

1. **License 合规**：bdl-main 为 MIT（Copyright (c) 2026 YUELI）。**借鉴思路不受限；若直接复制其代码片段，须在源码/文档保留版权与许可声明**。建议以"重写适配实现"为主，避免整段拷贝。
2. **PRD 边界不可破**：不登录微信、不抓历史消息、不破解验证。账号/凭证/风控绕过类逻辑一律不采用。
3. **变更分级**：P0-4（持久化）、P1-2（续传）属跨模块"重大"变更，动手前需确认方案取舍；S1/S2 为"常规"变更，可直接实施并补测试。
4. **依赖成本**：SQLite 完整版引入 `rusqlite(bundled)`，会增加编译时间与包体；轻量 JSON 快照可先规避。
5. **不碰范围**：不改动产物目录（dist/release）、不碰密钥与生产配置、不做无关重构。
6. **装包/执行提示**：如后续引入新依赖或运行外部工具，将先说明用途与风险再执行。

---

## 八、验收清单

- [ ] 命令错误统一为 `{code,message,detail}`，前端按 code 提示，敏感参数已脱敏
- [ ] 设置具备 schema_version，老配置自动迁移与校验
- [ ] 验证页/429/磁盘满/文件占用等失败有分级建议，新尝试不继承旧错误
- [ ] 下载中强杀进程，重启后任务以 Queued 恢复且可重试，已完成的不重下
- [ ] 同一链接重复采集，Skip/Append/Overwrite 三种策略行为正确
- [ ] 中文长标题按 UTF-8 边界安全截断，Windows 保留名被转义
- [ ] 限速可热更新且暂停可打断；损坏文件（长度对头错）被识别重下
- [ ] 视频中断后按 Range 续传，服务端变更时从头下载
- [ ] 批量解析按间隔发出，命中验证页后熔断期不再请求
- [ ] 全阶段 `check.bat` 通过，既有 F1–F12 功能不回归

---

## 九、设置页面专项分析

### 9.1 结论

**设置页有明确可借鉴点，但只抄"配套机制"，不抄"表单架构"。** BDL 设置页是一套重机制表单（draft/saved 双状态 + 显式保存 + 脏检查 + 离开确认 + 集中校验 + 环境健康检查），我方是轻量即时保存（300ms 防抖直写）。我方设置项仅 9 个、无联动复杂度，照搬重架构会破坏"改完即生效"的顺滑感，属过度设计。真正该补的是**校验收口、错误可见、保存反馈、目录健康提示**四项配套机制。

### 9.2 设置系统对比

| 维度 | BDL 设置页 | 我方设置页 | 是否借鉴 |
|---|---|---|---|
| 表单状态 | `saved`/`draft` 分离 + `changed` 脏检查 | 直改 store，300ms 防抖落盘 | **不抄**（保留轻量即时） |
| 保存交互 | 保存/撤销/恢复默认 + 脏指示器 | 无，改完自动生效 | **部分抄**（补"恢复默认"，撤销可选） |
| 离开确认 | `SettingsLeaveDialog`（保存并离开/放弃/继续编辑） | 无 | 不抄（无草稿即无此问题） |
| 校验收口 | Rust `validate()` + 前端多个 error getter | 分散：Rust 目录校验、HTML min/max、前端空判 | **抄**（并入 P0-3 迁移） |
| 错误可见 | 字段旁错误 + 保存按钮按错误禁用 | 仅保存目录有 `dirError`，其余静默 | **抄**（改动小收益高） |
| 保存反馈 | `InlineNotice` + toast（成功/失败） | 基本无 | **抄** |
| 环境健康检查 | `environmentHealth` 检查目录/FFmpeg，checkId 防竞态 | 无 | **抄思路**（改为"保存目录可写性"检查） |
| 写入串行化 | `enqueueSettingsWrite`（WeakMap promise 链） | 前端 debounce + 同步写 | 低优先（批量原子落盘才需要） |
| 选项集中定义 | `settingsCatalog.ts` 统一选项 | 组件顶部内联 `themes`/`targets` | 低优先（项数少，可顺手抽） |
| 分组导航 | 5 个 section + 左侧导航 | 单页平铺 4 组 | **触发式**：设置项 >15 再分组 |
| 预设管理 | 下载/命名预设库、分类搜索 | 无对应概念 | 不适用 |
| 命名模板预览 | 变量白名单 + 实时预览 | 无命名模板功能 | 不适用 |
| 更新检查/平台约束/FFmpeg | 应用更新、Android 约束、FFmpeg 路径 | PRD 无更新、仅 Windows、无 FFmpeg | 不适用 |

### 9.3 可升级项（按优先级）

**S1 级 · 立即受益、改动小**

1. **校验收口 + 错误可见**：把 `set_download_root` 的路径校验、`concurrency`/`retry_max`/质量值范围校验统一收进 `Settings::validate()`（P0-3 已引入）；前端 `SettingsSheet.vue` 为各字段增加就近错误文案（仿 BDL 的 `proxySettingsError`/`speedLimitMbError` getter），错误态加 `aria-invalid`。
   - 落地：`crates/core/src/model.rs`、`src/components/SettingsSheet.vue`、`src/types.ts`。
   - 验证：越界并发数→字段旁中文原因且不落盘；目录校验与 Rust 结果一致。

2. **保存反馈**：`api.ts` 把命令错误按 `code` 转中文（依赖 P0-1），设置页顶部一行 notice 显示"已保存 / 保存失败：<原因>"，避免静默失败。
   - 落地：`src/api.ts`、`src/store.ts`（`saveSettingsSoon` 捕获失败）、`SettingsSheet.vue`。
   - 验证：模拟只读磁盘，出现明确失败提示而非无反应。

3. **恢复默认**：页头加"恢复默认"按钮（仿 BDL `restoreAllDefaults`，但即时生效无需保存）。
   - 落地：`SettingsSheet.vue`、`src/store.ts`。
   - 验证：点击后主题/目录/并发回到出厂值并落盘。

**S3 级 · 随核心增强一起做**

4. **保存目录健康检查**：借鉴 `environmentHealth`，新增轻量检查命令（判断目录存在/可写/非只读盘），在目录输入行下方实时显示"可写 / 不存在 / 无权限"，替换当前只显示"实际保存到…"的提示。对"用户随意粘贴绝对路径"的我方尤其实用。
   - 落地：`src-tauri/src/commands.rs`、`api.ts`、`SettingsSheet.vue`。
   - 验证：指向不存在或只读路径，显示对应状态。

5. **设置项随能力扩展而增补**：P0-5 重名策略、P1-1 全局限速、P0-4 崩溃恢复开关、P0-2 日志级别都需在设置页新增控件。项数从 9 增至约 14，仍可单页平铺，但建议按"下载与保存 / 转换与命名 / 数据维护"三组分好，先不上左侧导航。

**触发式升级**：设置项超过 15 个（如再加平台扩展）时，再引入 BDL 的 section 导航与 draft/saved 机制；否则保持现状。

### 9.4 明确不抄的设置项

`ffmpeg_path`、`embed_cover/subtitles`、`subtitle/danmaku_format`、`media_mode`、`quality/codec` 编排、`proxy_mode/url`、`auto_check_updates`、`document_tree_output`、预设库——均属 B 站视频下载或 Android/更新场景，与我方"微信图片采集 + 仅 Windows + 无更新模块"的边界不符。

---

## 十、下一步

本 MD 文档即升级蓝图。**若确认按此推进，建议从 S1（错误边界 + 设置迁移 + 设置页校验收口）开始落地**——改动小、风险低、收益明确，且为后续诊断/持久化打好类型基础。可指定阶段让我直接实施代码改动，或先就 S3 的持久化选型（JSON 快照 vs SQLite）做取舍确认。
