//! 解析请求闸门：串行化 + 最小间隔 + 命中验证页后的短时熔断。

use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct ResolveGate {
    interval: Mutex<Duration>,
    next_slot: Mutex<Instant>,
    blocked_until: Mutex<Option<Instant>>,
}

impl ResolveGate {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval: Mutex::new(interval),
            next_slot: Mutex::new(Instant::now()),
            blocked_until: Mutex::new(None),
        }
    }

    pub fn set_interval(&self, interval: Duration) {
        if let Ok(mut current) = self.interval.lock() {
            *current = interval;
        }
    }

    /// 熔断剩余时间；None 表示当前可以继续请求。
    pub fn blocked_for(&self) -> Option<Duration> {
        let until = self.blocked_until.lock().ok().and_then(|value| *value)?;
        until
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
    }

    pub fn block_for(&self, duration: Duration) {
        if let Ok(mut until) = self.blocked_until.lock() {
            *until = Some(Instant::now() + duration);
        }
    }

    /// 按配置间隔排队，调用方串行使用即可保证相邻请求互不重叠。
    pub async fn acquire(&self) {
        let wait = {
            let interval = self
                .interval
                .lock()
                .map(|value| *value)
                .unwrap_or(Duration::ZERO);
            let Ok(mut next) = self.next_slot.lock() else {
                return;
            };
            let now = Instant::now();
            let start = (*next).max(now);
            *next = start + interval;
            start.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_breaker_expires() {
        let gate = ResolveGate::new(Duration::ZERO);
        assert!(gate.blocked_for().is_none());
        gate.block_for(Duration::from_millis(200));
        assert!(gate.blocked_for().is_some());
    }

    #[tokio::test]
    async fn zero_interval_does_not_wait() {
        let gate = ResolveGate::new(Duration::ZERO);
        tokio::time::timeout(Duration::from_millis(50), gate.acquire())
            .await
            .expect("零间隔不应等待");
    }
}
