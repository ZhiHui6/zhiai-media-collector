//! 全局令牌桶限速：速率可热更新，令牌不足时短睡，可被任务取消打断。

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const MAX_BURST_SECONDS: f64 = 2.0;
const MAX_SLEEP: Duration = Duration::from_millis(100);

pub struct BandwidthLimiter {
    rate: AtomicU64,
    state: Mutex<State>,
}

struct State {
    tokens: f64,
    last: Instant,
}

impl BandwidthLimiter {
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            rate: AtomicU64::new(bytes_per_sec),
            state: Mutex::new(State {
                tokens: bytes_per_sec as f64,
                last: Instant::now(),
            }),
        }
    }

    pub fn set_rate(&self, bytes_per_sec: u64) {
        self.rate.store(bytes_per_sec, Ordering::Relaxed);
        if let Ok(mut state) = self.state.lock() {
            state.tokens = bytes_per_sec as f64;
            state.last = Instant::now();
        }
    }

    pub fn rate(&self) -> u64 {
        self.rate.load(Ordering::Relaxed)
    }

    /// 0 表示不限速；限速时按字节串行消耗令牌。
    pub async fn acquire(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }
        loop {
            match self.take_tokens(bytes) {
                None => return,
                Some(wait) => tokio::time::sleep(wait).await,
            }
        }
    }

    fn take_tokens(&self, bytes: u64) -> Option<Duration> {
        let rate = self.rate.load(Ordering::Relaxed);
        if rate == 0 {
            return None;
        }
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last).as_secs_f64();
        state.last = now;
        let burst = rate as f64 * MAX_BURST_SECONDS;
        state.tokens = (state.tokens + elapsed * rate as f64).min(burst);
        if state.tokens >= bytes as f64 {
            state.tokens -= bytes as f64;
            return None;
        }
        let missing = bytes as f64 - state.tokens;
        Some(Duration::from_secs_f64(missing / rate as f64).min(MAX_SLEEP))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unlimited_never_blocks() {
        let limiter = BandwidthLimiter::new(0);
        tokio::time::timeout(Duration::from_millis(50), limiter.acquire(10_000_000))
            .await
            .expect("不限速时不应等待");
    }

    #[test]
    fn hot_updates_rate() {
        let limiter = BandwidthLimiter::new(0);
        assert_eq!(limiter.rate(), 0);
        limiter.set_rate(512 * 1024);
        assert_eq!(limiter.rate(), 512 * 1024);
    }
}
