# 模块分解

| 模块 | 文件 | 职责 | 输入 → 输出 | 依赖 |
|---|---|---|---|---|
| 共享类型 | crates/core/src/model.rs | ImageKind/ImageItem/ParseBatch/ParseOutcome/QueueCommand/UiEvent/Settings/HistoryEntry | — | serde |
| 错误 | crates/core/src/error.rs | 类型化错误 + 中文文案 + 重试判定 | — | thiserror |
| 链接 | crates/core/src/link.rs | 粘贴文本→URL 列表、链接种类判定 | 文本 → Vec<(Url, LinkKind)> | url, model, error |
| 抓取 | crates/core/src/fetch.rs | HTTP 封装（UA/Referer/超时）、验证页识别 | Url → HTML/bytes | reqwest, error |
| 提取 | crates/core/src/extract/* | 规范化、页面判定、图文/图片消息/合集提取器、兜底扫描、分类 | HTML → ParseBatch/ParseOutcome | fetch, link, scraper, regex |
| 下载 | crates/core/src/download/* | queue actor 调度；task 流式下载→嗅探→转换→落盘；preview 预览 | QueueCommand → UiEvent | fetch, imaging, storage |
| 图像 | crates/core/src/imaging.rs | guess_format 嗅探、扩展名、JPG/PNG/WebP 转换、缩略图 | bytes → bytes/RGBA | image, webp |
| 存储 | crates/core/src/storage.rs | settings/history 读写、原子写、文件名清洗、旧目录迁移 | Settings/HistoryEntry ↔ JSON | serde_json, dirs |
| Tauri 壳 | src-tauri/src/* | 状态、命令、事件桥、media 协议、窗口主题 | IPC ↔ QueueCommand/UiEvent | tauri, zhiai_core |
| 前端 | src/* | Vue 3 沉浸工作台、虚拟网格、浮层设置/历史/预览、风格化菜单与提示 | 镜像状态 → invoke | vue, @tauri-apps/api |
| 侦察 | crates/core/examples/probe_page.rs | 真实样本页面结构探针 | URL → 结构片段 | fetch |
| 图标 | crates/core/examples/make_icon.rs | 程序化生成应用图标套件 | — → src-tauri/icons | image |

## 提取子模块

- normalize.rs：反转义 → host 白名单 → /640→/0 → 去 query → 去重键
- article.rs：#js_content img（data-src 优先）、og:image 封面、profile 头像、二维码区、标题/账号名
- picture_post.rs：内嵌 JSON 提取（M5 侦察后落实）
- album.rs：__biz+album_id → getalbum 分页 → 文章链接批次
- fallback.rs：全响应正则扫描 mmbiz+qlogo（含转义），去重排序
- classify.rs：qlogo→头像、og:image→封面、qrcode→二维码、图片消息页→相册、其余→配图