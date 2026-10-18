use bytes::Bytes;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct CacheEntry {
    pub key: String,
    pub value: Bytes,
    pub content_type: Option<String>,
    pub status_code: u16,
    pub created_at: Instant,
    pub expires_at: Option<Instant>,
    pub last_accessed: AtomicU64,
    pub access_count: AtomicU64,
    pub size_bytes: usize,
}

impl CacheEntry {
    pub fn new(
        key: String,
        value: Bytes,
        ttl: Option<Duration>,
        content_type: Option<String>,
        status_code: u16,
    ) -> Self {
        let now = Instant::now();
        let expires_at = ttl.map(|t| now + t);
        let size_bytes = key.len() + value.len() + content_type.as_ref().map(|s| s.len()).unwrap_or(0) + 64;
        let now_millis = current_epoch_millis();

        Self {
            key,
            value,
            content_type,
            status_code,
            created_at: now,
            expires_at,
            last_accessed: AtomicU64::new(now_millis),
            access_count: AtomicU64::new(1),
            size_bytes,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            Instant::now() >= exp
        } else {
            false
        }
    }

    pub fn touch(&self) {
        self.last_accessed.store(current_epoch_millis(), Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn last_accessed_millis(&self) -> u64 {
        self.last_accessed.load(Ordering::Relaxed)
    }

    pub fn access_count(&self) -> u64 {
        self.access_count.load(Ordering::Relaxed)
    }

    pub fn ttl_remaining(&self) -> Option<Duration> {
        self.expires_at.and_then(|exp| {
            let now = Instant::now();
            if exp > now {
                Some(exp - now)
            } else {
                None
            }
        })
    }
}

fn current_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}
