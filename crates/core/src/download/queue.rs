use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::error::DownloadError;
use crate::extract;
use crate::fetch;
use crate::link;
use crate::model::{
    ImageItem, ItemState, ParseOutcome, QueueCommand, QueueStats, Settings, UiEvent,
};

use super::EventSink;
use super::preview;
use super::task::{self, TaskOutcome};

const PREVIEW_CONCURRENCY: usize = 4;

struct Entry {
    item: ImageItem,
    state: ItemState,
    message: Option<String>,
    attempts: u32,
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
}

pub async fn run(
    mut commands: UnboundedReceiver<QueueCommand>,
    sink: EventSink,
    settings: Settings,
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
    let mut actor = Actor {
        client,
        entries: HashMap::new(),
        order: Vec::new(),
        paused: false,
        limit: settings.concurrency.clamp(1, 16),
        settings,
        sink,
        complete_tx,
        preview_permits: Arc::new(Semaphore::new(PREVIEW_CONCURRENCY)),
    };
    loop {
        actor.schedule();
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
        }
    }
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
        let mut active = self
            .entries
            .values()
            .filter(|entry| entry.handle.is_some())
            .count();
        for id in &self.order {
            if active >= self.limit {
                break;
            }
            let id = *id;
            let sink = self.sink.clone();
            let client = self.client.clone();
            let settings = self.settings.clone();
            let complete_tx = self.complete_tx.clone();
            let Some(entry) = self.entries.get_mut(&id) else {
                continue;
            };
            if entry.state != ItemState::Queued || entry.handle.is_some() {
                continue;
            }
            let item = entry.item.clone();
            let attempts = entry.attempts;
            entry.state = ItemState::Downloading;
            entry.handle = Some(tokio::spawn(async move {
                if attempts > 0 {
                    let delay = Duration::from_secs(1u64 << (attempts.min(4) - 1));
                    tokio::time::sleep(delay).await;
                }
                sink.send(UiEvent::ItemStateChanged {
                    id,
                    state: ItemState::Downloading,
                    message: None,
                });
                let result = task::run(&client, &item, &settings, &sink).await;
                let _ = complete_tx.send((id, result));
            }));
            self.sink.send(UiEvent::ItemStateChanged {
                id,
                state: ItemState::Downloading,
                message: None,
            });
            active += 1;
        }
        self.sink.send(UiEvent::QueueStats(self.stats()));
    }

    fn handle_completion(&mut self, (id, result): Completion) {
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        entry.handle = None;
        if entry.state == ItemState::Cancelled {
            self.sink.send(UiEvent::QueueStats(self.stats()));
            return;
        }
        match result {
            Ok(_) => {
                entry.state = ItemState::Completed;
                entry.message = None;
                self.sink.send(UiEvent::ItemStateChanged {
                    id,
                    state: ItemState::Completed,
                    message: None,
                });
            }
            Err(err) => {
                if err.retryable() && entry.attempts < self.settings.retry_max {
                    entry.attempts += 1;
                    entry.state = ItemState::Queued;
                    let message = format!("第 {} 次重试", entry.attempts);
                    entry.message = Some(message.clone());
                    self.sink.send(UiEvent::ItemStateChanged {
                        id,
                        state: ItemState::Queued,
                        message: Some(message),
                    });
                } else {
                    let message = err.to_string();
                    entry.state = ItemState::Failed;
                    entry.message = Some(message.clone());
                    self.sink.send(UiEvent::ItemStateChanged {
                        id,
                        state: ItemState::Failed,
                        message: Some(message),
                    });
                }
            }
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
                    self.sink.send(UiEvent::ParseStarted { url: url.clone() });
                    let outcome = extract::parse_link(&self.client, &url).await;
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
                for item in items {
                    if let Some(existing) = self.entries.get_mut(&item.id) {
                        if matches!(existing.state, ItemState::Failed | ItemState::Cancelled) {
                            existing.item = item;
                            existing.state = ItemState::Queued;
                            existing.attempts = 0;
                            existing.message = None;
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
                            attempts: 0,
                            handle: None,
                        },
                    );
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
                    self.sink.send(UiEvent::ItemStateChanged {
                        id,
                        state: ItemState::Cancelled,
                        message: None,
                    });
                }
            }
            QueueCommand::RetryItem(id) => {
                if let Some(entry) = self.entries.get_mut(&id)
                    && matches!(entry.state, ItemState::Failed | ItemState::Cancelled)
                {
                    entry.state = ItemState::Queued;
                    entry.attempts = 0;
                    entry.message = None;
                    self.sink.send(UiEvent::ItemStateChanged {
                        id,
                        state: ItemState::Queued,
                        message: None,
                    });
                }
            }
            QueueCommand::RetryAllFailed => {
                for entry in self.entries.values_mut() {
                    if entry.state == ItemState::Failed && entry.handle.is_none() {
                        entry.state = ItemState::Queued;
                        entry.attempts = 0;
                        entry.message = None;
                        let id = entry.item.id;
                        self.sink.send(UiEvent::ItemStateChanged {
                            id,
                            state: ItemState::Queued,
                            message: None,
                        });
                    }
                }
            }
            QueueCommand::SetConcurrency(value) => {
                self.limit = value.clamp(1, 16);
                self.settings.concurrency = self.limit;
            }
            QueueCommand::UpdateSettings(settings) => {
                self.limit = settings.concurrency.clamp(1, 16);
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
            }
            QueueCommand::Shutdown => {
                return false;
            }
        }
        self.sink.send(UiEvent::QueueStats(self.stats()));
        true
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
