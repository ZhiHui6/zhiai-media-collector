use std::sync::LazyLock;

use regex::Regex;
use url::Url;

static URL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s"'<>()\[\]]+"#).unwrap());

/// 把文本中 URL 查询串里的敏感参数打码，避免日志/错误详情泄露凭证。
pub fn redact_sensitive(input: &str) -> String {
    URL_PATTERN
        .replace_all(input, |captures: &regex::Captures<'_>| {
            let raw = captures
                .get(0)
                .map(|matched| matched.as_str())
                .unwrap_or_default();
            redact_url(raw)
        })
        .into_owned()
}

fn redact_url(raw: &str) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return raw.to_string();
    };
    if url.query().is_none() {
        return raw.to_string();
    }
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let mut changed = false;
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        if is_sensitive_key(&key) {
            serializer.append_pair(&key, "REDACTED");
            changed = true;
        } else {
            serializer.append_pair(&key, &value);
        }
    }
    if !changed {
        return raw.to_string();
    }
    url.set_query(Some(&serializer.finish()));
    url.to_string()
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "key"
        || key == "sig"
        || key == "auth"
        || key == "cookie"
        || key.contains("token")
        || key.contains("secret")
        || key.contains("ticket")
        || key.contains("signature")
        || key.contains("session")
}

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

impl FetchError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Network(_) => "network",
            Self::Verification => "verification",
            Self::Status(status) => match status {
                401 | 403 => "http_auth",
                429 => "http_429",
                _ => "http_status",
            },
        }
    }
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

    pub fn code(&self) -> &'static str {
        match self {
            Self::Network(_) => "network",
            Self::Verification => "verification",
            Self::Status(status) => match status {
                401 | 403 => "http_auth",
                429 => "http_429",
                _ => "http_status",
            },
            Self::TooLarge => "too_large",
            Self::Timeout => "network_timeout",
            Self::Imaging(_) => "imaging_failed",
            Self::Io(message) => io_error_code(message),
        }
    }
}

fn io_error_code(message: &str) -> &'static str {
    if message.contains("os error 28")
        || message.contains("os error 112")
        || message.contains("磁盘空间")
    {
        "disk_full"
    } else if message.contains("os error 32")
        || message.contains("os error 33")
        || message.contains("另一个程序")
    {
        "file_busy"
    } else if message.contains("os error 5") || message.contains("拒绝访问") {
        "permission_denied"
    } else {
        "file_access"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_query_values() {
        let input = "请求失败 https://example.com/a?token=abc123&wx_fmt=png&pass_ticket=xyz";
        let output = redact_sensitive(input);
        assert!(output.contains("token=REDACTED"));
        assert!(output.contains("pass_ticket=REDACTED"));
        assert!(output.contains("wx_fmt=png"));
        assert!(!output.contains("abc123"));
        assert!(!output.contains("xyz"));
    }

    #[test]
    fn leaves_plain_text_untouched() {
        let input = "网络请求失败：连接被重置";
        assert_eq!(redact_sensitive(input), input);
        let url = "https://mp.weixin.qq.com/s/abc";
        assert_eq!(redact_sensitive(url), url);
    }

    #[test]
    fn maps_io_errors_to_codes() {
        assert_eq!(
            DownloadError::Io("os error 112".to_string()).code(),
            "disk_full"
        );
        assert_eq!(
            DownloadError::Io("os error 32".to_string()).code(),
            "file_busy"
        );
        assert_eq!(
            DownloadError::Io("os error 5".to_string()).code(),
            "permission_denied"
        );
        assert_eq!(DownloadError::Status(429).code(), "http_429");
        assert_eq!(DownloadError::Verification.code(), "verification");
    }
}
