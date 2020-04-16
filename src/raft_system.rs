use actix::prelude::*;
use actix_raft::NodeId;
use thiserror::Error;
use tracing::*;
use crate::{
    Configuration, NodeInfo,
    network::{Network, BindEndpoint},
    ports::PortData,
    ring::Ring,
    utils,
};

#[derive(Error, Debug)]
pub enum RaftSystemError {
    #[error("application system misconfigured")]
    Configuration(#[from] crate::ConfigurationError),

    #[error("application system failed to start host network")]
    NetworkError(#[from] crate::network::NetworkError),

    #[error("failed in actor send")]
    MailboxError(#[from] actix::MailboxError),

    #[error("unknown system error")]
    Unknown,
}

pub struct RaftSystem {
    pub id: NodeId,
    // pub raft: Addr<RaftClient>,
    pub network: Addr<Network>,
    configuration: Configuration,
    info: NodeInfo,
}

impl std::fmt::Debug for RaftSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "System(id:{}, info:{:?}, configuration:{:?})",
            self.id, self.info, self.configuration
        )
    }
}

impl RaftSystem {
    #[tracing::instrument]
    pub fn new() -> Result<RaftSystem,  RaftSystemError> {
        let config = Configuration::load()?;
        info!("configuration = {:?}", config);
        // let config = match Configuration::load() {
        //     Ok(c) => c,
        //     Err(err) => {
        //         // return futures::failed( actix::MailboxError::Closed)
        //         return futures::failed(err.into())
        //     }
        // };

        let ring = Ring::new(config.ring_replicas);
        let host_info = config.host_info();
        info!("Host node info:${:?}", host_info);
        let host_id = utils::generate_node_id(host_info.cluster_address.as_str());

        let network_arb = Arbiter::new();
        // let d = config.discovery_host;

        let mut cluster_network = Network::new(
            host_id,
            &host_info,
            ring,
            config.discovery_host,
        );
        cluster_network.configure_with(&config);
        let cluster_network_addr = Network::start_in_arbiter(&network_arb, |_| cluster_network);

        Ok(RaftSystem {
            id: host_id,
            network: cluster_network_addr,
            configuration: config,
            info: host_info,
        })
    }

    #[tracing::instrument]
    // pub fn start(&self) -> Result<(), RaftSystemError> {
    pub fn start(&self, data: PortData) -> Result<(), RaftSystemError> {
        self.network
            .send( BindEndpoint::new(data))
            .map(|res| res.unwrap() )
            .map_err(RaftSystemError::from)
            .wait()
    }
}