use crate::cache::entry::CacheEntry;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Adaptive,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self::Lru
    }
}

impl std::str::FromStr for EvictionPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "lru" => Ok(EvictionPolicy::Lru),
            "lfu" => Ok(EvictionPolicy::Lfu),
            "adaptive" | "tinylfu" | "lfu-lru" => Ok(EvictionPolicy::Adaptive),
            other => Err(format!("Unknown eviction policy: {}", other)),
        }
    }
}

impl EvictionPolicy {
    pub fn compute_score(&self, entry: &CacheEntry) -> f64 {
        match self {
            EvictionPolicy::Lru => {
                entry.last_accessed_millis() as f64
            }
            EvictionPolicy::Lfu => {
                entry.access_count() as f64
            }
            EvictionPolicy::Adaptive => {
                let now_millis = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let age_secs = (now_millis.saturating_sub(entry.last_accessed_millis()) as f64) / 1000.0;
                let freq = entry.access_count() as f64;
                // Decay frequency over time to prevent stale frequency pollution
                freq / (1.0 + 0.1 * age_secs)
            }
        }
    }
}
