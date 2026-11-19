use std::path::Path;

use serde::{Deserialize, Serialize};

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

pub fn default_download_root() -> String {
    dirs::picture_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ZhiaiCollect")
        .to_string_lossy()
        .into_owned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImageKind {
    Article,
    Cover,
    Avatar,
    QrCode,
    Album,
    Video,
    Unknown,
}

impl ImageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Article => "文章配图",
            Self::Cover => "封面",
            Self::Avatar => "头像",
            Self::QrCode => "二维码",
            Self::Album => "相册图片",
            Self::Video => "视频",
            Self::Unknown => "其他",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Article => "img",
            Self::Cover => "cover",
            Self::Avatar => "avatar",
            Self::QrCode => "qr",
            Self::Album => "album",
            Self::Video => "video",
            Self::Unknown => "other",
        }
    }

    pub fn is_video(self) -> bool {
        matches!(self, Self::Video)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageKind {
    Article,
    PicturePost,
    AlbumPage,
    Unknown,
}

impl PageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Article => "图文文章",
            Self::PicturePost => "图片消息",
            Self::AlbumPage => "合集",
            Self::Unknown => "未知页面",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageItem {
    pub id: u64,
    pub url: String,
    pub kind: ImageKind,
    pub seq: usize,
    pub page_url: String,
    pub page_title: String,
    pub format_hint: Option<String>,
    pub selected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParseBatch {
    pub page_url: String,
    pub page_title: String,
    pub account_name: String,
    pub page_kind: PageKind,
    pub items: Vec<ImageItem>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ParseOutcome {
    Images(ParseBatch),
    ArticleLinks {
        page_url: String,
        title: String,
        links: Vec<String>,
    },
    Failed {
        url: String,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    Queued,
    Downloading,
    Completed,
    Failed,
    Cancelled,
    Skipped,
}

impl ItemState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "等待中",
            Self::Downloading => "下载中",
            Self::Completed => "已完成",
            Self::Failed => "失败",
            Self::Cancelled => "已取消",
            Self::Skipped => "已跳过",
        }
    }

    pub fn is_finished(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConvertTarget {
    Jpg,
    Png,
    WebP,
}

impl ConvertTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jpg => "JPG",
            Self::Png => "PNG",
            Self::WebP => "WebP",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
            Self::WebP => "webp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "亮色",
            Self::Dark => "暗色",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplicatePolicy {
    Skip,
    #[default]
    AppendSuffix,
    Overwrite,
}

impl DuplicatePolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Skip => "跳过已有文件",
            Self::AppendSuffix => "自动加序号",
            Self::Overwrite => "覆盖已有文件",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 0 表示老版本配置（文件中没有版本字段）。
    #[serde(default)]
    pub schema_version: u32,
    pub theme: ThemeMode,
    pub download_root: String,
    pub concurrency: usize,
    pub retry_max: u32,
    pub convert_enabled: bool,
    pub convert_target: ConvertTarget,
    pub jpeg_quality: u8,
    pub webp_quality: u8,
    pub duplicate_policy: DuplicatePolicy,
    /// KB/s，0 表示不限速。
    pub speed_limit_kbps: u32,
    /// 批量解析时相邻请求的最小间隔（毫秒）。
    pub parse_interval_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 0,
            theme: ThemeMode::Dark,
            download_root: default_download_root(),
            concurrency: 4,
            retry_max: 3,
            convert_enabled: false,
            convert_target: ConvertTarget::Jpg,
            jpeg_quality: 92,
            webp_quality: 85,
            duplicate_policy: DuplicatePolicy::AppendSuffix,
            speed_limit_kbps: 0,
            parse_interval_ms: 500,
        }
    }
}

