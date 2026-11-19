use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State};

use zhiai_core::model::{ImageItem, QueueCommand, QueueStats, Settings};

use crate::state::AppState;

#[tauri::command]
pub fn parse_text(state: State<'_, AppState>, text: String) {
    if !text.trim().is_empty() {
        let _ = state.commands.send(QueueCommand::ParseText(text));
    }
}

#[tauri::command]
pub fn parse_album(state: State<'_, AppState>, links: Vec<String>) {
    if !links.is_empty() {
        let _ = state
            .commands
            .send(QueueCommand::ParseText(links.join("\n")));
    }
}

#[tauri::command]
pub fn enqueue(state: State<'_, AppState>, ids: Vec<u64>) {
    let items: Vec<ImageItem> = ids.iter().filter_map(|id| state.item(*id)).collect();
    if !items.is_empty() {
        let _ = state.commands.send(QueueCommand::Enqueue(items));
    }
}

#[tauri::command]
pub fn open_preview(state: State<'_, AppState>, id: u64) {
    if let Some(item) = state.item(id) {
        let path = state.path(id);
        let _ = state.commands.send(QueueCommand::LoadFull {
            id,
            url: item.url,
            path,
        });
    }
}

#[tauri::command]
pub fn cancel_item(state: State<'_, AppState>, id: u64) {
    let _ = state.commands.send(QueueCommand::CancelItem(id));
}

#[tauri::command]
pub fn retry_item(state: State<'_, AppState>, id: u64) {
    let _ = state.commands.send(QueueCommand::RetryItem(id));
}

#[tauri::command]
pub fn retry_all_failed(state: State<'_, AppState>) {
    let _ = state.commands.send(QueueCommand::RetryAllFailed);
}

#[tauri::command]
pub fn pause_all(state: State<'_, AppState>) {
    let _ = state.commands.send(QueueCommand::PauseAll);
}

#[tauri::command]
pub fn resume_all(state: State<'_, AppState>) {
    let _ = state.commands.send(QueueCommand::ResumeAll);
}

#[tauri::command]
pub fn set_concurrency(state: State<'_, AppState>, value: usize) {
    let _ = state.commands.send(QueueCommand::SetConcurrency(value));
}

#[tauri::command]
pub fn clear_finished(state: State<'_, AppState>) {
    let _ = state.commands.send(QueueCommand::ClearFinished);
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, settings: Settings) {
    let _ = zhiai_core::storage::save_settings(&settings);
    state.set_settings(settings.clone());
    let _ = state.commands.send(QueueCommand::UpdateSettings(settings));
}

#[tauri::command]
pub fn get_bootstrap(state: State<'_, AppState>) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "settings": state.settings_snapshot(),
        "history": state.history_snapshot(),
        "stats": QueueStats::default(),
    })
}

#[tauri::command]
pub fn set_download_root(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let trimmed = path.trim().trim_matches(|c| c == '"' || c == '\'');
    if trimmed.is_empty() {
        return Err("目录不能为空".to_string());
    }
    let candidate = std::path::Path::new(trimmed);
    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        std::path::absolute(candidate).map_err(|error| format!("路径无效：{error}"))?
    };
    std::fs::create_dir_all(&absolute).map_err(|error| format!("无法创建目录：{error}"))?;
    let normalized = absolute.to_string_lossy().into_owned();
    let mut settings = state.settings_snapshot();
    settings.download_root = normalized.clone();
    let _ = zhiai_core::storage::save_settings(&settings);
    state.set_settings(settings.clone());
    let _ = state.commands.send(QueueCommand::UpdateSettings(settings));
    Ok(normalized)
}

#[tauri::command]
pub fn reveal_path(path: String) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn();
}

#[tauri::command]
pub fn open_directory(path: String) {
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

#[tauri::command]
pub fn apply_theme(app: AppHandle, mode: String) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;
    let theme = match mode.as_str() {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    };
    window.set_theme(theme).map_err(|error| error.to_string())
}

#[derive(serde::Serialize)]
pub struct LibraryFile {
    path: String,
    id: u64,
    exists: bool,
    width: u32,
    height: u32,
}

/// 为素材库中已落盘的图片注册可访问地址，并标记文件是否仍然存在。
#[tauri::command]
pub fn library_prepare(state: State<'_, AppState>, paths: Vec<String>) -> Vec<LibraryFile> {
    paths
        .into_iter()
        .map(|path| {
            let exists = std::path::Path::new(&path).is_file();
            let (width, height) = if exists {
                image::image_dimensions(&path).unwrap_or((0, 0))
            } else {
                (0, 0)
            };
            let id = state.register_library_file(&path);
            LibraryFile {
                path,
                id,
                exists,
                width,
                height,
            }
        })
        .collect()
}

/// 删除素材包：可选同时删除磁盘文件，返回移除的记录数。
#[tauri::command]
pub fn library_delete(
    state: State<'_, AppState>,
    paths: Vec<String>,
    delete_files: bool,
) -> usize {
    if delete_files {
        let mut dirs = std::collections::HashSet::new();
        for path in &paths {
            let file = std::path::Path::new(path);
            let _ = std::fs::remove_file(file);
            if let Some(parent) = file.parent() {
                dirs.insert(parent.to_path_buf());
            }
        }
        for dir in dirs {
            // 仅清理已经空掉的素材包目录
            let _ = std::fs::remove_dir(&dir);
        }
    }
    let removed = state.remove_history_paths(&paths);
    state.save_history();
    removed
}
