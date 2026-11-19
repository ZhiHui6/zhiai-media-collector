# 智爱媒体采集器

解析、筛选并批量下载微信公众号文章中的图片与视频素材的 Windows 桌面工具。基于 Tauri 2 + Rust + Vue 3 构建，界面为自绘的沉浸式工作台，支持深色 / 浅色主题。

## 功能特性

- **链接解析**：粘贴一段文本即可自动识别其中的 `mp.weixin.qq.com` 链接，支持单篇文章、图文（图片消息）、合集链接批量展开。
- **素材分类**：识别文章配图、封面、头像、二维码、相册图片与视频，并按类型打标，便于筛选。
- **预览与勾选**：解析结果以缩略图网格呈现，支持大图预览、全选 / 反选、按类型筛选后批量入队。
- **流式下载**：并发下载、单任务进度、失败重试、暂停 / 继续、清除已完成；支持限制下载速度（KB/s）。
- **格式转换**：可选转换为 JPG / PNG / WebP，并调节 JPEG / WebP 质量；GIF 等动图自动保护，不做破坏性转换。
- **落盘策略**：自定义下载根目录；重名文件支持跳过 / 追加后缀 / 覆盖。
- **传输视图**：按任务分组的「任务 - 文件」两级折叠列表，主行展示汇总进度，展开后可查看每个文件的独立进度与状态。
- **素材库**：将已下载素材按文章聚合成素材包，支持搜索、包内网格预览、文件缺失标记，以及仅移除记录或同时删除本地文件两种删除模式。
- **右键菜单**：素材包与图片均提供打开目录、复制路径、删除等快捷操作。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面壳 | Tauri 2（自定义标题栏、`media://` 本地图片协议） |
| 核心逻辑 | Rust workspace：`crates/core`（抓取、提取、下载、图像处理、存储，UI 无关） |
| 前端 | Vue 3 + TypeScript + Vite |

## 目录结构

```
crates/core/        Rust 核心库：fetch / extract / download / imaging / storage
src-tauri/          Tauri 壳：命令注册、事件桥、状态、图片协议、窗口
src/                Vue 3 前端：采集、传输、素材库、设置、关于
dev.bat             开发模式启动（Vite + cargo run）
build.bat           正式构建（前端构建 + release 编译 + 复制到 release/）
check.bat           全量检查（fmt / check / clippy / test / 前端类型检查）
```

## 本地开发

前置要求：Windows 10/11（含 WebView2 运行时）、Node.js 18+、Rust stable（edition 2024）。

```bash
npm install
```

一键启动开发模式（先起 Vite，再运行调试程序）：

```bat
dev.bat
```

或分别启动：

```bash
npm run dev        # 前端开发服务器，端口 1420
cargo run -p zhiai_media_collector
```

## 构建与检查

```bat
build.bat   :: 产物输出到 release\zhiai_media_collector.exe
check.bat   :: 格式、静态检查、单元测试与前端构建
```

## 数据存放

配置、历史记录与任务记录保存在 `%APPDATA%\zhiai_media_collector\`（`settings.json`、`history.json`、`tasks.json`），素材文件保存在设置中指定的下载根目录。

## 说明

本项目仅用于个人学习与素材备份，请遵守微信公众平台的服务条款及原作者版权，不要用于商业转载或批量抓取。

## 许可证

[MIT](LICENSE)
