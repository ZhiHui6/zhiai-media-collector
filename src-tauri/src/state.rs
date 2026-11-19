use std::collections::HashMap;
use std::sync::Mutex;

use zhiai_core::model::{HistoryEntry, ImageItem, QueueCommand, Settings, TaskRecord};

use crate::media::MediaCache;

pub struct AppState {
    pub commands: tokio::sync::mpsc::UnboundedSender<QueueCommand>,
    pub items: Mutex<HashMap<u64, ImageItem>>,
    pub paths: Mutex<HashMap<u64, String>>,
    pub history: Mutex<Vec<HistoryEntry>>,
    pub settings: Mutex<Settings>,
    pub media: MediaCache,
    tasks: Vec<TaskRecord>,
    library_files: Mutex<(u64, HashMap<u64, String>)>,
}

impl AppState {
    pub fn new(
        commands: tokio::sync::mpsc::UnboundedSender<QueueCommand>,
        settings: Settings,
        history: Vec<HistoryEntry>,
        tasks: Vec<TaskRecord>,
    ) -> Self {
        // 恢复的任务也要可预览/可再次入队，因此同步回填 item 与路径索引。
        let mut items = HashMap::new();
        let mut paths = HashMap::new();
        for task in &tasks {
            items.insert(task.item.id, task.item.clone());
            if let Some(path) = &task.path {
                paths.insert(task.item.id, path.clone());
            }
        }
        Self {
            commands,
            items: Mutex::new(items),
            paths: Mutex::new(paths),
            history: Mutex::new(history),
            settings: Mutex::new(settings),
            media: MediaCache::new(),
            tasks,
            library_files: Mutex::new((0, HashMap::new())),
        }
    }

    pub fn tasks_snapshot(&self) -> Vec<TaskRecord> {
        self.tasks.clone()
    }

    pub fn register_library_file(&self, path: &str) -> u64 {
        let Ok(mut guard) = self.library_files.lock() else {
            return 0;
        };
        guard.0 += 1;
        let id = guard.0;
        guard.1.insert(id, path.to_string());
        id
    }

    pub fn library_path(&self, id: u64) -> Option<String> {
        self.library_files.lock().ok()?.1.get(&id).cloned()
    }

    pub fn remove_history_paths(&self, paths: &[String]) -> usize {
        let Ok(mut history) = self.history.lock() else {
            return 0;
        };
        let before = history.len();
        history.retain(|entry| !paths.iter().any(|path| path == &entry.path));
        before - history.len()
    }

    pub fn item(&self, id: u64) -> Option<ImageItem> {
        self.items.lock().ok()?.get(&id).cloned()
    }

    pub fn remember(&self, item: &ImageItem) {
        if let Ok(mut items) = self.items.lock() {
            items.insert(item.id, item.clone());
        }
    }

    pub fn remember_path(&self, id: u64, path: String) {
        if let Ok(mut paths) = self.paths.lock() {
            paths.insert(id, path);
        }
    }

    pub fn path(&self, id: u64) -> Option<String> {
        self.paths.lock().ok()?.get(&id).cloned()
    }

    pub fn push_history(&self, entry: HistoryEntry) {
        if let Ok(mut history) = self.history.lock() {
            history.push(entry);
            let len = history.len();
            let limit = zhiai_core::storage::HISTORY_LIMIT;
            if len > limit {
                history.drain(0..len - limit);
            }
        }
    }

    pub fn history_snapshot(&self) -> Vec<HistoryEntry> {
        self.history
            .lock()
            .map(|history| history.clone())
            .unwrap_or_default()
    }

    pub fn save_history(&self) {
        let history = self.history_snapshot();
        let _ = zhiai_core::storage::save_history(&history);
    }

    pub fn settings_snapshot(&self) -> Settings {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .unwrap_or_default()
    }

    pub fn set_settings(&self, settings: Settings) {
        if let Ok(mut current) = self.settings.lock() {
            *current = settings;
        }
    }
}
