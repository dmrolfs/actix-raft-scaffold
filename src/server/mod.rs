use actix::prelude::*;
use crate::network::Network;
use super::fib::FibActor;

pub mod http;

pub struct ServerData {
    pub fib: Addr<FibActor>,
    pub network: Addr<Network>,
}
