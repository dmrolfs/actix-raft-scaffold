use actix::prelude::*;
use actix::dev::ToEnvelope;
use actix_raft::{
    NodeId,
    AppData, AppDataResponse, AppError, RaftStorage,
};
use super::Raft;
use thiserror::Error;
use tracing::*;
use crate::{
    NodeInfo,
    Configuration, ConfigurationError,
    network::{Network, BindEndpoint},
    ports::PortData,
    ring::Ring,
    storage::StorageFactory,
    utils,
};
use std::option::NoneError;
use crate::network::RegisterRaft;

#[derive(Error, Debug)]
pub enum RaftSystemError {
    #[error("application system misconfigured")]
    Configuration(#[from] crate::ConfigurationError),

    #[error("application system failed to start host network")]
    NetworkError(#[from] crate::network::NetworkError),

    #[error("failed in actor send")]
    MailboxError(#[from] actix::MailboxError),

    #[error("Raft system builder not properly set before build: {0}")]
    BuilderError(String),

    #[error("unknown system error: {0}")]
    Unknown(String),
}

impl From<std::option::NoneError> for RaftSystemError {
    fn from(that: NoneError) -> Self {
        let desc = format!("{:?}", that);
        RaftSystemError::BuilderError(desc)
    }
}

pub struct RaftSystem<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    pub id: NodeId,
    pub raft: Addr<Raft<D, R, E, S>>,
    pub network: Addr<Network<D, R, E, S>>,
    pub info: NodeInfo,
    pub configuration: Configuration,
}

impl<D, R, E, S> std::fmt::Debug for RaftSystem<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "System(id:{}, info:{:?}, configuration:{:?})",
            self.id, self.info, self.configuration
        )
    }
}

impl<D, R, E, S> RaftSystem<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
//     #[tracing::instrument]
//     pub fn new() -> Result<RaftSystem<D, R, E, S>,  RaftSystemError> {
//         let config = Configuration::load()?;
//         info!("configuration = {:?}", config);
//         // let config = match Configuration::load() {
//         //     Ok(c) => c,
//         //     Err(err) => {
//         //         // return futures::failed( actix::MailboxError::Closed)
//         //         return futures::failed(err.into())
//         //     }
//         // };
//
//         let ring = Ring::new(config.ring_replicas);
//         let host_info = config.host_info();
//         info!("Host node info:${:?}", host_info);
//         let host_id = utils::generate_node_id(host_info.cluster_address.as_str());
//
//         let network_arb = Arbiter::new();
//         // let d = config.discovery_host;
//
//         let mut cluster_network = Network::new(
//             host_id,
//             &host_info,
//             ring,
//             config.discovery_host,
//         );
//         cluster_network.configure_with(&config);
//         let cluster_network_addr = Network::start_in_arbiter(&network_arb, |_| cluster_network);
//
//         Ok(RaftSystem {
//             id: host_id,
//             network: cluster_network_addr,
//             configuration: config,
//             info: host_info,
//         })
//     }

    #[tracing::instrument]
    // pub fn start(&self) -> Result<(), RaftSystemError> {
    pub fn start(&self, data: PortData<D, R, E, S>) -> Result<(), RaftSystemError> {
        self.network
            .send( BindEndpoint::new(data))
            .map_err(|err| RaftSystemError::MailboxError(err))
            .and_then(|res| { res.map_err(RaftSystemError::from) })
            .wait()
    }
}


pub struct RaftBuilder<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,

        S: RaftStorage<D, R, E>,
        S: std::fmt::Debug,
        S: Actor,
        S: Handler<actix_raft::storage::GetInitialState<E>>,
        S: Handler<actix_raft::storage::SaveHardState<E>>,
        S: Handler<actix_raft::storage::GetLogEntries<D, E>>,
        S: Handler<actix_raft::storage::AppendEntryToLog<D, E>>,
        S: Handler<actix_raft::storage::ReplicateToLog<D, E>>,
        S: Handler<actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
        S: Handler<actix_raft::storage::ReplicateToStateMachine<D, E>>,
        S: Handler<actix_raft::storage::CreateSnapshot<E>>,
        S: Handler<actix_raft::storage::InstallSnapshot<E>>,
        S: Handler<actix_raft::storage::GetCurrentSnapshot<E>>,
{
    id: NodeId,
    seed_members: Vec<NodeId>,
    network: Option<Addr<Network<D, R, E, S>>>,
    config: Option<Configuration>,
    storage_factory: Option<Box<dyn StorageFactory<D, R, E, S>>>,
}

impl<D, R, E, S> std::fmt::Debug for RaftBuilder<D, R, E, S>
where
    D: AppData,
    R: AppDataResponse,
    E: AppError,
    S: RaftStorage<D, R, E>,
    S: std::fmt::Debug,
    S: Handler<actix_raft::storage::GetInitialState<E>>,
    S: Handler<actix_raft::storage::SaveHardState<E>>,
    S: Handler<actix_raft::storage::GetLogEntries<D, E>>,
    S: Handler<actix_raft::storage::AppendEntryToLog<D, E>>,
    S: Handler<actix_raft::storage::ReplicateToLog<D, E>>,
    S: Handler<actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
    S: Handler<actix_raft::storage::ReplicateToStateMachine<D, E>>,
    S: Handler<actix_raft::storage::CreateSnapshot<E>>,
    S: Handler<actix_raft::storage::InstallSnapshot<E>>,
    S: Handler<actix_raft::storage::GetCurrentSnapshot<E>>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RaftBuilder(id:{:?}, seed_members:{:?}, is_network_set:{}, is_storage_factory_set:{}, configuration:{:?})",
            self.id, self.seed_members, self.network.is_some(), self.storage_factory.is_some(), self.config
        )
    }
}

