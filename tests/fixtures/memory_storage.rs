use std::collections::BTreeMap;
use std::io::{Seek, SeekFrom, Write};
use std::fs::{self, File};
use std::path::PathBuf;
use actix::prelude::*;
use tracing::*;
use serde::{Serialize, Deserialize};
use serde_cbor as cbor;
use actix_raft::{
    AppData, AppDataResponse, AppError, NodeId,
    messages::{Entry as RaftEntry, EntrySnapshotPointer, MembershipConfig},
    storage::{
        AppendEntryToLog,
        ReplicateToLog,
        ApplyEntryToStateMachine,
        ReplicateToStateMachine,
        CreateSnapshot,
        CurrentSnapshotData,
        GetCurrentSnapshot,
        GetInitialState,
        GetLogEntries,
        HardState,
        InitialState,
        InstallSnapshot,
        RaftStorage,
        SaveHardState,
    },
};


pub type Data = MemoryStorageData;
type Entry = RaftEntry<Data>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryStorageData {
    pub data: Vec<u8>,
}

impl AppData for MemoryStorageData {}


#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryStorageResponse;

impl AppDataResponse for MemoryStorageResponse {}


//todo use thiserror
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStorageError;

impl std::fmt::Display for MemoryStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemoryStorageError")
    }
}

impl std::error::Error for MemoryStorageError {}

impl AppError for MemoryStorageError {}


pub struct MemoryStorage {
    hs: HardState,
    log: BTreeMap<u64, Entry>,
    snapshot_data: Option<CurrentSnapshotData>,
    snapshot_dir: String,
    state_machine: BTreeMap<u64, Entry>,
    snapshot_actor: Addr<SnapshotActor>,
}

impl MemoryStorage {
    pub fn new(members: Vec<NodeId>, snapshot_dir: String) -> Self {
        let snapshot_path = std::path::PathBuf::from(snapshot_dir.as_str());
        let membership = MembershipConfig {
            members,
            non_voters: vec![],
            removing: vec![],
            is_in_joint_consensus: false
        };

        Self {
            hs: HardState { current_term: 0, voted_for: None, membership },
            log: Default::default(),
            snapshot_data: None,
            snapshot_dir,
            state_machine: Default::default(),
            snapshot_actor: SyncArbiter::start(1, move || SnapshotActor(snapshot_path.clone())),
        }
    }
}

impl Actor for MemoryStorage {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Self::Context) {}
}

impl RaftStorage<Data, MemoryStorageResponse, MemoryStorageError> for MemoryStorage {
    type Actor = Self;
    type Context = Context<Self>;
}

impl Handler<GetInitialState<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, InitialState, MemoryStorageError>;

    #[tracing::instrument(skip(self, _msg, _ctx))]
    fn handle(
        &mut self,
        _msg: GetInitialState<MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        Box::new(
            if let Some((last_id, last_entry)) = self.log.iter().last() {
                fut::ok( InitialState {
                    last_log_index: *last_id,
                    last_log_term: last_entry.term,
                    last_applied_log: *last_id,
                    hard_state: self.hs.clone(),
                })
            } else {
                // fut::err(MemoryStorageError)
                fut::ok( InitialState {
                    last_log_index: 0,
                    last_log_term: 0,
                    last_applied_log: 0,
                    hard_state: self.hs.clone(),
                })
            }
        )
    }
}

impl Handler<SaveHardState<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(&mut self, msg: SaveHardState<MemoryStorageError>, _ctx: &mut Self::Context) -> Self::Result {
        self.hs = msg.hs;
        Box::new(fut::ok(()))
    }
}

impl Handler<GetLogEntries<Data, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, Vec<Entry>, MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: GetLogEntries<Data, MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        Box::new(fut::ok(self.log.range(msg.start..msg.stop).map(|e| e.1.clone()).collect()))
    }
}

impl Handler<AppendEntryToLog<Data, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: AppendEntryToLog<Data, MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        self.log.insert(msg.entry.index, (*msg.entry).clone());
        Box::new(fut::ok(()))
    }
}

