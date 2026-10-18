use crate::config::GeoClusterConfig;
use crate::error::Result;
use crate::geo::node::{EdgeNode, EdgeNodeConfig};
use crate::geo::ring::GeoHashRing;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClusterHeartbeat {
    pub pop_code: String,
    pub addr: String,
    pub load_factor: f64,
    pub timestamp_epoch_sec: u64,
    pub is_healthy: bool,
}

pub struct GeoCluster {
    local_pop: Arc<EdgeNode>,
    ring: Arc<GeoHashRing>,
    replication_factor: usize,
}

impl GeoCluster {
    pub fn new(config: &GeoClusterConfig) -> Result<Self> {
        let ring = Arc::new(GeoHashRing::new(config.vnodes_per_node));

        let local_node = EdgeNode::new(EdgeNodeConfig {
            id: format!("pop-{}", config.local_pop_code.to_lowercase()),
            pop_code: config.local_pop_code.clone(),
            addr: format!("127.0.0.1"),
            latitude: config.latitude,
            longitude: config.longitude,
            weight: 100,
        });

        ring.add_node(local_node.clone());

        for peer in &config.peers {
            let peer_node = EdgeNode::new(EdgeNodeConfig {
                id: format!("pop-{}", peer.pop_code.to_lowercase()),
                pop_code: peer.pop_code.clone(),
                addr: peer.addr.clone(),
                latitude: peer.latitude,
                longitude: peer.longitude,
                weight: peer.weight,
            });
            ring.add_node(peer_node);
        }

        info!(
            local_pop = config.local_pop_code.as_str(),
            total_pops = ring.node_count(),
            vnodes = ring.vnode_count(),
            "Initialized GeoCluster topology"
        );

        Ok(Self {
            local_pop: local_node,
            ring,
            replication_factor: config.replication_factor,
        })
    }

    pub fn ring(&self) -> Arc<GeoHashRing> {
        self.ring.clone()
    }

    pub fn local_pop(&self) -> Arc<EdgeNode> {
        self.local_pop.clone()
    }

    pub fn replication_factor(&self) -> usize {
        self.replication_factor
    }

    pub fn handle_heartbeat(&self, hb: ClusterHeartbeat) {
        let node_id = format!("pop-{}", hb.pop_code.to_lowercase());
        if let Ok(node) = self.ring.get_node_by_key(&node_id) {
            node.set_healthy(hb.is_healthy);
        }
    }
}
