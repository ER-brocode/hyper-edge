use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use serde::Serialize;

#[derive(Debug, Default)]
pub struct EdgeMetrics {
    pub requests_total: AtomicU64,
    pub cache_hits_total: AtomicU64,
    pub cache_misses_total: AtomicU64,
    pub cache_evictions_total: AtomicU64,
    pub active_connections: AtomicU64,
    pub backend_requests_total: AtomicU64,
    pub backend_failures_total: AtomicU64,
    pub bytes_served_total: AtomicU64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub cache_hits_total: u64,
    pub cache_misses_total: u64,
    pub cache_evictions_total: u64,
    pub active_connections: u64,
    pub backend_requests_total: u64,
    pub backend_failures_total: u64,
    pub bytes_served_total: u64,
    pub hit_ratio: f64,
}

impl EdgeMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn inc_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_cache_hit(&self) {
        self.cache_hits_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_cache_miss(&self) {
        self.cache_misses_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_cache_eviction(&self) {
        self.cache_evictions_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_active_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_active_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn inc_backend_request(&self) {
        self.backend_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_backend_failure(&self) {
        self.backend_failures_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes_served(&self, bytes: u64) {
        self.bytes_served_total.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn hit_ratio(&self) -> f64 {
        let hits = self.cache_hits_total.load(Ordering::Relaxed);
        let misses = self.cache_misses_total.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let hits = self.cache_hits_total.load(Ordering::Relaxed);
        let misses = self.cache_misses_total.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_ratio = if total == 0 { 0.0 } else { hits as f64 / total as f64 };

        MetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            cache_hits_total: hits,
            cache_misses_total: misses,
            cache_evictions_total: self.cache_evictions_total.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            backend_requests_total: self.backend_requests_total.load(Ordering::Relaxed),
            backend_failures_total: self.backend_failures_total.load(Ordering::Relaxed),
            bytes_served_total: self.bytes_served_total.load(Ordering::Relaxed),
            hit_ratio,
        }
    }

    pub fn to_prometheus(&self) -> String {
        let s = self.snapshot();
        format!(
            "# HELP hyperedge_requests_total Total number of edge requests processed\n\
             # TYPE hyperedge_requests_total counter\n\
             hyperedge_requests_total {}\n\
             # HELP hyperedge_cache_hits_total Cache hit count\n\
             # TYPE hyperedge_cache_hits_total counter\n\
             hyperedge_cache_hits_total {}\n\
             # HELP hyperedge_cache_misses_total Cache miss count\n\
             # TYPE hyperedge_cache_misses_total counter\n\
             hyperedge_cache_misses_total {}\n\
             # HELP hyperedge_cache_evictions_total Evicted item count\n\
             # TYPE hyperedge_cache_evictions_total counter\n\
             hyperedge_cache_evictions_total {}\n\
             # HELP hyperedge_active_connections Current active client connections\n\
             # TYPE hyperedge_active_connections gauge\n\
             hyperedge_active_connections {}\n\
             # HELP hyperedge_backend_requests_total Forwarded upstream backend requests\n\
             # TYPE hyperedge_backend_requests_total counter\n\
             hyperedge_backend_requests_total {}\n\
             # HELP hyperedge_backend_failures_total Upstream connection or probe failures\n\
             # TYPE hyperedge_backend_failures_total counter\n\
             hyperedge_backend_failures_total {}\n\
             # HELP hyperedge_bytes_served_total Total egress payload in bytes\n\
             # TYPE hyperedge_bytes_served_total counter\n\
             hyperedge_bytes_served_total {}\n\
             # HELP hyperedge_cache_hit_ratio Current edge cache hit ratio [0.0 - 1.0]\n\
             # TYPE hyperedge_cache_hit_ratio gauge\n\
             hyperedge_cache_hit_ratio {:.4}\n",
            s.requests_total,
            s.cache_hits_total,
            s.cache_misses_total,
            s.cache_evictions_total,
            s.active_connections,
            s.backend_requests_total,
            s.backend_failures_total,
            s.bytes_served_total,
            s.hit_ratio,
        )
    }
}
