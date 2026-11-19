#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("链接无效：{0}")]
    Invalid(String),
    #[error("不支持的链接类型，仅支持 mp.weixin.qq.com 文章与合集链接")]
    Unsupported,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("网络请求失败：{0}")]
    Network(String),
    #[error("页面需要访问验证，请在浏览器打开链接完成验证后重试")]
    Verification,
    #[error("页面返回异常状态码 {0}")]
    Status(u16),
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("页面中未找到可采集的图片或视频")]
    NoImages,
    #[error("页面结构解析失败：{0}")]
    Parse(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("网络请求失败：{0}")]
    Network(String),
    #[error("图片需要访问验证，请在浏览器打开文章完成验证后重试")]
    Verification,
    #[error("图片返回异常状态码 {0}")]
    Status(u16),
    #[error("图片超过 64MB 上限")]
    TooLarge,
    #[error("下载超时")]
    Timeout,
    #[error("图片处理失败：{0}")]
    Imaging(String),
    #[error("文件写入失败：{0}")]
    Io(String),
}

impl DownloadError {
    pub fn retryable(&self) -> bool {
        match self {
            Self::Network(_) | Self::Timeout => true,
            Self::Status(code) => *code == 429 || *code >= 500,
            _ => false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImagingError {
    #[error("无法识别图片格式")]
    UnknownFormat,
    #[error("图片解码失败：{0}")]
    Decode(String),
    #[error("格式转换失败：{0}")]
    Convert(String),
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("配置读写失败：{0}")]
    Io(String),
}
