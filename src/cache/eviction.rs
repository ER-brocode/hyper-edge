use crate::cache::entry::CacheEntry;
use crate::cache::policy::EvictionPolicy;
use dashmap::DashMap;
use rand::seq::IteratorRandom;
use std::sync::Arc;

pub struct EvictionEngine {
    policy: EvictionPolicy,
    sample_size: usize,
}

impl EvictionEngine {
    pub fn new(policy: EvictionPolicy, sample_size: usize) -> Self {
        Self {
            policy,
            sample_size: sample_size.max(8),
        }
    }

    /// Select candidates using random sampling across shards and pick the lowest-score entry
    pub fn select_eviction_candidates(
        &self,
        map: &DashMap<String, Arc<CacheEntry>>,
        count_to_evict: usize,
    ) -> Vec<String> {
        if map.is_empty() || count_to_evict == 0 {
            return Vec::new();
        }

        let mut rng = rand::thread_rng();
        // Sample random entries from the concurrent hashmap without taking a global write lock
        let samples: Vec<(String, f64)> = map
            .iter()
            .choose_multiple(&mut rng, (count_to_evict * self.sample_size).max(16))
            .into_iter()
            .map(|entry| {
                let score = self.policy.compute_score(entry.value());
                (entry.key().clone(), score)
            })
            .collect();

        if samples.is_empty() {
            return Vec::new();
        }

        // Sort ascending: smallest score is evicted first (least recently used / least frequently used)
        let mut sorted = samples;
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        sorted
            .into_iter()
            .take(count_to_evict)
            .map(|(k, _)| k)
            .collect()
    }
}
