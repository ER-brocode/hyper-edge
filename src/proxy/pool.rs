use crate::error::{ProxyError, Result};
use crate::proxy::backend::Backend;
use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, warn};

type Http1Sender = http1::SendRequest<Full<Bytes>>;

struct PooledConnection {
    sender: Http1Sender,
    created_at: Instant,
}

pub struct BackendConnectionPool {
    backend: Arc<Backend>,
    idle_pool: Mutex<VecDeque<PooledConnection>>,
    max_idle: usize,
    idle_timeout: Duration,
    connect_timeout: Duration,
}

impl BackendConnectionPool {
    pub fn new(backend: Arc<Backend>, max_idle: usize, idle_timeout: Duration) -> Self {
        Self {
            backend,
            idle_pool: Mutex::new(VecDeque::new()),
            max_idle: max_idle.max(2),
            idle_timeout,
            connect_timeout: Duration::from_secs(3),
        }
    }

    /// Acquire a live connection sender from pool or create a new TCP + HTTP handshake
    pub async fn get_sender(&self) -> Result<Http1Sender> {
        // Prune expired idle connections
        let now = Instant::now();
        {
            let mut pool = self.idle_pool.lock();
            pool.retain(|c| now.duration_since(c.created_at) < self.idle_timeout && c.sender.is_ready());
            if let Some(conn) = pool.pop_front() {
                if conn.sender.is_ready() {
                    return Ok(conn.sender);
                }
            }
        }

        // Establish new TCP stream to backend
        let tcp_stream = match timeout(self.connect_timeout, TcpStream::connect(&self.backend.addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => return Err(ProxyError::ConnectionFailed(format!("{}: {}", self.backend.addr, e)).into()),
            Err(_) => return Err(ProxyError::Timeout(self.connect_timeout).into()),
        };

        let io = TokioIo::new(tcp_stream);
        let (sender, conn) = match http1::handshake(io).await {
            Ok(res) => res,
            Err(e) => return Err(ProxyError::Protocol(format!("HTTP handshake failed: {}", e)).into()),
        };

        // Spawn driver task for hyper HTTP/1 connection
        tokio::spawn(async move {
            if let Err(err) = conn.await {
                debug!("Backend HTTP connection ended: {:?}", err);
            }
        });

        Ok(sender)
    }

    /// Return an active connection to the idle pool for reuse
    pub fn return_sender(&self, sender: Http1Sender) {
        if !sender.is_ready() {
            return;
        }

        let mut pool = self.idle_pool.lock();
        if pool.len() < self.max_idle {
            pool.push_back(PooledConnection {
                sender,
                created_at: Instant::now(),
            });
        }
    }
}
