use std::sync::{Arc, RwLock};
use hash_ring::HashRing;
use actix_raft::NodeId;

pub type RingType = Arc<RwLock<HashRing<NodeId>>>;

pub struct Ring;

impl Ring {
    /// Creates a new has ring with the specified nodes.
    /// replicas is the number of virtual nodes each node has to increase the distribution.
    pub fn new(replicas: isize) -> RingType {
        Arc::new(RwLock::new(HashRing::new(Vec::new(), replicas)))
    }
}