impl Handler<ReplicateToLog<Data, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: ReplicateToLog<Data, MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        msg.entries.iter().for_each(|e| {
            self.log.insert(e.index, e.clone());
        });
        Box::new(fut::ok(()))
    }
}

impl Handler<ApplyEntryToStateMachine<Data, MemoryStorageResponse, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, MemoryStorageResponse, MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: ApplyEntryToStateMachine<Data, MemoryStorageResponse, MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        Box::new(fut::result(
            if let Some(old) = self.state_machine.insert(msg.payload.index, (*msg.payload).clone()) {
                error!(
                    entry = ?old,
                    "Critical error. State machine entries are not allowed to be overwritten."
                );
                Err(MemoryStorageError)
            } else {
                Ok(MemoryStorageResponse)
            }
        ))
    }
}

impl Handler<ReplicateToStateMachine<Data, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: ReplicateToStateMachine<Data, MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        Box::new(fut::result(
            msg.payload.iter().try_for_each(|e| {
                if let Some(old) = self.state_machine.insert(e.index, e.clone()) {
                    error!(
                        entry = ?old,
                        "Critical error. State machine entries are not allowed to be overwritten."
                    );
                    Err(MemoryStorageError)
                } else {
                    Ok(())
                }
            })
        ))
    }
}

impl Handler<CreateSnapshot<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, CurrentSnapshotData, MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: CreateSnapshot<MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        debug!(
            snapshot_dir = ?self.snapshot_dir, through_index = msg.through,
            "Creating new snapshot."
        );

        let entries = self.log.range(0u64 ..= msg.through).map(|(_, v)| v.clone()).collect::<Vec<_>>();
        debug!("Creating snapshot with {} entries.", entries.len());

        let (index, term) = entries.last().map(|e| (e.index, e.term)).unwrap_or((0, 0));
        let snapdata = match cbor::to_vec(&entries) {
            Ok(snapdata) => snapdata,
            Err(err) => {
                error!(error = ?err, "Error serializing log for creating a snapshot.");
                return Box::new(fut::err(MemoryStorageError));
            }
        };

        // Create a snapshot file and write snapshot data to it.
        let filename = format!("{}", msg.through);
        let filepath = std::path::PathBuf::from(self.snapshot_dir.as_str()).join(filename);
        Box::new(fut::wrap_future::<_, Self>(self.snapshot_actor.send(CreateSnapshotWithData(filepath.clone(), snapdata)))
            .map_err(|err, _, _| {
                error!(error = ?err, "Error communicating with snapshot actor.");
                panic!("Error communicating with snapshot actor: {}", err)
            })
            .and_then(|res, _, _| fut::result(res))
            // Clean up old log entries which are now part of the new snapshot.
            .and_then(move |res, act, _| {
                let path = filepath.to_string_lossy().to_string();
                debug!(snapshot_file = ?path, "Finished creating snapshot file.");

                act.log = act.log.split_off(&msg.through);
                let pointer = EntrySnapshotPointer { path };
                let entry = Entry::new_snapshot_pointer(pointer.clone(), index, term);
                act.log.insert(msg.through, entry);

                // Cache the most recent snapshot data.
                let current_snap_data = CurrentSnapshotData {
                    term,
                    index,
                    membership: act.hs.membership.clone(),
                    pointer
                };
                act.snapshot_data = Some(current_snap_data.clone());
                fut::ok(current_snap_data)
            })
        )
    }
}

