use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeConfig {
    pub server: ServerConfig,
    pub cache: CacheConfig,
    pub backends: Vec<BackendConfig>,
    pub geo: GeoClusterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
    pub port: u16,
    pub worker_threads: usize,
    pub max_connections: usize,
    pub client_timeout_secs: u64,
    pub enable_http2: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_capacity: usize,
    pub max_memory_bytes: usize,
    pub default_ttl_secs: u64,
    pub eviction_policy: String, // "lru", "lfu", "adaptive"
    pub cleanup_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub id: String,
    pub addr: String,
    pub weight: u32,
    pub max_connections: usize,
    pub health_path: String,
    pub probe_interval_secs: u64,
    pub probe_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoClusterConfig {
    pub local_pop_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub replication_factor: usize,
    pub vnodes_per_node: usize,
    pub peers: Vec<PeerPopConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerPopConfig {
    pub pop_code: String,
    pub addr: String,
    pub latitude: f64,
    pub longitude: f64,
    pub weight: u32,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: "0.0.0.0".to_string(),
                port: 8080,
                worker_threads: 4,
                max_connections: 10000,
                client_timeout_secs: 30,
                enable_http2: true,
            },
            cache: CacheConfig {
                max_capacity: 100_000,
                max_memory_bytes: 256 * 1024 * 1024, // 256 MB
                default_ttl_secs: 300,
                eviction_policy: "lru".to_string(),
                cleanup_interval_secs: 10,
            },
            backends: vec![
                BackendConfig {
                    id: "origin-primary".to_string(),
                    addr: "127.0.0.1:9001".to_string(),
                    weight: 100,
                    max_connections: 500,
                    health_path: "/healthz".to_string(),
                    probe_interval_secs: 3,
                    probe_timeout_secs: 2,
                },
                BackendConfig {
                    id: "origin-secondary".to_string(),
                    addr: "127.0.0.1:9002".to_string(),
                    weight: 80,
                    max_connections: 500,
                    health_path: "/healthz".to_string(),
                    probe_interval_secs: 3,
                    probe_timeout_secs: 2,
                },
            ],
            geo: GeoClusterConfig {
                local_pop_code: "IAD".to_string(),
                latitude: 38.9531,
                longitude: -77.4565,
                replication_factor: 3,
                vnodes_per_node: 128,
                peers: vec![
                    PeerPopConfig {
                        pop_code: "SFO".to_string(),
                        addr: "sfo.edge.hyper.internal:8080".to_string(),
                        latitude: 37.6213,
                        longitude: -122.3790,
                        weight: 100,
                    },
                    PeerPopConfig {
                        pop_code: "LHR".to_string(),
                        addr: "lhr.edge.hyper.internal:8080".to_string(),
                        latitude: 51.4700,
                        longitude: -0.4543,
                        weight: 90,
                    },
                    PeerPopConfig {
                        pop_code: "NRT".to_string(),
                        addr: "nrt.edge.hyper.internal:8080".to_string(),
                        latitude: 35.7720,
                        longitude: 140.3929,
                        weight: 85,
                    },
                ],
            },
        }
    }
}

impl EdgeConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.listen_addr, self.server.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 8080)))
    }

    pub fn client_timeout(&self) -> Duration {
        Duration::from_secs(self.server.client_timeout_secs)
    }

    pub fn default_ttl(&self) -> Duration {
        Duration::from_secs(self.cache.default_ttl_secs)
    }

    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_secs(self.cache.cleanup_interval_secs)
    }
}
