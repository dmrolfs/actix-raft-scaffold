use serde::{Serialize, Deserialize};

// pub mod api {
//     pub mod cluster {
//         tonic::include_proto!("api.cluster");
//     }
// }

// pub mod cluster;
pub mod fib;
pub mod server;
pub mod network;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeInfo {
    pub cluster_addr: String,
    pub app_addr: String,
    pub ui_addr: String,
}

impl Default for NodeInfo {
    fn default() -> Self {
        NodeInfo {
            cluster_addr: "".to_owned(),
            app_addr: "".to_owned(),
            ui_addr: "".to_owned(),
        }
    }
}

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