use std::fmt::{Debug, Display};
use actix::prelude::*;
use actix_raft::NodeId;
use crate::NodeInfo;
use super::{Node, NodeError};
use crate::ports::http::entities::NodeInfoMessage;
use crate::network::messages;
use crate::ports::http::entities::{self, change_cluster_membership_response as entities_response};

pub trait ChangeClusterBehavior {
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError>;
}

pub trait ConnectionBehavior {
    fn remote_id(&self) -> NodeId;

    //todo: make nonblocking because of distributed network calls
    #[tracing::instrument(skip(self, _ctx))]
    fn connect(
        &mut self,
        local_id: NodeId,
        _local_info: &NodeInfo,
        _ctx: &mut <Node as Actor>::Context
    ) -> Result<messages::ClusterMembershipChange, NodeError> {
        Ok(messages::ClusterMembershipChange {
            node_id: Some(local_id.into()),
            action: messages::MembershipAction::Joining,
        })
    }

    //todo: make nonblocking because of distributed network calls
    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&mut self, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        Ok(())
    }
}


pub trait ProximityBehavior :
ChangeClusterBehavior +
ConnectionBehavior +
Debug + Display
{ }


#[derive(Debug)]
pub struct LocalNode {
    id: NodeId,
}

impl LocalNode {
    #[tracing::instrument]
    pub fn new(id: NodeId) -> LocalNode { LocalNode { id } }
}

impl Display for LocalNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "LocalNode#{}", self.id) }
}

impl ProximityBehavior for LocalNode {}

impl ChangeClusterBehavior for LocalNode {
    #[tracing::instrument]
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError> {
        let _node_addr = ctx.address();

        unimplemented!()
    }
}

impl ConnectionBehavior for LocalNode {
    fn remote_id(&self) -> NodeId { self.id }
}

pub struct RemoteNode {
    remote_id: NodeId,
    scope: String,
}

impl RemoteNode {
    #[tracing::instrument]
    pub fn new<S: AsRef<str> + Debug>(remote_id: NodeId, discovery_address: S) -> RemoteNode {
        //todo: use encryption
        let scope = format!("http://{}/api/cluster", discovery_address.as_ref());
        RemoteNode { remote_id, scope, }
    }
}

impl Display for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode#{}(to:{})", self.remote_id, self.scope)
    }
}

impl Debug for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode#{}(to:{})", self.remote_id, self.scope)
    }
}

impl ProximityBehavior for RemoteNode {}

impl ChangeClusterBehavior for RemoteNode {
    #[tracing::instrument]
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError> {

        // client.put(cluster_nodes_route)
        //     .header("Content-Type", "application/json")
        //     .send_json(&act.id)
        unimplemented!()
    }
}


impl ConnectionBehavior for RemoteNode {
    fn remote_id(&self) -> NodeId { self.remote_id }

    #[tracing::instrument(skip(self, _ctx))]
    fn connect(
        &mut self,
        local_id: NodeId,
        local_info: &NodeInfo,
        _ctx: &mut <Node as Actor>::Context
    ) -> Result<messages::ClusterMembershipChange, NodeError> {
        let register_node_route = format!("http://{}/api/cluster/nodes/{}", local_info.cluster_address, local_id);
        let body = NodeInfoMessage {
            node_id: Some(local_id.into()),
            node_info: Some(local_info.clone().into()),
        };

        reqwest::Client::new()
            .post(&register_node_route)
            .json(&body)
            .send()
            .map_err(|err| NodeError::RemoteNodeSendError(err.to_string()))
            .and_then(|mut res| {
                res.json::<entities::ChangeClusterMembershipResponse>()
                    .map_err(|err| NodeError::Unknown(err.to_string()))
                    .and_then(|cresp| {
                        if let Some(response) = cresp.response {
                            match response {
                                entities_response::Response::Result(cmc) => Ok(cmc.into()),

                                entities_response::Response::Failure(f) => {
                                    Err(NodeError::ResponseFailure(f.description))
                                }

                                entities_response::Response::CommandRejectedNotLeader(leader) => {
                                    Err(NodeError::RemoteNotLeaderError {
                                        leader_id: Some(leader.leader_id.into()),
                                        leader_address: Some(leader.leader_address.to_owned()),
                                    })
                                }
                            }
                        } else {
                            Err(NodeError::Unknown("good ChangeClusterResponse had empty response".to_string()))
                        }
                    })
            })
    }

