use actix::prelude::*;
use super::fib::FibActor;

pub mod http;

pub struct ServerData {
    pub fib: Addr<FibActor>
}
