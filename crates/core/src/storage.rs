use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::StorageError;
use crate::model::{DuplicatePolicy, HistoryEntry, ItemState, Settings, TaskRecord};

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

fn tasks_path() -> PathBuf {
    config_dir().join("tasks.json")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Settings::default().normalized();
    };
    match serde_json::from_slice::<Settings>(&bytes) {
        Ok(settings) => {
            let normalized = settings.normalized();
            if normalized.validate().is_ok() {
                normalized
            } else {
                quarantine(&path);
                Settings::default().normalized()
            }
        }
        Err(_) => {
            quarantine(&path);
            Settings::default().normalized()
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

/// 读取队列快照；中断的下载统一恢复为等待中，供用户确认后继续。
pub fn load_tasks() -> Vec<TaskRecord> {
    let path = tasks_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    match serde_json::from_slice::<Vec<TaskRecord>>(&bytes) {
        Ok(tasks) => tasks.into_iter().map(recover_task).collect(),
        Err(_) => {
            quarantine(&path);
            Vec::new()
        }
    }
}

fn recover_task(mut task: TaskRecord) -> TaskRecord {
    if task.state == ItemState::Downloading {
        task.state = ItemState::Queued;
        task.message = Some("上次退出时未完成，已重新排队".to_string());
    }
    task
}

pub fn save_tasks(tasks: &[TaskRecord]) -> Result<(), StorageError> {
    let bytes = serde_json::to_vec(tasks).map_err(|e| StorageError::Io(e.to_string()))?;
    atomic_write(&tasks_path(), &bytes)
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

pub fn file_stem(seq: usize, kind_slug: &str, media_hash: &str) -> String {
    format!("{seq:03}_{kind_slug}_{media_hash}")
}

pub fn file_name(seq: usize, kind_slug: &str, media_hash: &str, extension: &str) -> String {
    format!("{}.{extension}", file_stem(seq, kind_slug, media_hash))
}

/// Windows 路径大小写不敏感，预留键统一转大写。
pub fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_uppercase()
}

/// 按冲突策略挑一个不撞车（磁盘 + 本批预留）的落盘路径。
pub fn reserve_path(
    dir: &Path,
    base: &str,
    extension: &str,
    policy: DuplicatePolicy,
    reserved: &mut HashSet<String>,
) -> Option<PathBuf> {
    let candidate = dir.join(format!("{base}.{extension}"));
    let key = path_key(&candidate);
    match policy {
        DuplicatePolicy::Skip => {
            if candidate.exists() || reserved.contains(&key) {
                return None;
            }
            reserved.insert(key);
            Some(candidate)
        }
        DuplicatePolicy::Overwrite => {
            reserved.insert(key);
            Some(candidate)
        }
        DuplicatePolicy::AppendSuffix => {
            if !candidate.exists() && !reserved.contains(&key) {
                reserved.insert(key);
                return Some(candidate);
            }
            for index in 1..=10_000 {
                let path = dir.join(format!("{base}_{index}.{extension}"));
                let key = path_key(&path);
                if !path.exists() && !reserved.contains(&key) {
                    reserved.insert(key);
                    return Some(path);
                }
            }
            None
        }
    }
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

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zhiai-naming-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn skip_policy_refuses_existing_and_reserved() {
        let dir = test_dir("skip");
        std::fs::write(dir.join("001_img_abc.jpg"), b"x").unwrap();
        let mut reserved = HashSet::new();
        assert!(
            reserve_path(
                &dir,
                "001_img_abc",
                "jpg",
                DuplicatePolicy::Skip,
                &mut reserved
            )
            .is_none()
        );
        assert!(
            reserve_path(
                &dir,
                "002_img_abc",
                "jpg",
                DuplicatePolicy::Skip,
                &mut reserved
            )
            .is_some()
        );
        assert!(
            reserve_path(
                &dir,
                "002_img_abc",
                "jpg",
                DuplicatePolicy::Skip,
                &mut reserved
            )
            .is_none()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_policy_picks_next_free_suffix() {
        let dir = test_dir("append");
        std::fs::write(dir.join("001_img_abc.jpg"), b"x").unwrap();
        let mut reserved = HashSet::new();
        let first = reserve_path(
            &dir,
            "001_img_abc",
            "jpg",
            DuplicatePolicy::AppendSuffix,
            &mut reserved,
        )
        .unwrap();
        assert_eq!(first.file_name().unwrap(), "001_img_abc_1.jpg");
        let second = reserve_path(
            &dir,
            "001_img_abc",
            "jpg",
            DuplicatePolicy::AppendSuffix,
            &mut reserved,
        )
        .unwrap();
        assert_eq!(second.file_name().unwrap(), "001_img_abc_2.jpg");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovers_interrupted_task_as_queued() {
        let task = TaskRecord {
            item: crate::model::ImageItem {
                id: 7,
                url: "https://mmbiz.qpic.cn/a/0".to_string(),
                kind: crate::model::ImageKind::Article,
                seq: 1,
                page_url: "https://mp.weixin.qq.com/s/abc".to_string(),
                page_title: "标题".to_string(),
                format_hint: None,
                selected: true,
            },
            state: ItemState::Downloading,
            attempts: 2,
            message: None,
            path: None,
        };
        let recovered = recover_task(task);
        assert_eq!(recovered.state, ItemState::Queued);
        assert!(recovered.message.is_some());
        assert_eq!(recovered.attempts, 2);
    }

    #[test]
    fn overwrite_policy_keeps_candidate() {
        let dir = test_dir("overwrite");
        let mut reserved = HashSet::new();
        let path = reserve_path(
            &dir,
            "001_img_abc",
            "jpg",
            DuplicatePolicy::Overwrite,
            &mut reserved,
        )
        .unwrap();
        assert_eq!(path.file_name().unwrap(), "001_img_abc.jpg");
        assert!(reserved.contains(&path_key(&path)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
