pub mod cache;
pub mod config;
pub mod error;
pub mod geo;
pub mod metrics;
pub mod proxy;

pub use cache::EdgeCache;
pub use config::EdgeConfig;
pub use error::{HyperEdgeError, Result};
pub use geo::{GeoCluster, GeoHashRing};
pub use metrics::EdgeMetrics;
pub use proxy::{Backend, HealthProber, ProxyRouter};

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{error, info};

/// Start the hyper-edge daemon with graceful shutdown listener
pub async fn start_edge_server(
    config: EdgeConfig,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let metrics = EdgeMetrics::new();
    let cache = EdgeCache::new(&config.cache, metrics.clone());

    // Start background cache janitor for TTL reclamation
    EdgeCache::start_janitor(cache.clone(), config.cleanup_interval());

    // Initialize geo cluster topology
    let cluster = Arc::new(GeoCluster::new(&config.geo)?);

    // Initialize backends and start health probers
    let mut backends = Vec::new();
    for b_cfg in &config.backends {
        let backend = Backend::new(
            b_cfg.id.clone(),
            b_cfg.addr.clone(),
            b_cfg.weight,
            b_cfg.max_connections,
        );

        let prober = Arc::new(HealthProber::new(
            backend.clone(),
            b_cfg.health_path.clone(),
            std::time::Duration::from_secs(b_cfg.probe_interval_secs),
            std::time::Duration::from_secs(b_cfg.probe_timeout_secs),
        ));
        prober.start();

        backends.push(backend);
    }

    let router = Arc::new(ProxyRouter::new(
        backends,
        cache.clone(),
        metrics.clone(),
        config.geo.local_pop_code.clone(),
    ));

    let addr: SocketAddr = config.socket_addr();
    let listener = TcpListener::bind(addr).await?;
    info!(
        addr = %addr,
        pop = config.geo.local_pop_code.as_str(),
        "HyperEdge Anycast Reverse Proxy listening"
    );

    let auto_server = auto::Builder::new(TokioExecutor::new());

    loop {
        tokio::select! {
            accept_res = listener.accept() => {
                let (stream, remote_addr) = match accept_res {
                    Ok(res) => res,
                    Err(e) => {
                        error!("TCP accept error: {}", e);
                        continue;
                    }
                };

                metrics.inc_active_connections();
                let io = TokioIo::new(stream);
                let router_clone = router.clone();
                let metrics_clone = metrics.clone();
                let server_builder = auto_server.clone();

                tokio::spawn(async move {
                    let service = service_fn(move |req| {
                        let r = router_clone.clone();
                        async move {
                            r.handle_request(req).await
                        }
                    });

                    if let Err(err) = server_builder.serve_connection_with_upgrades(io, service).await {
                        // Connection closed or client disconnected
                        tracing::trace!(remote = %remote_addr, "Connection closed: {:?}", err);
                    }
                    metrics_clone.dec_active_connections();
                });
            }
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Graceful shutdown signal received. Stopping listener...");
                    break;
                }
            }
        }
    }

    info!("HyperEdge server stopped cleanly.");
    Ok(())
}
