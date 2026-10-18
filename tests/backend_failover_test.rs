use bytes::Bytes;
use http::{Request, StatusCode};
use http_body_util::Full;
use hyper_edge::cache::EdgeCache;
use hyper_edge::config::CacheConfig;
use hyper_edge::metrics::EdgeMetrics;
use hyper_edge::proxy::backend::{Backend, BackendState};
use hyper_edge::proxy::router::ProxyRouter;
use std::sync::Arc;

#[test]
fn test_backend_state_transitions() {
    let backend = Backend::new("test-b1".to_string(), "127.0.0.1:8001".to_string(), 100, 10);
    assert_eq!(backend.state(), BackendState::Healthy);
    assert!(backend.is_available());

    // 1st failure -> Degraded
    backend.record_failure();
    assert_eq!(backend.state(), BackendState::Degraded);
    assert!(backend.is_available());

    // 2nd failure -> Still Degraded
    backend.record_failure();
    assert_eq!(backend.state(), BackendState::Degraded);

    // 3rd failure -> Unhealthy
    backend.record_failure();
    assert_eq!(backend.state(), BackendState::Unhealthy);
    assert!(!backend.is_available());

    // Recovery: 2 consecutive successes restore to Healthy
    backend.record_success(15);
    assert_eq!(backend.state(), BackendState::Unhealthy);

    backend.record_success(12);
    assert_eq!(backend.state(), BackendState::Healthy);
    assert!(backend.is_available());
}

#[tokio::test]
async fn test_proxy_failover_selection() {
    let metrics = EdgeMetrics::new();
    let cache_cfg = CacheConfig {
        max_capacity: 100,
        max_memory_bytes: 1024 * 1024,
        default_ttl_secs: 60,
        eviction_policy: "lru".to_string(),
        cleanup_interval_secs: 10,
    };
    let cache = EdgeCache::new(&cache_cfg, metrics.clone());

    let b1 = Backend::new("origin-1".to_string(), "127.0.0.1:9001".to_string(), 100, 100);
    let b2 = Backend::new("origin-2".to_string(), "127.0.0.1:9002".to_string(), 100, 100);

    let router = ProxyRouter::new(
        vec![b1.clone(), b2.clone()],
        cache,
        metrics,
        "IAD".to_string(),
    );

    // Both healthy, candidate selected
    let (selected_1, _) = router.select_backend().expect("Should find healthy backend");
    assert!(selected_1.id == "origin-1" || selected_1.id == "origin-2");

    // Simulate complete crash of origin-1
    b1.set_state(BackendState::Unhealthy);

    // Router must failover and select origin-2 exclusively
    for _ in 0..10 {
        let (selected, _) = router.select_backend().expect("Must route to backup");
        assert_eq!(selected.id, "origin-2");
    }

    // Now origin-1 recovers
    b1.set_state(BackendState::Healthy);

    // Both should receive traffic again
    let mut selected_ids = Vec::new();
    for _ in 0..10 {
        let (selected, _) = router.select_backend().unwrap();
        selected_ids.push(selected.id.clone());
    }
    assert!(selected_ids.contains(&"origin-1".to_string()));
    assert!(selected_ids.contains(&"origin-2".to_string()));
}
