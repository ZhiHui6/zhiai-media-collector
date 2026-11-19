use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use zhiai_core::model::{ParseOutcome, UiEvent};

use crate::media::MediaKind;
use crate::state::AppState;

const EVENT: &str = "core://event";

pub fn spawn(app: AppHandle, events: Receiver<UiEvent>) {
    std::thread::spawn(move || {
        let mut history_dirty = false;
        let mut last_change = Instant::now();
        loop {
            match events.recv_timeout(Duration::from_millis(250)) {
                Ok(event) => {
                    let is_history = matches!(event, UiEvent::HistoryAppended(_));
                    emit(&app, event);
                    if is_history {
                        history_dirty = true;
                        last_change = Instant::now();
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if history_dirty && last_change.elapsed() >= Duration::from_secs(1) {
                app.state::<AppState>().save_history();
                history_dirty = false;
            }
        }
        if history_dirty {
            app.state::<AppState>().save_history();
        }
    });
}

fn emit(app: &AppHandle, event: UiEvent) {
    let state = app.state::<AppState>();
    let payload = match event {
        UiEvent::ParseStarted { url } => json!({ "type": "parseStarted", "url": url }),
        UiEvent::BatchParsed(outcome) => {
            if let ParseOutcome::Images(batch) = &outcome {
                for item in &batch.items {
                    state.remember(item);
                }
            }
            json!({ "type": "batchParsed", "outcome": outcome })
        }
        UiEvent::ItemStateChanged {
            id,
            state: item_state,
            message,
        } => json!({ "type": "itemState", "id": id, "state": item_state, "message": message }),
        UiEvent::ItemProgress { id, done, total } => {
            json!({ "type": "itemProgress", "id": id, "done": done, "total": total })
        }
        UiEvent::ItemCompleted {
            id,
            path,
            bytes,
            format,
        } => {
            state.remember_path(id, path.clone());
            json!({ "type": "itemCompleted", "id": id, "path": path, "bytes": bytes, "format": format })
        }
        UiEvent::ThumbnailReady {
            id,
            rgba,
            width,
            height,
        } => {
            state
                .media
                .insert_rgba(MediaKind::Thumb, id, rgba, width, height);
            json!({ "type": "thumbReady", "id": id, "width": width, "height": height })
        }
        UiEvent::FullImageReady {
            id,
            rgba,
            width,
            height,
        } => {
            state
                .media
                .insert_rgba(MediaKind::Full, id, rgba, width, height);
            json!({ "type": "fullReady", "id": id, "width": width, "height": height })
        }
        UiEvent::FullImageFailed { id, message } => {
            json!({ "type": "fullFailed", "id": id, "message": message })
        }
        UiEvent::QueueStats(stats) => json!({ "type": "queueStats", "stats": stats }),
        UiEvent::HistoryAppended(entry) => {
            state.push_history(entry.clone());
            json!({ "type": "history", "entry": entry })
        }
    };
    let _ = app.emit(EVENT, payload);
}
