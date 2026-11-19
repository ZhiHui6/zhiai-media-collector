use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::diagnostics;
use crate::error::DownloadError;
use crate::extract;
use crate::fetch;
use crate::link;
use crate::model::{
    ImageItem, ItemState, ParseOutcome, QueueCommand, QueueStats, Settings, TaskRecord, UiEvent,
};
use crate::storage;

use super::EventSink;
use super::limiter::BandwidthLimiter;
use super::pacing::ResolveGate;
use super::preview;
use super::task::{self, TaskOutcome};

const PREVIEW_CONCURRENCY: usize = 4;
const PERSIST_DEBOUNCE: Duration = Duration::from_millis(800);
const VERIFICATION_COOLDOWN: Duration = Duration::from_secs(60);

struct Entry {
    item: ImageItem,
    state: ItemState,
    message: Option<String>,
    action: Option<String>,
    attempts: u32,
    path: Option<String>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

type Completion = (u64, Result<TaskOutcome, DownloadError>);

struct Actor {
    client: reqwest::Client,
    entries: HashMap<u64, Entry>,
    order: Vec<u64>,
    paused: bool,
    limit: usize,
    settings: Settings,
    sink: EventSink,
    complete_tx: UnboundedSender<Completion>,
    preview_permits: Arc<Semaphore>,
    limiter: Arc<BandwidthLimiter>,
    reserved: task::ReservedPaths,
    gate: Arc<ResolveGate>,
    dirty: bool,
    flush_at: Option<Instant>,
}

pub async fn run(
    mut commands: UnboundedReceiver<QueueCommand>,
    sink: EventSink,
    settings: Settings,
    recovered: Vec<TaskRecord>,
) {
    let client = match fetch::build_client() {
        Ok(client) => client,
        Err(err) => {
            sink.send(UiEvent::BatchParsed(ParseOutcome::Failed {
                url: String::new(),
                message: err.to_string(),
            }));
            return;
        }
    };
    let (complete_tx, mut complete_rx) = tokio::sync::mpsc::unbounded_channel();
    let limiter = Arc::new(BandwidthLimiter::new(speed_limit_bytes(&settings)));
    let gate = Arc::new(ResolveGate::new(Duration::from_millis(
        settings.parse_interval_ms,
    )));
    let mut actor = Actor {
        client,
        entries: HashMap::new(),
        order: Vec::new(),
        paused: !recovered.is_empty(),
        limit: settings.concurrency.clamp(1, 16),
        settings,
        sink,
        complete_tx,
        preview_permits: Arc::new(Semaphore::new(PREVIEW_CONCURRENCY)),
        limiter,
        reserved: task::new_reserved(),
        gate,
        dirty: false,
        flush_at: None,
    };
    if let Some(max_id) = recovered.iter().map(|task| task.item.id).max() {
        extract::ensure_id_above(max_id);
    }
    for record in recovered {
        if matches!(record.state, ItemState::Completed | ItemState::Skipped) {
            continue;
        }
        actor.order.push(record.item.id);
        actor.entries.insert(
            record.item.id,
            Entry {
                item: record.item,
                state: record.state,
                message: record.message,
                action: None,
                attempts: record.attempts,
                path: record.path,
                handle: None,
            },
        );
    }
    // 只有存在待执行任务时才进入暂停态，避免仅剩失败任务时误显示“已暂停”。
    actor.paused = actor
        .entries
        .values()
        .any(|entry| entry.state == ItemState::Queued);
    loop {
        actor.schedule();
        let flush_in = actor.flush_in();
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else {
                    break;
                };
                if !actor.handle_command(command).await {
                    break;
                }
            }
            Some(completion) = complete_rx.recv() => {
                actor.handle_completion(completion);
            }
            _ = tokio::time::sleep(flush_in), if actor.dirty => {
                actor.persist();
            }
        }
    }
    actor.persist();
    for entry in actor.entries.values_mut() {
        if let Some(handle) = entry.handle.take() {
            handle.abort();
        }
    }
}

