use actix::prelude::*;
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
    storage::StorageFactory,
};
use std::option::NoneError;
use crate::network::{RegisterRaft, DiscoverNodes};
use actix_raft::admin::InitWithConfig;

#[derive(Error, Debug)]
pub enum RaftSystemError {
    #[error("Raft initialization failed.")]
    RaftInitializationError(#[from] actix_raft::admin::InitWithConfigError ),

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


/////////////////////////////////////////////////////////////////////////
// Raft System

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

impl<D, R, E, S> Actor for RaftSystem<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    type Context = Context<Self>;

    #[tracing::instrument(skip(ctx))]
    fn started(&mut self, ctx: &mut Self::Context) {
        fut::wrap_future::<_, Self>(self.network.send(DiscoverNodes))
            .map_err(|err, s, _| {
                error!(
                    network_id = s.id, error = ?err,
                    "Network actor failed for DiscoverNodes command"
                );

                crate::network::NetworkError::from(err)
            })
            .and_then(|res, _, _| { fut::result(res) })
            .map_err(|err, s, _| {
                error!(network_id = s.id, error = ?err, "Network failed to discover nodes");
                ()
            })
            .and_then(|seed_members, system, _| {
                info!(
                    network_id = system.id, ?seed_members,
                    "Initializing Raft system with seed members"
                );

                fut::wrap_future::<_, Self>(system.raft.send(InitWithConfig::new(seed_members)))
                    .map_err(|err, system, _| {
                        error!(
                            network_id = system.id, error = ?err,
                            "Raft actor failure on InitWithConfig"
                        );

                        ()
                    })
                    .and_then(|res, s, _| {
                        match res {
                            Ok(_) => {
                                info!(network_id = s.id, "Raft initialization complete.");
                                fut::ok(())
                            },

                            Err(err) => {
                                error!(network_id = s.id, error = ?err, "Raft InitWithConfig failed.");
                                fut::err(())
                            }
                        }
                    })
            })
            .spawn(ctx);
    }
}

// impl<D, R, E, S> RaftSystem<D, R, E, S>
//     where
//         D: AppData,
//         R: AppDataResponse,
//         E: AppError,
//         S: RaftStorage<D, R, E>,
// {
//     #[tracing::instrument(skip(self))]
//     pub fn start(&self, seed_members: Vec<NodeId>) -> Result<(), RaftSystemError> {
//         // send via network?
//         self.raft.send( InitWithConfig::new(seed_members))
//             .map_err(|err| {
//                 error!(
//                     network_id = self.id, error = ?err,
//                     "Actor send error during Raft initialization."
//                 );
//                 RaftSystemError::MailboxError(err)
//             })
//             .and_then(|res| {
//                 res
//                     .map(|_| {
//                         info!(network_id = self.id, "Raft initialization completed.");
//                         ()
//                     })
//                     .map_err(|err| {
//                         error!(network_id = self.id, error = ?err, "Raft initialization failed.");
//                         RaftSystemError::from(err)
//                     })
//             })
//             .wait()
//     }
// }

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

impl<D, R, E, S> std::clone::Clone for RaftSystem<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            raft: self.raft.clone(),
            network: self.network.clone(),
            info: self.info.clone(),
            configuration: self.configuration.clone(),
        }
    }
}


/////////////////////////////////////////////////////////////////////////
// Raft System Builder

pub struct RaftSystemBuilder<D, R, E, S>
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
    network: Option<Addr<Network<D, R, E, S>>>,
    config: Option<Configuration>,
    storage_factory: Option<Box<dyn StorageFactory<D, R, E, S>>>,
    server_data: Option<PortData<D, R, E, S>>,
}

impl<D, R, E, S> std::fmt::Debug for RaftSystemBuilder<D, R, E, S>
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
            "RaftBuilder(id:{:?}, is_network_set:{}, is_storage_factory_set:{}, is_server_data_set:{}, configuration:{:?})",
            self.id,
            self.network.is_some(), self.storage_factory.is_some(), self.server_data.is_some(),
            self.config
        )
    }
}