    // #[tracing::instrument(skip(self, _ctx))]
    // fn connect(
    //     &mut self,
    //     local_id: NodeId,
    //     local_info: &NodeInfo,
    //     ctx: &mut <Node as Actor>::Context
    // ) -> Result<messages::ClusterMembershipChange, NodeError> {
    //     let register_node_route = format!("http://{}/api/cluster/nodes/{}", local_info.cluster_address, local_id);
    //     let body = NodeInfoMessage {
    //         node_id: Some(local_id.into()),
    //         node_info: Some(local_info.clone().into()),
    //     };
    //
    //
    //     let task = self.client.post(register_node_route)
    //         .timeout(std::time::Duration::from_secs(10))
    //         .send_json(&body)
    //         .map_err(|err| NodeError::RemoteNodeSendError(err.to_string()))
    //         .and_then(|resp| {
    //             resp.body()
    //                 .map_err(|err| NodeError::PayloadError(err.to_string()))
    //                 .and_then(|body| {
    //                     let change_resp = serde_json::from_slice::<entities::ChangeClusterMembershipResponse>(&body)
    //                         .map_err(|err| NodeError::Unknown(err.to_string())) // NodeError::from(err))
    //                         .and_then(|cresp| {
    //                             if let Some(response) = cresp.response {
    //                                 match response {
    //                                     entities_response::Response::Result(cmc) => Ok(cmc.into()),
    //
    //                                     entities_response::Response::Failure(f) => {
    //                                         Err(NodeError::ResponseFailure(f.description))
    //                                     }
    //
    //                                     entities_response::Response::CommandRejectedNotLeader(leader) => {
    //                                         Err(NodeError::RemoteNotLeaderError {
    //                                             leader_id: Some(leader.leader_id.into()),
    //                                             leader_address: Some(leader.leader_address.to_owned()),
    //                                         })
    //                                     }
    //                                 }
    //                             } else {
    //                                 Err(NodeError::Unknown("good ChangeClusterResponse had empty response".to_string()))
    //                             }
    //                         });
    //
    //                     change_resp
    //
    //                 })
    //         }).
    //
    //     Box::new(task)
    //
    //
    //
    //
    //     // let task = fut::wrap_future::<_, Node>(
    //     //     self.client.post(register_node_route)
    //     //         .header("Content-Type", "application/json")
    //     //         .send_json(&body)
    //     // )
    //     //     .map_err(|err, _, _| NodeError::from(err))
    //     //     .and_then(|res, _, _| {
    //     //         let mut res = res;
    //     //         fut::wrap_future::<_, Node>(res.json())
    //     //             .map_err(|err| NodeError::from(err))
    //     //             .and_then(|json, _, _| {
    //     //                 let change_resp = serde_json::from_slice::<ChangeClusterMembershipResponse>(json)
    //     //                     .map_err(|err| NodeError::from(err))
    //     //                     .and_then(|cresp| {
    //     //                         if let Some(response) = cresp.response {
    //     //                             match response {
    //     //                                 ChangeResponse::Result(cmc) => Ok(cmc),
    //     //
    //     //                                 ChangeResponse::Failure(f) => {
    //     //                                     Err(NodeError::ResponseFailure(f.description))
    //     //                                 }
    //     //
    //     //                                 ChangeResponse::CommandRejectedNotLeader(leader) => {
    //     //                                     Err(NodeError::RemoteNotLeaderError {
    //     //                                         leader_id: Some(leader.leader_id.into()),
    //     //                                         leader_address: Some(leader.leader_address.to_owned()),
    //     //                                     })
    //     //                                 }
    //     //                             }
    //     //                         } else {
    //     //                             Err(NodeError::Unknown("good ChangeClusterResponse had empty response".to_string()))
    //     //                         }
    //     //                     });
    //     //
    //     //                 change_resp
    //     //             })
    //     //     });
    //     //
    //     // Box::new(task)
    //
    //
    //     // let task = self.client.post(register_node_route)
    //     //     .header("Content-Type", "application/json")
    //     //     .send_json(&body)
    //     //     .map_err(|err| NodeError::from(err))
    //     //     .and_then(|res| {
    //     //         let mut res = res;
    //     //
    //     //         let foo = fut::wrap_future::<_, Self>(res.body());
    //     //
    //     //         .then(move |resp, _, _| {
    //     //             if let Ok(body) = resp {
    //     //                 let change_resp = serde_json::from_slice::<ChangeClusterMembershipResponse>(body.as_ref())
    //     //                     .map_err(|err| NodeError::from(err))
    //     //                     .and_then(|cresp| {
    //     //                         if let Some(response) = cresp.response {
    //     //                           match response {
    //     //                               ChangeResponse::Result(cmc) => Ok(cmc),
    //     //
    //     //                               ChangeResponse::Failure(f) => {
    //     //                                   Err(NodeError::ResponseFailure(f.description))
    //     //                               }
    //     //
    //     //                               ChangeResponse::CommandRejectedNotLeader(leader) => {
    //     //                                   Err(NodeError::RemoteNotLeaderError {
    //     //                                       leader_id: Some(leader.leader_id.into()),
    //     //                                       leader_address: Some(leader.leader_address.to_owned()),
    //     //                                   })
    //     //                               }
    //     //                           }
    //     //                         } else {
    //     //                             Err(NodeError::Unknown("good ChangeClusterResponse had empty response".to_string()))
    //     //                         }
    //     //                     });
    //     //
    //     //                 change_resp
    //     //             } else {
    //     //                 fut::result(resp.map_err(|err| NodeError::from(err)))
    //     //             }
    //     //         })
    //     //     });
    //     //
    //     // Box::new(task)
    // }

    fn disconnect(&mut self, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        unimplemented!()
    }
}