impl Actor {
    fn schedule(&mut self) {
        if self.paused {
            self.sink.send(UiEvent::QueueStats(self.stats()));
            return;
        }
        let active = self
            .entries
            .values()
            .filter(|entry| entry.handle.is_some())
            .count();
        let mut ready = Vec::new();
        for id in &self.order {
            if active + ready.len() >= self.limit {
                break;
            }
            let Some(entry) = self.entries.get(id) else {
                continue;
            };
            if entry.state == ItemState::Queued && entry.handle.is_none() {
                ready.push((*id, entry.item.clone(), entry.attempts));
            }
        }
        let mut changed = false;
        for (id, item, attempts) in ready {
            let sink = self.sink.clone();
            let client = self.client.clone();
            let settings = self.settings.clone();
            let complete_tx = self.complete_tx.clone();
            let limiter = Arc::clone(&self.limiter);
            let reserved = Arc::clone(&self.reserved);
            let Some(entry) = self.entries.get_mut(&id) else {
                continue;
            };
            entry.state = ItemState::Downloading;
            entry.message = None;
            entry.action = None;
            entry.handle = Some(tokio::spawn(async move {
                if attempts > 0 {
                    let delay = Duration::from_secs(1u64 << (attempts.min(4) - 1));
                    tokio::time::sleep(delay).await;
                }
                sink.send(UiEvent::ItemStateChanged {
                    id,
                    state: ItemState::Downloading,
                    message: None,
                    action: None,
                });
                let result = task::run(&client, &item, &settings, &limiter, &reserved, &sink).await;
                let _ = complete_tx.send((id, result));
            }));
            self.sink.send(UiEvent::ItemStateChanged {
                id,
                state: ItemState::Downloading,
                message: None,
                action: None,
            });
            changed = true;
        }
        if changed {
            self.mark_dirty();
        }
        if self
            .entries
            .values()
            .all(|entry| entry.handle.is_none() && entry.state != ItemState::Queued)
            && let Ok(mut reserved) = self.reserved.lock()
        {
            reserved.clear();
        }
        self.sink.send(UiEvent::QueueStats(self.stats()));
    }

    fn handle_completion(&mut self, (id, result): Completion) {
        let mut changed = false;
        let mut cancelled = false;
        let mut event = None;
        {
            let Some(entry) = self.entries.get_mut(&id) else {
                return;
            };
            entry.handle = None;
            if entry.state == ItemState::Cancelled {
                cancelled = true;
            } else {
                match result {
                    Ok(outcome) => {
                        if outcome.skipped {
                            entry.state = ItemState::Skipped;
                            entry.message = Some("同名文件已存在，已跳过".to_string());
                            entry.action = None;
                            event = Some(UiEvent::ItemStateChanged {
                                id,
                                state: ItemState::Skipped,
                                message: entry.message.clone(),
                                action: None,
                            });
                        } else {
                            entry.state = ItemState::Completed;
                            entry.message = None;
                            entry.action = None;
                            entry.path = Some(outcome.path.clone());
                            event = Some(UiEvent::ItemStateChanged {
                                id,
                                state: ItemState::Completed,
                                message: None,
                                action: None,
                            });
                        }
                        changed = true;
                    }
                    Err(err) => {
                        let message = err.to_string();
                        if err.retryable() && entry.attempts < self.settings.retry_max {
                            entry.attempts += 1;
                            entry.state = ItemState::Queued;
                            let retry_message = format!("第 {} 次重试", entry.attempts);
                            entry.message = Some(retry_message.clone());
                            entry.action = None;
                            event = Some(UiEvent::ItemStateChanged {
                                id,
                                state: ItemState::Queued,
                                message: Some(retry_message),
                                action: None,
                            });
                        } else {
                            let action = diagnostics::classify(Some(err.code()), &message)
                                .map(str::to_string);
                            entry.state = ItemState::Failed;
                            entry.message = Some(message.clone());
                            entry.action = action.clone();
                            event = Some(UiEvent::ItemStateChanged {
                                id,
                                state: ItemState::Failed,
                                message: Some(message),
                                action,
                            });
                        }
                        changed = true;
                    }
                }
            }
        }
        if cancelled {
            self.sink.send(UiEvent::QueueStats(self.stats()));
            return;
        }
        if changed {
            self.mark_dirty();
        }
        if let Some(event) = event {
            self.sink.send(event);
        }
        self.sink.send(UiEvent::QueueStats(self.stats()));
    }

