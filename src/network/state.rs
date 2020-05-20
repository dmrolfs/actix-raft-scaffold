use std::collections::HashSet;
use std::time::Duration;
use std::fmt::{Debug, Display};
use actix_raft::NodeId;
use serde::{Serialize, Deserialize};
use tracing::*;
use thiserror::*;
use chrono::prelude::*;
use rayon::prelude::*;
use strum_macros::{Display as StrumDisplay};
use Status::*;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("cannot transition from {0} status to {1}")]
    InvalidStatusTransition(Status, Status),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    status: Status,
    entry: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit: Option<DateTime<Utc>>,
}

impl StatusEntry {
    pub fn new(status: Status, entry: DateTime<Utc>) -> Self { Self { status, entry, exit: None, } }

    pub fn from_system_time(status: Status, entry_st: std::time::SystemTime) -> Self {
        Self::new(status, entry_st.into())
    }

    pub fn duration_in(&self) -> time::Duration {
        let last = self.exit.unwrap_or(Utc::now());
        last - self.entry
    }
}

#[derive(Debug, StrumDisplay, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Joining,
    WeaklyUp,
    Up,
    Leaving,
    Exiting,
    Down,
    Removed,
}

#[derive(Debug, StrumDisplay, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Extent {
    SingleNode,
    Cluster,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkState {
    pub status: Status,
    log: Vec<StatusEntry>,
    connected_nodes: HashSet<NodeId>,
    isolated_nodes: HashSet<NodeId>,
}

impl NetworkState {
    pub fn unwrap(&self) -> Status { self.status }

    pub fn is_connected(&self) -> bool {
        self.status == Up ||
            self.status == WeaklyUp ||
            (self.status == Joining && !self.connected_nodes.is_empty())
    }

    /// After verifying next status is valid considering current, advance NetworkState to next
    /// and return previous status.
    /// Invalid status transitions will panic.
    pub fn advance(&mut self, next: Status) -> Status {
        self.check_next_status(next).expect("unexpected status transition");
        let now = Utc::now();
        let previous = self.log.last_mut().expect("There's always at least one state entry");
        previous.exit = Some(now.clone());

        let old = self.status;
        let next_entry = StatusEntry::new(next, now);
        self.status = next_entry.status;
        self.log.push(next_entry);
        old
    }

    fn check_next_status(&self, next: Status) -> Result<Status, StateError> {
        match (self.status, next) {
            (Removed, n) => Err(StateError::InvalidStatusTransition(Removed, n)),
            (_, Down) => Ok(Down),
            (Joining, WeaklyUp) => Ok(WeaklyUp),
            (Joining, Up) => Ok(Up),
            (WeaklyUp, Up) => Ok(Up),
            (Up, Leaving) => Ok(Leaving),
            (Leaving, Exiting) => Ok(Exiting),
            (Down, Removed) => Ok(Removed),
            (Exiting, Removed) => Ok(Removed),
            (c, n) => Err(StateError::InvalidStatusTransition(c, n)),
        }
    }

    pub fn timeline(&self) -> Vec<StatusEntry> { self.log.clone() }

    pub fn duration_in(&self, status: Status) -> Duration {
        let millis = self.log.par_iter()
            .filter(|e| e.status == status)
            .map(|e| e.duration_in().num_milliseconds())
            .sum();

        ::time::Duration::milliseconds(millis).to_std().unwrap()
    }

    pub fn connected_nodes(&self) -> &HashSet<NodeId> { &self.connected_nodes }

    pub fn is_connected_node(&self, node_id: NodeId) -> bool {
        self.connected_nodes.contains(&node_id)
    }

    pub fn isolated_nodes(&self) -> &HashSet<NodeId> { &self.isolated_nodes }

    pub fn is_isolated_node(&self, node_id: NodeId) -> bool {
        self.isolated_nodes.contains(&node_id)
    }

    pub fn extent(&self) -> Extent {
        if 1 < self.connected_nodes.len() {
            Extent::Cluster
        } else {
            Extent::SingleNode
        }
    }

    pub fn isolate_node(&mut self, id: NodeId) {
        if self.isolated_nodes.contains(&id) {
            debug!("Network node#{} already isolated.", &id);
        } else {
            info!("Isolating node#{} for network.", &id);
            self.isolated_nodes.insert(id);
            self.connected_nodes.remove(&id);
        }
    }

    pub fn restore_node(&mut self, id: NodeId) {
        if self.isolated_nodes.contains(&id) {
            info!("Restoring node#{} for network.", &id);
            self.connected_nodes.insert(id);
            self.isolated_nodes.remove(&id);
        }
    }

}

impl Default for NetworkState {
    fn default() -> Self {
        let entry = StatusEntry::new(Joining, Utc::now());
        NetworkState {
            status: entry.status,
            log: vec![entry],
            connected_nodes: HashSet::default(),
            isolated_nodes: HashSet::default(),
        }
    }
}

impl Debug for NetworkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NetworkState({}:{}, connected:{:?}, isolated:{:?}, log:{:?})",
            self.status, self.extent(), self.connected_nodes(), self.isolated_nodes(), self.log
        )
    }
}

impl Display for NetworkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.status, self.extent())
    }
}
