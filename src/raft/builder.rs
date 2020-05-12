use actix::prelude::*;
use actix::dev::ToEnvelope;
use actix_raft::{
    NodeId,
    AppData, AppDataResponse, AppError, RaftStorage,
};
use crate::network::Network;
use crate::storage::StorageFactory;
use crate::{Configuration, ConfigurationError};
use crate::raft::Raft;

pub struct RaftBuilder<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E, Actor = S, Context = Context<S>>,
        S: Actor<Context = Context<S>>,
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
        S: ToEnvelope<S, actix_raft::storage::GetInitialState<E>>,
        S: ToEnvelope<S, actix_raft::storage::SaveHardState<E>>,
        S: ToEnvelope<S, actix_raft::storage::GetLogEntries<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::AppendEntryToLog<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::ReplicateToLog<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
        S: ToEnvelope<S, actix_raft::storage::ReplicateToStateMachine<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::CreateSnapshot<E>>,
        S: ToEnvelope<S, actix_raft::storage::InstallSnapshot<E>>,
        S: ToEnvelope<S, actix_raft::storage::GetCurrentSnapshot<E>>,


// S: RaftStorage<D, R, E, Actor = Self> + Actor<Context = Context<S>>,
        // S: RaftStorage<D, R, E> + Actor<Context = Context<S>>,
{
    id: NodeId,
    seed_members: Vec<NodeId>,
    network: Option<Addr<Network<D, R, E, S>>>,
    config: Option<Configuration>,
    storage_factory: Option<Box<dyn StorageFactory<D, R, E, S>>>,
}

impl<D, R, E, S> RaftBuilder<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E, Actor = S, Context = Context<S>>,
        S: std::fmt::Debug,
        S: Actor<Context = Context<S>>,
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
        S: ToEnvelope<S, actix_raft::storage::GetInitialState<E>>,
        S: ToEnvelope<S, actix_raft::storage::SaveHardState<E>>,
        S: ToEnvelope<S, actix_raft::storage::GetLogEntries<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::AppendEntryToLog<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::ReplicateToLog<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::ApplyEntryToStateMachine<D, R, E>>,
        S: ToEnvelope<S, actix_raft::storage::ReplicateToStateMachine<D, E>>,
        S: ToEnvelope<S, actix_raft::storage::CreateSnapshot<E>>,
        S: ToEnvelope<S, actix_raft::storage::InstallSnapshot<E>>,
        S: ToEnvelope<S, actix_raft::storage::GetCurrentSnapshot<E>>,



        // S: RaftStorage<D, R, E, Actor = Self> + Actor<Context = Context<S>>,
        // S: RaftStorage<D, R, E> + Actor<Context = Context<S>>,
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

    pub fn with_storage_factory(
        &mut self,
        factory: Box<dyn StorageFactory<D, R, E, S>>
    ) -> &mut Self {
        self.storage_factory = Some(factory);
        self
    }

    // #[tracing::instrument(skip(self))]
    pub fn build(&self) -> Result<Addr<Raft<D, R, E, S>>, ConfigurationError> {
        match self.check_requirements() {
            Ok(_) => {
                let raft_config = self.config.as_ref()?.clone().into();

                let local_id = self.id;
                let storage = self.storage_factory.as_ref()?.create();
                let network = self.build_network_if_needed();
                let metrics_recipient = network.clone().recipient();

                let raft = Raft::create(move |_| {
                    let r = Raft::new(
                        local_id,
                        raft_config,
                        network,
                        storage,
                        metrics_recipient,
                    );

                    r
                });

                Ok(raft)
            },

            Err(err) => Err(err),
        }
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
                let host_id = crate::utils::generate_node_id(host_info.cluster_address.as_str());

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