    async fn handle_command(&mut self, command: QueueCommand) -> bool {
        match command {
            QueueCommand::ParseText(text) => {
                let urls = link::extract_urls(&text);
                if urls.is_empty() {
                    self.sink.send(UiEvent::BatchParsed(ParseOutcome::Failed {
                        url: "输入".to_string(),
                        message: "未识别到公众号链接".to_string(),
                    }));
                    return true;
                }
                for url in urls {
                    if let Some(left) = self.gate.blocked_for() {
                        self.sink.send(UiEvent::BatchParsed(ParseOutcome::Failed {
                            url,
                            message: format!(
                                "访问验证未解除，约 {} 秒后自动恢复解析",
                                left.as_secs().max(1)
                            ),
                        }));
                        continue;
                    }
                    self.sink.send(UiEvent::ParseStarted { url: url.clone() });
                    self.gate.acquire().await;
                    let outcome = extract::parse_link(&self.client, &url).await;
                    if let ParseOutcome::Failed { message, .. } = &outcome
                        && message.contains("验证")
                    {
                        self.gate.block_for(VERIFICATION_COOLDOWN);
                    }
                    self.sink.send(UiEvent::BatchParsed(outcome));
                }
            }
            QueueCommand::Preview(items) => {
                for item in items {
                    let client = self.client.clone();
                    let sink = self.sink.clone();
                    let permits = Arc::clone(&self.preview_permits);
                    tokio::spawn(async move {
                        let Ok(_permit) = permits.acquire_owned().await else {
                            return;
                        };
                        preview::fetch(&client, &item, &sink).await;
                    });
                }
            }
            QueueCommand::LoadFull { id, url, path } => {
                let client = self.client.clone();
                let sink = self.sink.clone();
                let permits = Arc::clone(&self.preview_permits);
                tokio::spawn(async move {
                    let Ok(_permit) = permits.acquire_owned().await else {
                        return;
                    };
                    preview::full(&client, id, &url, path, &sink).await;
                });
            }
            QueueCommand::Enqueue(items) => {
                let mut changed = false;
                for item in items {
                    if let Some(existing) = self.entries.get_mut(&item.id) {
                        if matches!(
                            existing.state,
                            ItemState::Failed | ItemState::Cancelled | ItemState::Skipped
                        ) {
                            existing.item = item;
                            existing.state = ItemState::Queued;
                            existing.attempts = 0;
                            existing.message = None;
                            existing.action = None;
                            existing.path = None;
                            changed = true;
                        }
                        continue;
                    }
                    self.order.push(item.id);
                    self.entries.insert(
                        item.id,
                        Entry {
                            item,
                            state: ItemState::Queued,
                            message: None,
                            action: None,
                            attempts: 0,
                            path: None,
                            handle: None,
                        },
                    );
                    changed = true;
                }
                if changed {
                    self.mark_dirty();
                }
            }
            QueueCommand::PauseAll => {
                self.paused = true;
            }
            QueueCommand::ResumeAll => {
                self.paused = false;
            }
            QueueCommand::CancelItem(id) => {
                if let Some(entry) = self.entries.get_mut(&id) {
                    if let Some(handle) = entry.handle.take() {
                        handle.abort();
                    }
                    entry.state = ItemState::Cancelled;
                    entry.message = None;
                    entry.action = None;
                    self.mark_dirty();
                    self.sink.send(UiEvent::ItemStateChanged {
                        id,
                        state: ItemState::Cancelled,
                        message: None,
                        action: None,
                    });
                }
            }
            QueueCommand::RetryItem(id) => {
                if let Some(entry) = self.entries.get_mut(&id)
                    && matches!(
                        entry.state,
                        ItemState::Failed | ItemState::Cancelled | ItemState::Skipped
                    )
                {
                    entry.state = ItemState::Queued;
                    entry.attempts = 0;
                    entry.message = None;
                    entry.action = None;
                    self.mark_dirty();
                    self.sink.send(UiEvent::ItemStateChanged {
                        id,
                        state: ItemState::Queued,
                        message: None,
                        action: None,
                    });
                }
            }
            QueueCommand::RetryAllFailed => {
                let mut changed = false;
                for entry in self.entries.values_mut() {
                    if entry.state == ItemState::Failed && entry.handle.is_none() {
                        entry.state = ItemState::Queued;
                        entry.attempts = 0;
                        entry.message = None;
                        entry.action = None;
                        let id = entry.item.id;
                        self.sink.send(UiEvent::ItemStateChanged {
                            id,
                            state: ItemState::Queued,
                            message: None,
                            action: None,
                        });
                        changed = true;
                    }
                }
                if changed {
                    self.mark_dirty();
                }
            }
            QueueCommand::SetConcurrency(value) => {
                self.limit = value.clamp(1, 16);
                self.settings.concurrency = self.limit;
            }
            QueueCommand::UpdateSettings(settings) => {
                self.limit = settings.concurrency.clamp(1, 16);
                self.limiter.set_rate(speed_limit_bytes(&settings));
                self.gate
                    .set_interval(Duration::from_millis(settings.parse_interval_ms));
                self.settings = settings;
            }
            QueueCommand::ClearFinished => {
                let finished: Vec<u64> = self
                    .entries
                    .iter()
                    .filter(|(_, entry)| entry.state.is_finished())
                    .map(|(id, _)| *id)
                    .collect();
                for id in finished {
                    self.entries.remove(&id);
                }
                self.order.retain(|id| self.entries.contains_key(id));
                self.mark_dirty();
            }
            QueueCommand::Shutdown => {
                return false;
            }
        }
        self.sink.send(UiEvent::QueueStats(self.stats()));
        true
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        if self.flush_at.is_none() {
            self.flush_at = Some(Instant::now() + PERSIST_DEBOUNCE);
        }
    }

