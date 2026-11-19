pub mod limiter;
pub mod pacing;
pub mod preview;
pub mod queue;
pub mod task;

use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::model::UiEvent;

pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct EventSink {
    tx: Sender<UiEvent>,
    wake: Wake,
}

impl EventSink {
    pub fn new(tx: Sender<UiEvent>, wake: Wake) -> Self {
        Self { tx, wake }
    }

    pub fn send(&self, event: UiEvent) {
        if self.tx.send(event).is_ok() {
            (self.wake)();
        }
    }
}
