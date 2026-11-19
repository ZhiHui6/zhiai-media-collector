use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub download_root: String,
    pub concurrency: usize,
    pub retry_max: u32,
    pub convert_enabled: bool,
    pub convert_target: ConvertTarget,
    pub jpeg_quality: u8,
    pub webp_quality: u8,
}

impl Default for Settings {
    fn default() -> Self {
        let root = dirs::picture_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("ZhiaiCollect");
        Self {
            theme: ThemeMode::Dark,
            download_root: root.to_string_lossy().into_owned(),
            concurrency: 4,
            retry_max: 3,
            convert_enabled: false,
            convert_target: ConvertTarget::Jpg,
            jpeg_quality: 92,
            webp_quality: 85,
        }
    }
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
