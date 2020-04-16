use serde::{Serialize, Deserialize};


mod network;
pub mod node;

pub use network::*;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum NetworkState {
    Initialized,
    SingleNode,
    Cluster,
}
