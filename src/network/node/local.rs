use actix::prelude::*;
use actix_raft::{
    NodeId,
    AppData, AppDataResponse, AppError, storage::RaftStorage,
    admin as raft_admin_protocol,
};
use tracing::*;
use crate::raft::Raft;
use super::{Node, NodeError};
use crate::NodeInfo;
use crate::network::messages;
use crate::network::proximity::{ProximityBehavior, ChangeClusterBehavior, ConnectionBehavior};

pub struct LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    pub id: NodeId,
    pub raft: Addr<Raft<D, R, E, S>>,
}

impl<D, R, E, S> LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(raft))]
    pub fn new(id: NodeId, raft: Addr<Raft<D, R, E, S>>) -> Self {
        Self { id, raft }
    }
}

impl<D, R, E, S> std::clone::Clone for LocalNode<D, R, E, S>
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
        }
    }
}

impl<D, R, E, S> std::fmt::Debug for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "LocalNode#{}", self.id) }
}

impl<D, R, E, S> std::fmt::Display for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "LocalNode#{}", self.id) }
}

impl<D, R, E, S> ProximityBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{}

impl<D, R, E, S> ChangeClusterBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(ctx))]
    fn change_cluster_config(
        &self,
        add_members: Vec<NodeId>,
        remove_members: Vec<NodeId>,
        ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<(), NodeError> {
        let _node_addr = ctx.address();
        info!(proximity = ?self, ?add_members, ?remove_members, "Changing cluster config.");
        let proximity_rep = std::rc::Rc::new(format!("{:?}", self));
        let prep_1 = proximity_rep.clone();
        let prep_2 = proximity_rep.clone();
        let proposal = raft_admin_protocol::ProposeConfigChange::<D, R, E>::new(
            add_members.clone(),
            remove_members.clone()
        );

        let task = fut::wrap_future(
            self.raft.send(proposal)
                .map_err(move |err| {
                    error!(
                        proximity = ?prep_1, error = ?err,
                        "Failed to call Node actor to propose config changes."
                    );

                    ()
                })
                .and_then(move |res| {
                    match res {
                        Ok(_) => {
                            info!(
                                proximity = ?prep_2, ?add_members, ?remove_members,
                                 "Raft completed cluster config change."
                            );

                            Ok(())
                        },

                        Err(err) => {
                            error!(
                                proximity = ?prep_2, error = ?err,
                                "Failed to call Node actor to propose config changes."
                            );

                            Err(())
                        }
                    }
                })
        );

        ctx.spawn(task);

        Ok(())
    }
}

impl<D, R, E, S> ConnectionBehavior<D> for LocalNode<D, R, E, S>
    where
        D: AppData,
        R: AppDataResponse,
        E: AppError,
        S: RaftStorage<D, R, E>,
{
    #[tracing::instrument(skip(self, local_id_info, _ctx))]
    fn connect(
        &self,
        local_id_info: (NodeId, &NodeInfo),
        _ctx: &mut <Node<D> as Actor>::Context
    ) -> Result<messages::Acknowledged, NodeError> {
        info!(proximity = ?self, local_id = local_id_info.0, "LocalNode connected");
        Ok(messages::Acknowledged {})
    }

    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&self, _ctx: &mut <Node<D> as Actor>::Context) -> Result<(), NodeError> {
        info!(proximity = ?self, "LocalNode disconnected");
        Ok(())
    }
}
