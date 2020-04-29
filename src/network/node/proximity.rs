use std::fmt::{Debug, Display};
use actix::prelude::*;
use actix_raft::NodeId;
use tracing::*;
use crate::NodeInfo;
use super::{Node, NodeError};
use crate::ports::http::entities::NodeInfoMessage;
use crate::network::messages;
use crate::ports::http::entities::{self, change_cluster_membership_response as entities_response};
use crate::network::node::NodeError::RemoteNotLeaderError;

pub trait ChangeClusterBehavior {
    fn change_cluster_config(
        &self,
        to_add: Vec<NodeId>,
        to_remove: Vec<NodeId>,
        ctx: &<Node as Actor>::Context
    ) -> Result<(), NodeError>;
}

pub trait ConnectionBehavior {
    //todo: make nonblocking because of distributed network calls
    fn connect(
        &mut self,
        local_id_info: (NodeId, &NodeInfo),
        ctx: &mut <Node as Actor>::Context
    ) -> Result<messages::ConnectionAcknowledged, NodeError>;

    //todo: make nonblocking because of distributed network calls
    fn disconnect(&mut self, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError>;
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
    #[tracing::instrument(skip(self, local_id_info, _ctx))]
    fn connect(
        &mut self,
        local_id_info: (NodeId, &NodeInfo),
        _ctx: &mut <Node as Actor>::Context
    ) -> Result<messages::ConnectionAcknowledged, NodeError> {
        debug!("connect for local Node#{}->{}", local_id_info.0, self.id);
        Ok(messages::ConnectionAcknowledged {})
    }

    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&mut self, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        Ok(())
    }
}

pub struct RemoteNode {
    remote_id: NodeId,
    remote_info: NodeInfo,
    client: reqwest::Client,
}

impl RemoteNode {
    #[tracing::instrument]
    pub fn new(remote_id: NodeId, remote_info: NodeInfo) -> RemoteNode {
        let client = reqwest::Client::builder()
            .build()
            .expect(format!("prebuilt client for RemoteNode#{}", remote_id).as_str());

        RemoteNode { remote_id, remote_info, client, }
    }

    fn scope(&self) -> String {
        //todo: use encryption
        format!("http://{}/api/cluster", self.remote_info.cluster_address.as_str())
    }
}

impl Display for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode#{}(to:{})", self.remote_id, self.scope())
    }
}

impl Debug for RemoteNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteNode#{}(to:{})", self.remote_id, self.scope())
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


impl RemoteNode {
    fn convert_error( &self, error: reqwest::Error ) -> NodeError {
        match error {
            e if e.is_timeout() => NodeError::Timeout(e.to_string()),
            e if e.is_serialization() => NodeError::ResponseFailure(e.to_string()),
            e if e.is_client_error() => NodeError::RequestError(e),
            e if e.is_http() => NodeError::RequestError(e),
            e if e.is_server_error() => {
                NodeError::ResponseFailure(format!("Error in {:?} server", self))
            },
            e if e.is_redirect() => {
                // need to parse redirect into:
                // NodeError::RemoteNotLeaderError {
                //         leader_id: Option<NodeId>,
                //         leader_address: Option<String>,
                //     }
                NodeError::Unknown(
                    format!(
                        "TODO: PROPERLY HANDLE REDIRECT; E.G., IF REMOTE IS NOT LEADER: {:?}",
                        e
                    )
                )
            },
            e => NodeError::from(e),
        }
    }
}

impl ConnectionBehavior for RemoteNode {
    #[tracing::instrument(skip(self, local_id_info, _ctx))]
    fn connect(
        &mut self,
        local_id_info: (NodeId, &NodeInfo),
        _ctx: &mut <Node as Actor>::Context
    ) -> Result<messages::ConnectionAcknowledged, NodeError> {
        let register_node_route = format!("{}/nodes/{}", self.scope(), self.remote_id);
        debug!("connect Node#{} to RemoteNode#{} via {}", local_id_info.0, self.remote_id, register_node_route);

        let body = NodeInfoMessage {
            node_id: Some(local_id_info.0.into()),
            node_info: Some(local_id_info.1.clone().into()),
        };

        let self_rep = self.to_string();
        let result = self.client
            // .get("https://my-json-server.typicode.com/dmrolfs/json-test-server/connection")
            .post(&register_node_route)
            .json(&body)
            .send()
            .map_err(|err| self.convert_error(err))?
            .json::<entities::ChangeClusterMembershipResponse>()
            .map_err(|err| self.convert_error(err))
            .and_then(|cresp| {
                debug!("SUCCESS connect_to_{} PARSED response: |{:?}|", self_rep, cresp);

                let ack = if let Some(response) = cresp.response {
                    match response {
                        entities_response::Response::Result(r) => {
                            let ack: messages::ConnectionAcknowledged = r.into();
                            Ok(ack)
                        }

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
                };

                ack
            })?;

        Ok(result)
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

    #[tracing::instrument(skip(self, _ctx))]
    fn disconnect(&mut self, _ctx: &mut <Node as Actor>::Context) -> Result<(), NodeError> {
        info!("disconnecting {:?}", self);
        Ok(())
    }
}
