use std::path::{Path, PathBuf};

use crate::error::StorageError;
use crate::model::{HistoryEntry, Settings};

pub const HISTORY_LIMIT: usize = 5000;

const APP_DIR: &str = "zhiai_media_collector";
const LEGACY_APP_DIR: &str = "wechat_image_downloader";

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
}

pub fn migrate_legacy_config() {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let legacy = base.join(LEGACY_APP_DIR);
    let target = base.join(APP_DIR);
    if !legacy.is_dir() || target.exists() {
        return;
    }
    if std::fs::rename(&legacy, &target).is_ok() {
        return;
    }
    if std::fs::create_dir_all(&target).is_err() {
        return;
    }
    for name in ["settings.json", "history.json"] {
        let source = legacy.join(name);
        if source.is_file() {
            let _ = std::fs::copy(&source, target.join(name));
        }
    }
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn history_path() -> PathBuf {
    config_dir().join("history.json")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Settings::default();
    };
    match serde_json::from_slice(&bytes) {
        Ok(settings) => settings,
        Err(_) => {
            quarantine(&path);
            Settings::default()
        }
    }
}

pub fn save_settings(settings: &Settings) -> Result<(), StorageError> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| StorageError::Io(e.to_string()))?;
    atomic_write(&settings_path(), &bytes)
}

pub fn load_history() -> Vec<HistoryEntry> {
    let path = history_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    match serde_json::from_slice(&bytes) {
        Ok(entries) => entries,
        Err(_) => {
            quarantine(&path);
            Vec::new()
        }
    }
}

pub fn save_history(entries: &[HistoryEntry]) -> Result<(), StorageError> {
    let start = entries.len().saturating_sub(HISTORY_LIMIT);
    let bytes =
        serde_json::to_vec(&entries[start..]).map_err(|e| StorageError::Io(e.to_string()))?;
    atomic_write(&history_path(), &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| StorageError::Io(e.to_string()))?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes).map_err(|e| StorageError::Io(e.to_string()))?;
    std::fs::rename(&temp, path).map_err(|e| StorageError::Io(e.to_string()))
}

fn quarantine(path: &Path) {
    let _ = std::fs::rename(path, path.with_extension("bad"));
}

pub fn sanitize_component(name: &str, max_len: usize) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']);
    let base = if trimmed.is_empty() {
        "untitled"
    } else {
        trimmed
    };
    let base = truncate_chars(base, max_len);
    let stem = base
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&stem.as_str()) {
        format!("_{base}")
    } else {
        base
    }
}

fn truncate_chars(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    value.chars().take(max_len).collect()
}

pub fn output_dir(root: &str, page_title: &str) -> PathBuf {
    PathBuf::from(root).join(sanitize_component(page_title, 80))
}

pub fn file_name(seq: usize, kind_slug: &str, media_hash: &str, extension: &str) -> String {
    format!("{seq:03}_{kind_slug}_{media_hash}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_illegal_characters() {
        assert_eq!(
            sanitize_component("a<b>c:d\"e/f\\g|h?i*j", 80),
            "a_b_c_d_e_f_g_h_i_j"
        );
    }

    #[test]
    fn trims_trailing_dots_and_spaces() {
        assert_eq!(sanitize_component("标题...  ", 80), "标题");
        assert_eq!(sanitize_component("   ", 80), "untitled");
    }

    #[test]
    fn escapes_reserved_device_names() {
        assert_eq!(sanitize_component("CON", 80), "_CON");
        assert_eq!(sanitize_component("nul.txt", 80), "_nul.txt");
        assert_eq!(sanitize_component("CONSOLE", 80), "CONSOLE");
    }

    #[test]
    fn truncates_by_characters() {
        let result = sanitize_component("一二三四五六七八九十", 4);
        assert_eq!(result.chars().count(), 4);
    }

    #[test]
    fn builds_named_file() {
        assert_eq!(
            file_name(7, "cover", "AbC123", "jpg"),
            "007_cover_AbC123.jpg"
        );
    }
}
