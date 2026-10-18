use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EdgeNodeConfig {
    pub id: String,
    pub pop_code: String,
    pub addr: String,
    pub latitude: f64,
    pub longitude: f64,
    pub weight: u32,
}

#[derive(Debug)]
pub struct EdgeNode {
    pub id: String,
    pub pop_code: String,
    pub addr: String,
    pub latitude: f64,
    pub longitude: f64,
    pub weight: u32,
    healthy: AtomicBool,
}

impl EdgeNode {
    pub fn new(cfg: EdgeNodeConfig) -> Arc<Self> {
        Arc::new(Self {
            id: cfg.id,
            pop_code: cfg.pop_code,
            addr: cfg.addr,
            latitude: cfg.latitude,
            longitude: cfg.longitude,
            weight: cfg.weight.max(1),
            healthy: AtomicBool::new(true),
        })
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }
}
