use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use std::time::Duration;

pub struct StreamPipeline;

impl StreamPipeline {
    /// Collect hyper incoming body into contiguous bytes with maximum buffer protection
    pub async fn collect_body(body: Incoming, max_bytes: usize) -> Result<Bytes, String> {
        let mut collected = Vec::new();
        let mut stream = body;

        while let Some(frame_res) = stream.frame().await {
            let frame = frame_res.map_err(|e| format!("Body frame read error: {}", e))?;
            if let Ok(data) = frame.into_data() {
                if collected.len() + data.len() > max_bytes {
                    return Err(format!("Payload exceeds maximum buffer limit: {} bytes", max_bytes));
                }
                collected.extend_from_slice(&data);
            }
        }

        Ok(Bytes::from(collected))
    }

    /// Check if response headers indicate cacheability
    pub fn is_response_cacheable(status: StatusCode, headers: &HeaderMap) -> bool {
        if status != StatusCode::OK && status != StatusCode::NON_AUTHORITATIVE_INFORMATION {
            return false;
        }

        if let Some(cc) = headers.get("cache-control") {
            if let Ok(cc_str) = cc.to_str() {
                let cc_lower = cc_str.to_lowercase();
                if cc_lower.contains("no-store") || cc_lower.contains("private") {
                    return false;
                }
            }
        }

        true
    }

    /// Parse max-age from Cache-Control header if present
    pub fn parse_max_age(headers: &HeaderMap) -> Option<Duration> {
        if let Some(cc) = headers.get("cache-control") {
            if let Ok(cc_str) = cc.to_str() {
                for part in cc_str.split(',') {
                    let trimmed = part.trim();
                    if trimmed.starts_with("max-age=") {
                        if let Ok(secs) = trimmed["max-age=".len()..].parse::<u64>() {
                            return Some(Duration::from_secs(secs));
                        }
                    }
                }
            }
        }
        None
    }
}
