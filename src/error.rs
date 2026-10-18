use thiserror::Error;

#[derive(Error, Debug)]
pub enum HyperEdgeError {
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),

    #[error("Proxy error: {0}")]
    Proxy(#[from] ProxyError),

    #[error("Geographic routing error: {0}")]
    Geo(#[from] GeoError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CacheError {
    #[error("Key not found in cache")]
    NotFound,

    #[error("Cache entry expired")]
    Expired,

    #[error("Cache capacity exceeded")]
    CapacityExceeded,

    #[error("Cache serialization failure: {0}")]
    Serialization(String),

    #[error("Lock acquisition timeout")]
    LockTimeout,
}

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("No healthy upstream backends available")]
    NoHealthyBackends,

    #[error("Backend connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Backend request timeout after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Connection pool exhausted for backend: {0}")]
    PoolExhausted(String),

    #[error("Upstream HTTP protocol error: {0}")]
    Protocol(String),

    #[error("Stream pipeline error: {0}")]
    Pipeline(String),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum GeoError {
    #[error("Hash ring is empty, no PoP nodes configured")]
    EmptyRing,

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Invalid geographic coordinates: lat={lat}, lon={lon}")]
    InvalidCoordinates { lat: f64, lon: f64 },

    #[error("Requested replication factor {requested} exceeds available nodes {available}")]
    InvalidReplicationFactor { requested: usize, available: usize },
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file read error: {0}")]
    Io(String),

    #[error("Configuration deserialization failed: {0}")]
    Parse(String),

    #[error("Missing required configuration field: {0}")]
    MissingField(String),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}

pub type Result<T> = std::result::Result<T, HyperEdgeError>;
