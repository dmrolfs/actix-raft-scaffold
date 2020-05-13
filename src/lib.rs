#![feature(try_trait)]

use serde::{Serialize, Deserialize};
use actix_raft::NodeId;

// pub mod api {
//     pub mod cluster {
//         tonic::include_proto!("api.cluster");
//     }
// }

// pub mod cluster;
pub mod config;
pub mod fib;
pub mod ports;
pub mod network;
pub mod storage;
pub mod raft;
pub mod ring; //todo move into private and support tailoring via configuration

pub use self::config::{Configuration, ConfigurationError, JoinStrategy};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub name: String,
    pub cluster_address: String,
    pub app_address: String,
    pub public_address: String,
}

impl NodeInfo {
    pub fn node_id(&self) -> NodeId { utils::generate_node_id(self.cluster_address.as_str()) }
}

impl Default for NodeInfo {
    fn default() -> Self {
        NodeInfo {
            name: "".to_owned(),
            cluster_address: "".to_owned(),
            app_address: "".to_owned(),
            public_address: "".to_owned(),
        }
    }
}

impl Into<std::collections::HashMap<String, String>> for NodeInfo {
    fn into(self) -> std::collections::HashMap<String, String> {
        let mut table = std::collections::HashMap::new();
        table.insert("name".to_string(), self.name);
        table.insert("clusterAddress".to_string(), self.cluster_address);
        table.insert("appAddress".to_string(), self.app_address);
        table.insert("publicAddress".to_string(), self.public_address);
        table
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