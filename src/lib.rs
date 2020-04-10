use serde::{Serialize, Deserialize};

// pub mod api {
//     pub mod cluster {
//         tonic::include_proto!("api.cluster");
//     }
// }

// pub mod cluster;
pub mod config;
pub mod fib;
pub mod server;
pub mod network;
pub mod ring; //todo move into private and support tailoring via configuration

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeInfo {
    name: String,
    cluster_addr: String,
    app_addr: String,
    ui_addr: String,
}

impl Default for NodeInfo {
    fn default() -> Self {
        NodeInfo {
            name: "".to_owned(),
            cluster_addr: "".to_owned(),
            app_addr: "".to_owned(),
            ui_addr: "".to_owned(),
        }
    }
}

pub type NodeList = Vec<NodeInfo>;

// pub mod hash_ring {
//     use std::sync::{Arc, RwLock};
//     use hash_ring::HashRing;
//     use actix_raft::NodeId;
//
//     pub type RingType = Arc<RwLock<HashRing<NodeId>>>;
//
//     pub struct Ring;
//
//     impl Ring {
//         /// Creates a new has ring with the specified nodes.
//         /// replicas is the number of virtual nodes each node has to increase the distribution.
//         pub fn new(replicas: isize) -> RingType {
//             Arc::new(RwLock::new(HashRing::new(Vec::new(), replicas)))
//         }
//     }
// }

pub mod utils {
    use crypto::digest::Digest;
    use crypto::sha2::Sha256;

    pub fn generate_node_id<S>(address: S) -> u64
    where
        S: AsRef<str>,
    {
        let mut hasher = Sha256::new();
        hasher.input_str( address.as_ref() );

        let hash_prefix = &hasher.result_str()[..8];
        let mut buf: [u8; 8] = [0; 8];
        buf.copy_from_slice(hash_prefix.as_bytes());
        u64::from_be_bytes(buf)
    }
}