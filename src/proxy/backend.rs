use parking_lot::RwLock;
use serde::Serialize;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BackendState {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug)]
pub struct Backend {
    pub id: String,
    pub addr: String,
    pub weight: u32,
    pub max_connections: usize,
    pub active_connections: AtomicUsize,
    pub total_requests: AtomicU64,
    pub consecutive_failures: AtomicU32,
    pub consecutive_successes: AtomicU32,
    pub latency_ema_ms: AtomicU64,
    state: RwLock<BackendState>,
}

impl Backend {
    pub fn new(id: String, addr: String, weight: u32, max_connections: usize) -> Arc<Self> {
        Arc::new(Self {
            id,
            addr,
            weight: weight.max(1),
            max_connections: max_connections.max(1),
            active_connections: AtomicUsize::new(0),
            total_requests: AtomicU64::new(0),
            consecutive_failures: AtomicU32::new(0),
            consecutive_successes: AtomicU32::new(0),
            latency_ema_ms: AtomicU64::new(10), // seed with 10ms
            state: RwLock::new(BackendState::Healthy),
        })
    }

    pub fn state(&self) -> BackendState {
        *self.state.read()
    }

    pub fn is_healthy(&self) -> bool {
        *self.state.read() == BackendState::Healthy
    }

    pub fn is_available(&self) -> bool {
        let st = *self.state.read();
        st != BackendState::Unhealthy
            && self.active_connections.load(Ordering::Relaxed) < self.max_connections
    }

    pub fn set_state(&self, new_state: BackendState) {
        let mut w = self.state.write();
        *w = new_state;
    }

    pub fn record_success(&self, latency_ms: u64) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        let succ = self.consecutive_successes.fetch_add(1, Ordering::Relaxed) + 1;

        // Exponential moving average: 80% old + 20% new
        let current_ema = self.latency_ema_ms.load(Ordering::Relaxed);
        let new_ema = (current_ema * 8 + latency_ms * 2) / 10;
        self.latency_ema_ms.store(new_ema, Ordering::Relaxed);

        if succ >= 2 && *self.state.read() != BackendState::Healthy {
            self.set_state(BackendState::Healthy);
        }
    }

    pub fn record_failure(&self) {
        self.consecutive_successes.store(0, Ordering::Relaxed);
        let fails = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;

        if fails >= 3 {
            self.set_state(BackendState::Unhealthy);
        } else if fails >= 1 {
            self.set_state(BackendState::Degraded);
        }
    }

    pub fn acquire_connection(&self) -> Option<ConnectionGuard> {
        loop {
            let current = self.active_connections.load(Ordering::Relaxed);
            if current >= self.max_connections {
                return None;
            }
            if self
                .active_connections
                .compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                self.total_requests.fetch_add(1, Ordering::Relaxed);
                return Some(ConnectionGuard {
                    active_connections: &self.active_connections,
                });
            }
        }
    }
}

pub struct ConnectionGuard<'a> {
    active_connections: &'a AtomicUsize,
}

impl<'a> Drop for ConnectionGuard<'a> {
    fn drop(&mut self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}
