use actix_raft::Raft as ActixRaft;
use crate::network::Network;

pub use self::system::RaftSystemBuilder;

pub mod system;
pub mod network;
mod builder;

pub type Raft<D, R, E, S> = ActixRaft<D, R, E, Network<D, R, E, S>, S>;
