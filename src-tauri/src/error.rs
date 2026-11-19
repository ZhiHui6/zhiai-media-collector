use serde::Serialize;

use zhiai_core::error::{DownloadError, FetchError, ImagingError, StorageError, redact_sensitive};

/// 统一命令错误：前端按 `code` 分支处理，`message` 可直接展示。
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub type CommandResult<T> = Result<T, CommandError>;

impl CommandError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: redact_sensitive(&message.into()),
            detail: None,
        }
    }

    pub fn from_io(context: &str, error: &std::io::Error) -> Self {
        let code = match error.raw_os_error() {
            Some(28) | Some(112) => "disk_full",
            Some(32) | Some(33) => "file_busy",
            Some(5) => "permission_denied",
            _ => "file_access",
        };
        Self::new(code, format!("{context}：{error}"))
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::new("invalid_input", message)
    }
}

impl From<StorageError> for CommandError {
    fn from(error: StorageError) -> Self {
        Self::new("storage", error.to_string())
    }
}

impl From<DownloadError> for CommandError {
    fn from(error: DownloadError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

impl From<FetchError> for CommandError {
    fn from(error: FetchError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

impl From<ImagingError> for CommandError {
    fn from(error: ImagingError) -> Self {
        Self::new("imaging_failed", error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_raw_os_errors() {
        let full = CommandError::from_io("写入", &std::io::Error::from_raw_os_error(112));
        assert_eq!(full.code, "disk_full");
        let busy = CommandError::from_io("写入", &std::io::Error::from_raw_os_error(32));
        assert_eq!(busy.code, "file_busy");
        let denied = CommandError::from_io("写入", &std::io::Error::from_raw_os_error(5));
        assert_eq!(denied.code, "permission_denied");
    }

    #[test]
    fn redacts_before_display() {
        let error = CommandError::new("network", "失败 https://a.com/x?token=abc&k=1");
        assert!(error.message.contains("token=REDACTED"));
        assert!(!error.message.contains("abc"));
    }
}