impl Handler<InstallSnapshot<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: InstallSnapshot<MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        let index = msg.index;
        let term = msg.term;

        Box::new(fut::wrap_future::<_, Self>(self.snapshot_actor.send(SyncInstallSnapshot(msg)))
            .map_err(|err, _, _| {
                error!(error = ?err, "Error communicating with snapshot actor.");
                panic!("Error communicating with snapshot actor: {}", err)
            })
            .and_then(|res, _, _| fut::result(res))

            // Snapshot file has been created. Perform final steps of this algorithm.
            .and_then(move |pointer, act, ctx| {
                // Cache the most recent snapshot data.
                act.snapshot_data = Some(CurrentSnapshotData {
                    index,
                    term,
                    membership: act.hs.membership.clone(),
                    pointer: pointer.clone(),
                });

                // Update target index with the new snapshot points.
                let entry = Entry::new_snapshot_pointer(pointer.clone(), index, term);
                act.log = act.log.split_off(&index);
                let previous = act.log.insert(index, entry);

                // If there are any logs newer than `index`, then we are done; else, the state
                // machine should be reset, and recreated from the new snapshot.
                match &previous {
                    Some(entry) if entry.index == index && entry.term == term => {
                        fut::Either::A(fut::ok(()))
                    }

                    // There are no newr entries in the log, which means that we need to rebuild
                    // the state machine. Open the snapshot file and read out its entries.
                    _ => {
                        let pathbuf = PathBuf::from(pointer.path);
                        fut::Either::B(act.rebuild_state_machine_from_snapshot(ctx, pathbuf))
                    }
                }
            })
        )
    }
}

impl Handler<GetCurrentSnapshot<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, Option<CurrentSnapshotData>, MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(
        &mut self,
        msg: GetCurrentSnapshot<MemoryStorageError>,
        _ctx: &mut Self::Context
    ) -> Self::Result {
        debug!("Checking for current snapshot.");
        Box::new(fut::ok(self.snapshot_data.clone()))
    }
}

impl MemoryStorage {
    /// Rebuild the state machine from the given snapshot.
    #[tracing::instrument(skip(self, _ctx))]
    fn rebuild_state_machine_from_snapshot(
        &mut self,
        _ctx: &mut <MemoryStorage as Actor>::Context,
        path: std::path::PathBuf
    ) -> impl ActorFuture<Actor = Self, Item = (), Error = MemoryStorageError> {
        // Read full contents of the snapshot file.
        fut::wrap_future::<_, Self>(self.snapshot_actor.send(DeserializeSnapshot(path)))
            .map_err(|err, _, _| {
                error!(error = ?err, "Error communicating with snapshot actor.");
                panic!("Error communicating with snapshot actor: {}", err)
            })
            .and_then(|res, _, _| fut::result(res))

            // Rebuild state machine from the deserialized data.
            .and_then(|entries, act, _| {
                act.state_machine.clear();
                act.state_machine.extend(entries.into_iter().map(|e| (e.index, e)));
                fut::ok(())
            })
            .map(|_, _, _| debug!("Finished rebuilding statemachine from snapshot successfully."))
    }
}


//////////////////////////////////////////////////////////////////////////////////////////////////
// SnapshotActor /////////////////////////////////////////////////////////////////////////////////

/// A simple synchronous actor for interfacing with the filesystem for snapshots.
struct SnapshotActor( std::path::PathBuf );

impl Actor for SnapshotActor {
    type Context = SyncContext<Self>;
}

struct SyncInstallSnapshot( InstallSnapshot<MemoryStorageError> );

//////////////////////////////////////////////////////////////////////////////
// CreateSnapshotWithData ////////////////////////////////////////////////////

#[derive(Debug)]
struct CreateSnapshotWithData(PathBuf, Vec<u8>);

impl Message for CreateSnapshotWithData {
    type Result = Result<(), MemoryStorageError>;
}

impl Handler<CreateSnapshotWithData> for SnapshotActor {
    type Result = Result<(), MemoryStorageError>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: CreateSnapshotWithData, _ctx: &mut Self::Context) -> Self::Result {
        fs::write(msg.0.clone(), msg.1).map_err(|err| {
            error!(error = ?err, "Error writing snapshot file. {}", err);
            MemoryStorageError
        })
    }
}


//////////////////////////////////////////////////////////////////////////////
// DeserializeSnapshot ///////////////////////////////////////////////////////

#[derive(Debug)]
struct DeserializeSnapshot( PathBuf );

impl Message for DeserializeSnapshot {
    type Result = Result<Vec<Entry>, MemoryStorageError>;
}

impl Handler<DeserializeSnapshot> for SnapshotActor {
    type Result = Result<Vec<Entry>, MemoryStorageError>;