impl Settings {
    /// 把任意来源的配置归一为当前版本，保证后续逻辑拿到的值可用。
    pub fn normalized(mut self) -> Self {
        self.schema_version = SETTINGS_SCHEMA_VERSION;
        self.download_root = self
            .download_root
            .trim()
            .trim_matches(|character| character == '"' || character == '\'')
            .to_string();
        if self.download_root.is_empty() {
            self.download_root = default_download_root();
        } else if !Path::new(&self.download_root).is_absolute() {
            self.download_root = std::path::absolute(&self.download_root)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|_| default_download_root());
        }
        self.concurrency = self.concurrency.clamp(1, 16);
        self.retry_max = self.retry_max.min(5);
        self.jpeg_quality = self.jpeg_quality.clamp(1, 100);
        self.webp_quality = self.webp_quality.clamp(1, 100);
        self.speed_limit_kbps = self.speed_limit_kbps.min(1_048_576);
        self.parse_interval_ms = self.parse_interval_ms.min(10_000);
        self
    }

    /// 归一后仍不合法说明配置不可信，调用方应回退默认值。
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=16).contains(&self.concurrency) {
            return Err("并发数需在 1–16 之间".to_string());
        }
        if self.retry_max > 5 {
            return Err("重试次数需在 0–5 之间".to_string());
        }
        if !(1..=100).contains(&self.jpeg_quality) || !(1..=100).contains(&self.webp_quality) {
            return Err("图片质量需在 1–100 之间".to_string());
        }
        if self.download_root.trim().is_empty() {
            return Err("保存目录不能为空".to_string());
        }
        if !Path::new(&self.download_root).is_absolute() {
            return Err("保存目录必须是绝对路径".to_string());
        }
        Ok(())
    }
}

/// 队列快照中的单条任务，用于崩溃恢复。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRecord {
    pub item: ImageItem,
    pub state: ItemState,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub page_url: String,
    pub page_title: String,
    pub image_url: String,
    pub kind: ImageKind,
    pub path: String,
    pub bytes: u64,
    pub format: String,
    pub finished_at: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct QueueStats {
    pub queued: usize,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
    pub paused: bool,
    pub concurrency: usize,
}

#[derive(Debug)]
pub enum QueueCommand {
    ParseText(String),
    Preview(Vec<ImageItem>),
    LoadFull {
        id: u64,
        url: String,
        path: Option<String>,
    },
    Enqueue(Vec<ImageItem>),
    PauseAll,
    ResumeAll,
    CancelItem(u64),
    RetryItem(u64),
    RetryAllFailed,
    SetConcurrency(usize),
    UpdateSettings(Settings),
    ClearFinished,
    Shutdown,
}

#[derive(Debug)]
pub enum UiEvent {
    ParseStarted {
        url: String,
    },
    BatchParsed(ParseOutcome),
    ItemStateChanged {
        id: u64,
        state: ItemState,
        message: Option<String>,
        action: Option<String>,
    },
    ItemProgress {
        id: u64,
        done: u64,
        total: u64,
    },
    ItemCompleted {
        id: u64,
        path: String,
        bytes: u64,
        format: String,
    },
    ThumbnailReady {
        id: u64,
        rgba: Vec<u8>,
        width: usize,
        height: usize,
    },
    FullImageReady {
        id: u64,
        rgba: Vec<u8>,
        width: usize,
        height: usize,
    },
    FullImageFailed {
        id: u64,
        message: String,
    },
    QueueStats(QueueStats),
    HistoryAppended(HistoryEntry),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_legacy_settings() {
        let settings: Settings =
            serde_json::from_str(r#"{"concurrency": 99, "download_root": "  "}"#).unwrap();
        assert_eq!(settings.schema_version, 0);
        let normalized = settings.normalized();
        assert_eq!(normalized.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(normalized.concurrency, 16);
        assert!(!normalized.download_root.is_empty());
        assert!(Path::new(&normalized.download_root).is_absolute());
        assert!(normalized.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_settings() {
        let mut settings = Settings::default().normalized();
        settings.concurrency = 0;
        assert!(settings.validate().is_err());
        settings.concurrency = 4;
        settings.download_root = "relative/path".to_string();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn clamps_quality_and_interval() {
        let settings = Settings {
            jpeg_quality: 0,
            webp_quality: 255,
            parse_interval_ms: 99_999,
            speed_limit_kbps: u32::MAX,
            ..Settings::default()
        }
        .normalized();
        assert_eq!(settings.jpeg_quality, 1);
        assert_eq!(settings.webp_quality, 100);
        assert_eq!(settings.parse_interval_ms, 10_000);
        assert_eq!(settings.speed_limit_kbps, 1_048_576);
    }
}