impl<D, R, E, S> RaftSystemBuilder<D, R, E, S>
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
    pub fn new(id: NodeId) -> RaftSystemBuilder<D, R, E, S> {
        Self {
            id,
            network: None,
            config: None,
            storage_factory: None,
            server_data: None,
        }
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

    //todo: generalize to consider how to incorporate application concerns beywond raft admin
    // current thinking is separate endpoint for app, so this binding would be for raft system only.
    // todo: even needed since network is provided or created?
    pub fn bind_with( &mut self, server_data: PortData<D, R, E, S>) -> &mut Self {
        self.server_data = Some(server_data);
        self
    }

    #[tracing::instrument]
    pub fn build(&self) -> Result<RaftSystem<D, R, E, S>, RaftSystemError> {
        self.check_requirements()?;

        let c = self.config.as_ref()?;
        let network = self.build_network_if_needed();
        let network2 = network.clone();
        let raft = self.build_raft(network.clone())?;
        let raft2 = raft.clone();
        let host_id = self.id;

        debug!("Binding raft endpoint...");
        let system = self.bind_raft_endpoint(&network)
            .and_then(move |_| {
                debug!("Raft endpoint binding completed. Registering raft system with network...");
                self.register_raft_with_network(&network2, &raft2)
            })
            .and_then(move |_| {
                let host = c.host.clone();
                let host_info = self.config.as_ref()?.seed_nodes.get(&host)?.clone();

                Ok(
                    RaftSystem {
                        id: host_id,
                        raft,
                        network,
                        info: host_info,
                        configuration: c.clone(),
                    }
                )
            })
            .wait();

        debug!("Raft registration complete: {:?}", system);
        system
    }

    #[tracing::instrument(skip(self, network))]
    fn build_raft(
        &self,
        network: Addr<Network<D, R, E, S>>
    ) -> Result<Addr<Raft<D, R, E, S>>, RaftSystemError> {
        let c = self.config.as_ref()?;
        let raft_config = c.clone().into();
        let metrics_recipient = network.clone().recipient();
        let storage = self.storage_factory.as_ref()?.create();
        let host_id = self.id;

        Ok(Raft::create(move |_| {
            Raft::new(
                host_id,
                raft_config,
                network,
                storage,
                metrics_recipient,
            )
        }))
    }

    #[tracing::instrument(skip(self, network, raft))]
    fn register_raft_with_network(
        &self,
        network: &Addr<Network<D, R, E, S>>,
        raft: &Addr<Raft<D, R, E, S>>
    ) -> impl Future<Item = (), Error = RaftSystemError> {
        let host_id = self.id;
        info!(network_id = host_id, "Registering Raft actor with host Network...");
        network.send( RegisterRaft(raft.clone()))
            .map_err( RaftSystemError::from)
            .and_then(move |res| {
                info!(network_id = host_id, "Result of Raft registration: {:?}", res);
                res.map_err(RaftSystemError::from)
            })
    }

    #[tracing::instrument(skip(self, network))]
    fn bind_raft_endpoint(
        &self,
        network: &Addr<Network<D, R, E, S>>
    ) -> impl Future<Item = (), Error = RaftSystemError> {
        let host_id = self.id;
        let data = self.server_data.as_ref()
            .map(|data| data.clone())
            .unwrap_or_else(move || {
            info!(network_id = host_id, "no Raft port data provided, using default.");
            let fib_act = crate::fib::FibActor::new();
            let fib_addr = fib_act.start();

            PortData {
                network: network.clone(),
                fib: fib_addr,
            }
        });

        info!(
            network_id = self.id,
            endpoint_address = self.config.as_ref().unwrap().seed_nodes.get(&(self.config.as_ref().unwrap().host)).unwrap().cluster_address.as_str(),
            "Binding raft service endpoint..."
        );

        let network2 = network.clone();

        // debug!("entering delay...");
        // tokio::run(
        //     tokio::timer::Delay::new(std::time::Instant::now() + std::time::Duration::from_millis(1))
        //         .map_err(move |err| {
        //             error!(network_id = host_id, error = ?err, "Error in delay.");
        //             ()
        //         })
        // );
        // debug!("... exiting delay");

        network2.send(BindEndpoint::new(data))
            .map_err(|err| RaftSystemError::MailboxError(err))
            .and_then(move |res| {
                debug!(network_id = host_id, "Raft endpoint binding complete.");
                res.map_err(RaftSystemError::from)
            })
    }

    fn check_requirements(&self) -> Result<(), ConfigurationError> {
        if self.config.is_none() {
            return Err(ConfigurationError::Unknown(
                "No means to configuration Raft system was provided".to_string()
            ));
        }

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