    #[tracing::instrument(skip(self, _ctx))]
    fn handle(&mut self, msg: DeserializeSnapshot, _ctx: &mut Self::Context) -> Self::Result {
        fs::read(msg.0)
            .map_err(|err| {
                error!(error = ?err, "Error reading contexts of snapshot file.");
                MemoryStorageError
            })

            // Deserialize the data of the snapshot file.
            .and_then(|snapdata| {
                cbor::from_slice::<Vec<Entry>>(snapdata.as_slice())
                    .map_err(|err| {
                        error!(error = ?err, "Error deserializing snapshot contents.");
                        MemoryStorageError
                    })
            })
    }
}


//////////////////////////////////////////////////////////////////////////////
// SyncInstallSnapshot ///////////////////////////////////////////////////////

impl Message for SyncInstallSnapshot {
    type Result = Result<EntrySnapshotPointer, MemoryStorageError>;
}

impl Handler<SyncInstallSnapshot> for SnapshotActor {
    type Result = Result<EntrySnapshotPointer, MemoryStorageError>;

    #[tracing::instrument(skip(self, msg, _ctx))]
    fn handle(&mut self, msg: SyncInstallSnapshot, _ctx: &mut Self::Context) -> Self::Result {
        let filename = format!("{}", &msg.0.index);
        let filepath = std::path::PathBuf::from(self.0.clone()).join(filename);

        // Create the new snapshot file.
        let mut snapfile = File::create(&filepath)
            .map_err(|err| {
                error!(error = ?err, "Error creating new snapshot file.");
                MemoryStorageError
            })?;

        let chunk_stream = msg.0.stream
            .map_err( |_| {
                error!("Snapshot chunk stream hit an error in the memory_storage system.");
                MemoryStorageError
            })
            .wait();

        let mut did_process_final_chunk = false;
        for chunk in chunk_stream {
            let chunk = chunk?;
            snapfile
                .seek(SeekFrom::Start(chunk.offset))
                .map_err(|err| {
                    error!(error = ?err, "Error seeking to file location for writing snapshot chunk.");
                    MemoryStorageError
                })?;

            snapfile
                .write_all(&chunk.data)
                .map_err(|err| {
                    error!(error = ?err, "Error writing snapshot chunk to snapshot file.");
                    MemoryStorageError
                })?;

            if chunk.done {
                did_process_final_chunk = true;
            }

            let _ = chunk.cb.send(());
        }

        if !did_process_final_chunk {
            error!("Prematurely exiting snapshot chunk stream. Never reached final chunk.");
            Err(MemoryStorageError)
        } else {
            Ok(EntrySnapshotPointer { path: filepath.to_string_lossy().to_string() })
        }
    }
}


//////////////////////////////////////////////////////////////////////////////////////////////////
// Other Message Types & Handlers ////////////////////////////////////////////////////////////////
//
// NOTE WELL: these following types, just as the MemoryStorage system overall, is intended
// primarily for testing purposes. Don't build your application using this storage implementation.

/// Get the current state of the storage engine.
#[derive(Debug)]
pub struct GetCurrentState;

impl Message for GetCurrentState {
    type Result = Result<CurrentStateData, ()>;
}


/// The current state of the storage engine.
pub struct CurrentStateData {
    pub hs: HardState,
    pub log: BTreeMap<u64, Entry>,
    pub snapshot_data: Option<CurrentSnapshotData>,
    pub snapshot_dir: String,
    pub state_machine: BTreeMap<u64, Entry>,
}

impl Handler<GetCurrentState> for MemoryStorage {
    type Result = Result<CurrentStateData, ()>;

    fn handle(&mut self, _: GetCurrentState, _: &mut Self::Context) -> Self::Result {
        Ok(CurrentStateData {
            hs: self.hs.clone(),
            log: self.log.clone(),
            snapshot_data: self.snapshot_data.clone(),
            snapshot_dir: self.snapshot_dir.clone(),
            state_machine: self.state_machine.clone(),
        })
    }
}
