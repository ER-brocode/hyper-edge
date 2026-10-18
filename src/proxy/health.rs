use crate::proxy::backend::{Backend, BackendState};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::sleep;
use tracing::{info, warn};

pub struct HealthProber {
    backend: Arc<Backend>,
    health_path: String,
    interval: Duration,
    timeout: Duration,
}

impl HealthProber {
    pub fn new(
        backend: Arc<Backend>,
        health_path: String,
        interval: Duration,
        timeout: Duration,
    ) -> Self {
        Self {
            backend,
            health_path,
            interval,
            timeout,
        }
    }

    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let start = Instant::now();
                let probe_result = tokio::time::timeout(
                    self.timeout,
                    TcpStream::connect(&self.backend.addr),
                )
                .await;

                let latency = start.elapsed().as_millis() as u64;

                match probe_result {
                    Ok(Ok(_stream)) => {
                        let prev_state = self.backend.state();
                        self.backend.record_success(latency);
                        let curr_state = self.backend.state();

                        if prev_state != curr_state && curr_state == BackendState::Healthy {
                            info!(
                                backend_id = self.backend.id.as_str(),
                                addr = self.backend.addr.as_str(),
                                latency_ms = latency,
                                "Backend recovered to HEALTHY state"
                            );
                        }
                    }
                    Ok(Err(err)) => {
                        let prev_state = self.backend.state();
                        self.backend.record_failure();
                        let curr_state = self.backend.state();

                        if prev_state != curr_state {
                            warn!(
                                backend_id = self.backend.id.as_str(),
                                addr = self.backend.addr.as_str(),
                                error = %err,
                                new_state = ?curr_state,
                                "Backend degraded/unhealthy after connection error"
                            );
                        }
                    }
                    Err(_) => {
                        let prev_state = self.backend.state();
                        self.backend.record_failure();
                        let curr_state = self.backend.state();

                        if prev_state != curr_state {
                            warn!(
                                backend_id = self.backend.id.as_str(),
                                addr = self.backend.addr.as_str(),
                                timeout_ms = self.timeout.as_millis(),
                                new_state = ?curr_state,
                                "Backend degraded/unhealthy after probe timeout"
                            );
                        }
                    }
                }

                sleep(self.interval).await;
            }
        })
    }
}