    fn flush_in(&self) -> Duration {
        self.flush_at
            .map(|at| at.saturating_duration_since(Instant::now()))
            .unwrap_or(PERSIST_DEBOUNCE)
    }

    fn persist(&mut self) {
        let tasks = self.snapshot();
        let _ = storage::save_tasks(&tasks);
        self.dirty = false;
        self.flush_at = None;
    }

    fn snapshot(&self) -> Vec<TaskRecord> {
        self.order
            .iter()
            .filter_map(|id| self.entries.get(id))
            .filter(|entry| !matches!(entry.state, ItemState::Completed | ItemState::Skipped))
            .map(|entry| TaskRecord {
                item: entry.item.clone(),
                state: entry.state,
                attempts: entry.attempts,
                message: entry.message.clone(),
                path: entry.path.clone(),
            })
            .collect()
    }

    fn stats(&self) -> QueueStats {
        let mut stats = QueueStats {
            paused: self.paused,
            concurrency: self.limit,
            ..Default::default()
        };
        for entry in self.entries.values() {
            if entry.handle.is_some() {
                stats.active += 1;
            }
            match entry.state {
                ItemState::Queued => stats.queued += 1,
                ItemState::Completed => stats.completed += 1,
                ItemState::Failed => stats.failed += 1,
                _ => {}
            }
        }
        stats
    }
}

fn speed_limit_bytes(settings: &Settings) -> u64 {
    settings.speed_limit_kbps as u64 * 1024
}
