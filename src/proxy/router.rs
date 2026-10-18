use crate::cache::EdgeCache;
use crate::error::{HyperEdgeError, ProxyError, Result};
use crate::metrics::EdgeMetrics;
use crate::proxy::backend::Backend;
use crate::proxy::pipeline::StreamPipeline;
use crate::proxy::pool::BackendConnectionPool;
use bytes::Bytes;
use http::{HeaderValue, Method, Request, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

pub struct ProxyRouter {
    backends: RwLock<Vec<(Arc<Backend>, Arc<BackendConnectionPool>)>>,
    cache: Arc<EdgeCache>,
    metrics: Arc<EdgeMetrics>,
    pop_code: String,
    rr_counter: AtomicUsize,
}

impl ProxyRouter {
    pub fn new(
        backends: Vec<Arc<Backend>>,
        cache: Arc<EdgeCache>,
        metrics: Arc<EdgeMetrics>,
        pop_code: String,
    ) -> Self {
        let backend_pools = backends
            .into_iter()
            .map(|b| {
                let pool = Arc::new(BackendConnectionPool::new(
                    b.clone(),
                    32,
                    std::time::Duration::from_secs(60),
                ));
                (b, pool)
            })
            .collect();

        Self {
            backends: RwLock::new(backend_pools),
            cache,
            metrics,
            pop_code,
            rr_counter: AtomicUsize::new(0),
        }
    }

    pub fn add_backend(&self, backend: Arc<Backend>) {
        let pool = Arc::new(BackendConnectionPool::new(
            backend.clone(),
            32,
            std::time::Duration::from_secs(60),
        ));
        self.backends.write().push((backend, pool));
    }

    /// Select next available healthy backend using least-connections with round-robin tiebreaker
    pub fn select_backend(&self) -> Result<(Arc<Backend>, Arc<BackendConnectionPool>)> {
        let list = self.backends.read();
        if list.is_empty() {
            return Err(ProxyError::NoHealthyBackends.into());
        }

        let mut candidate: Option<(Arc<Backend>, Arc<BackendConnectionPool>)> = None;
        let mut min_conns = usize::MAX;

        let start = self.rr_counter.fetch_add(1, Ordering::Relaxed);
        let len = list.len();

        for i in 0..len {
            let idx = (start + i) % len;
            let (backend, pool) = &list[idx];
            if backend.is_available() {
                let conns = backend.active_connections.load(Ordering::Relaxed);
                if conns < min_conns {
                    min_conns = conns;
                    candidate = Some((backend.clone(), pool.clone()));
                }
            }
        }

        candidate.ok_or_else(|| ProxyError::NoHealthyBackends.into())
    }

    /// Handle incoming HTTP request with cache inspection, backend forwarding, and failover
    pub async fn handle_request(
        &self,
        req: Request<Incoming>,
    ) -> std::result::Result<Response<Full<Bytes>>, hyper::Error> {
        let start_time = Instant::now();
        let (parts, body) = req.into_parts();
        let uri_path = parts.uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/").to_string();
        let method = parts.method.clone();

        // 1. Health check & Metrics endpoints directly intercepted by edge
        if method == Method::GET && parts.uri.path() == "/healthz" {
            let res = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("X-Edge-Pop", &self.pop_code)
                .body(Full::new(Bytes::from(r#"{"status":"healthy","pop":"# + &self.pop_code + r#""}"#)))
                .unwrap();
            return Ok(res);
        }

        if method == Method::GET && parts.uri.path() == "/metrics" {
            let metrics_str = self.metrics.to_prometheus();
            let res = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/plain; version=0.0.4")
                .header("X-Edge-Pop", &self.pop_code)
                .body(Full::new(Bytes::from(metrics_str)))
                .unwrap();
            return Ok(res);
        }

        let cache_key = format!("{}:{}", method, uri_path);

        // 2. Cache Lookup for idempotent requests
        if method == Method::GET || method == Method::HEAD {
            if let Some(entry) = self.cache.get(&cache_key) {
                let latency = start_time.elapsed().as_millis();
                let mut builder = Response::builder()
                    .status(StatusCode::from_u16(entry.status_code).unwrap_or(StatusCode::OK))
                    .header("X-Cache", "HIT")
                    .header("X-Edge-Pop", &self.pop_code)
                    .header("X-Edge-Latency-Ms", latency.to_string());

                if let Some(ct) = &entry.content_type {
                    builder = builder.header("content-type", ct);
                }

                let body_bytes = if method == Method::HEAD {
                    Bytes::new()
                } else {
                    entry.value.clone()
                };

                return Ok(builder.body(Full::new(body_bytes)).unwrap());
            }
        }

        // 3. Cache Miss: buffer request payload for upstream dispatch
        let req_body_bytes = match StreamPipeline::collect_body(body, 10 * 1024 * 1024).await {
            Ok(b) => b,
            Err(err) => {
                let res = Response::builder()
                    .status(StatusCode::PAYLOAD_TOO_LARGE)
                    .body(Full::new(Bytes::from(err)))
                    .unwrap();
                return Ok(res);
            }
        };

        // 4. Upstream Forwarding with Failover Retry (up to 3 attempts)
        let max_retries = 3;
        let mut last_err = String::from("No backend tried");

        for attempt in 0..max_retries {
            let (backend, pool) = match self.select_backend() {
                Ok(b) => b,
                Err(e) => {
                    last_err = format!("{}", e);
                    break;
                }
            };

            let _guard = match backend.acquire_connection() {
                Some(g) => g,
                None => {
                    last_err = "Backend capacity reached".to_string();
                    continue;
                }
            };

            let mut upstream_req = Request::builder()
                .method(method.clone())
                .uri(&uri_path)
                .header("host", &backend.addr)
                .header("X-Forwarded-For", "127.0.0.1")
                .header("X-Edge-Pop", &self.pop_code);

            for (k, v) in parts.headers.iter() {
                if k != "host" && k != "content-length" {
                    upstream_req = upstream_req.header(k, v);
                }
            }

            let built_req = upstream_req.body(Full::new(req_body_bytes.clone())).unwrap();

            let req_start = Instant::now();
            self.metrics.inc_backend_request();

            let mut sender = match pool.get_sender().await {
                Ok(s) => s,
                Err(err) => {
                    backend.record_failure();
                    self.metrics.inc_backend_failure();
                    last_err = format!("Connection error: {}", err);
                    continue;
                }
            };

            match sender.send_request(built_req).await {
                Ok(upstream_resp) => {
                    let req_latency = req_start.elapsed().as_millis() as u64;
                    backend.record_success(req_latency);

                    let (resp_parts, resp_body) = upstream_resp.into_parts();
                    let resp_bytes = match StreamPipeline::collect_body(resp_body, 50 * 1024 * 1024).await {
                        Ok(b) => b,
                        Err(e) => {
                            last_err = format!("Upstream body streaming error: {}", e);
                            continue;
                        }
                    };

                    pool.return_sender(sender);

                    let content_type = resp_parts
                        .headers
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());

                    // Store in edge cache if cacheable
                    let is_cacheable = (method == Method::GET || method == Method::HEAD)
                        && StreamPipeline::is_response_cacheable(resp_parts.status, &resp_parts.headers);

                    if is_cacheable {
                        let ttl = StreamPipeline::parse_max_age(&resp_parts.headers);
                        self.cache.insert(
                            cache_key.clone(),
                            resp_bytes.clone(),
                            ttl,
                            content_type.clone(),
                            resp_parts.status.as_u16(),
                        );
                    }

                    let total_latency = start_time.elapsed().as_millis();
                    let mut builder = Response::builder()
                        .status(resp_parts.status)
                        .header("X-Cache", "MISS")
                        .header("X-Edge-Pop", &self.pop_code)
                        .header("X-Edge-Latency-Ms", total_latency.to_string())
                        .header("X-Backend-Id", &backend.id);

                    for (k, v) in resp_parts.headers.iter() {
                        builder = builder.header(k, v);
                    }

                    self.metrics.add_bytes_served(resp_bytes.len() as u64);
                    return Ok(builder.body(Full::new(resp_bytes)).unwrap());
                }
                Err(err) => {
                    backend.record_failure();
                    self.metrics.inc_backend_failure();
                    last_err = format!("Backend dispatch error: {}", err);
                    warn!(backend = %backend.id, attempt, error = %err, "Failover triggered");
                }
            }
        }

        // Return 502 Bad Gateway if all backends or retries failed
        error!(error = %last_err, "All upstream backends failed to serve request");
        let err_response = Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .header("X-Edge-Pop", &self.pop_code)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(format!(
                r#"{{"error":"Bad Gateway","detail":"{}","pop":"{}"}}"#,
                last_err, self.pop_code
            ))))
            .unwrap();

        Ok(err_response)
    }
}