impl<D, R, E, S> RaftBuilder<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,

        S: RaftStorage<D, R, E>,
        S: std::fmt::Debug,

        S: Handler<actix_raft::storage::GetInitialState<E>>,
        S: Handler<actix_raft::storage::SaveHardState<E>>,
        S: Handler<actix_raft::storage::GetLogEntries<D, E>>,
        S: Handler<actix_raft::storage::AppendEntryToLog<D, E>>,
        S: Handler<actix_raft::storage::ReplicateToLog<D, E>>,
        S: Handler<actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
        S: Handler<actix_raft::storage::ReplicateToStateMachine<D, E>>,
        S: Handler<actix_raft::storage::CreateSnapshot<E>>,
        S: Handler<actix_raft::storage::InstallSnapshot<E>>,
        S: Handler<actix_raft::storage::GetCurrentSnapshot<E>>,
{
    pub fn new(id: NodeId) -> RaftBuilder<D, R, E, S> {
        Self {
            id,
            seed_members: Vec::new(),
            network: None,
            config: None,
            storage_factory: None,
        }
    }

    pub fn with_seed_members(&mut self, seed_members: &Vec<NodeId>) -> &mut Self {
        self.seed_members = seed_members.clone();
        self
    }

    pub fn push_seed_member(&mut self, seed_member: NodeId) -> &mut Self {
        self.seed_members.push(seed_member);
        self
    }

    pub fn with_configuration(&mut self, config: &Configuration) -> &mut Self {
        self.config = Some(config.clone());
        self
    }

    pub fn with_network(&mut self, network: Addr<Network<D, R, E, S>>) -> &mut Self {
        self.network = Some(network);
        self
    }

    pub fn with_storage_factory<F>(
        &mut self,
        factory: F
    ) -> &mut Self
    where
        F: StorageFactory<D, R, E, S> + 'static,
    {
        self.storage_factory = Some(Box::new(factory));
        self
    }

    // #[tracing::instrument(skip(self))]
    #[tracing::instrument]
    pub fn build(&self) -> Result<RaftSystem<D, R, E, S>, RaftSystemError> {
        self.check_requirements()?;

        let c = self.config.as_ref()?;
        let raft_config = c.clone().into();

        let host_id = self.id;

        let host = c.host.clone();
        let host_info = self.config.as_ref()?.seed_nodes.get(&host)?.clone();

        let storage = self.storage_factory.as_ref()?.create();
        let network = self.build_network_if_needed();
        let host_network = network.clone();
        let metrics_recipient = network.clone().recipient();

        let raft = Raft::create(move |_| {
            Raft::new(
                host_id,
                raft_config,
                host_network,
                storage,
                metrics_recipient,
            )
        });

        info!(network_id = host_id, "Registering Raft actor with host Network...");
        network.send( RegisterRaft(raft.clone()))
            .map_err( RaftSystemError::from)
            .and_then(|res| {
                info!(network_id = host_id, "Result of Raft registration: {:?}", res);
                res.map_err(RaftSystemError::from)
            })
            .map(|_| {
                RaftSystem {
                    id: host_id,
                    raft,
                    network,
                    info: host_info,
                    configuration: c.clone(),
                }
            })
            .wait()
    }

    fn check_requirements(&self) -> Result<(), ConfigurationError> {
        if self.storage_factory.is_none() {
            return Err(ConfigurationError::Unknown(
                "No means to build RaftStorage was provided".to_string()
            ));
        }

        Ok(())
    }

    fn build_network_if_needed(&self) -> Addr<Network<D, R, E, S>> {
        self.network
            .as_ref()
            .map(|n| n.clone())
            .unwrap_or_else(|| {
                let c = self.config.as_ref().unwrap();
                let ring = crate::ring::Ring::new(c.ring_replicas);
                let host_info = c.host_info();
                let host_id = host_info.node_id();

                let mut network = Network::new(
                    host_id,
                    &host_info,
                    ring,
                    c.discovery_host,
                );

                network.configure_with(&c);
                let network_arb = Arbiter::new();
                Network::start_in_arbiter(&network_arb, |_| network)
            })
    }
}

// Ok()

// match self.check_requirements() {
//     Ok(_) => {
//         let c = self.config.as_ref()?;
//         let raft_config = c.clone().into();
//
//         let host_id = self.id;
//
//         let host = c.host.clone();
//         let host_info = self.config.as_ref()?.seed_nodes.get(&host)?.clone();
//
//         let storage = self.storage_factory.as_ref()?.create();
//         let network = self.build_network_if_needed();
//         let host_network = network.clone();
//         let metrics_recipient = network.clone().recipient();
//
//         let raft = Raft::create(move |_| {
//             let r = Raft::new(
//                 host_id,
//                 raft_config,
//                 network,
//                 storage,
//                 metrics_recipient,
//             );
//
//             r
//         });
//
//         Ok(RaftSystem {
//             id: host_id,
//             raft,
//             network: host_network,
//             info: host_info,
//             configuration: c.clone(),
//         })
//     },
//
//     Err(err) => RaftSystemError::from(err),
// }
