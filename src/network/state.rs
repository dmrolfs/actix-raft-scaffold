use std::time::{Instant, Duration};
use std::fmt::{Debug, Display};
use thiserror::*;
use rayon::prelude::*;
use super::Node;
use State::*;
use strum_macros::{Display as StrumDisplay};

#[derive(Error, Debug)]
pub enum StateError {
    #[error("cannot transition from {0} to {1}")]
    InvalidStateTransition(State, State),
}

#[derive(Debug, Clone)]
struct StateEntry {
    state: State,
    entry: Instant,
    exit: Option<Instant>,
}

impl StateEntry {
    pub fn new(state: State, entry_time: Instant) -> Self {
        Self {
            state,
            entry: entry_time,
            exit: None,
        }
    }

    pub fn duration_in(&self) -> Duration {
        let last = self.exit.unwrap_or(Instant::now());
        last - self.entry
    }
}

#[derive(Debug, StrumDisplay, Clone, Copy, PartialEq)]
enum State {
    Joining,
    WeaklyUp,
    Up,
    Leaving,
    Exiting,
    Down,
    Removed,
}

// impl Display for State {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let rep = match self {
//             Joining => "Joining",
//             WeaklyUp => "WeaklyUp",
//             Up => "Up",
//             Leaving => "Leaving",
//             Exiting => "Exiting",
//             Down => "Down",
//             Removed => "Removed",
//         };
//         write!(f, "{}", rep)
//     }
// }

#[derive(Clone)]
pub struct StateMachine {
    pub state: State,
    log: Vec<StateEntry>,
}

impl StateMachine {
    pub fn unwrap(&self) -> State { self.state }

    pub fn is_connected(&self) -> bool { self.state == Up }

    pub fn into(&mut self, next: State) -> State {
        self.check_next_state(next);
        let now = Instant::now();
        let previous = self.log.last_mut().expect("There's always at least one state entry");
        previous.exit = Some(now.clone());

        let old = self.state;
        let next_entry = StateEntry::new(next, now);
        self.state = next_entry.state;
        self.log.push(next_entry);
        old
    }

    fn check_next_state(&self, next: State) -> Result<State, StateError> {
        match (self.state, next) {
            (Removed, n) => Err(StateError::InvalidStateTransition(Removed, n)),
            (_, Down) => Ok(Down),
            (Joining, WeaklyUp) => Ok(WeaklyUp),
            (Joining, Up) => Ok(Up),
            (WeaklyUp, Up) => Ok(Up),
            (Up, Leaving) => Ok(Leaving),
            (Leaving, Exiting) => Ok(Exiting),
            (Down, Removed) => Ok(Removed),
            (Exiting, Removed) => Ok(Removed),
            (c, n) => Err(StateError::InvalidStateTransition(c, n)),
        }
    }

    pub fn timeline(&self) -> Vec<StateEntry> { self.log.clone() }

    pub fn duration_in(&self, state: State) -> Duration {
        self.log.par_iter()
            .filter(|e| e.state == state)
            .map(|e| e.duration_in())
            .sum()
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        let entry = StateEntry::new(Joining, Instant::now());
        StateMachine {
            state: entry.state,
            log: vec![entry],
        }
    }
}

impl Debug for StateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StateMachine({}, log:{:?})", self.state, self.log)
    }
}

impl Display for StateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.state)
    }
}

// #[derive(Debug, Clone, Copy)]
// pub struct Up {
//     pub confirmation_duration: Duration, //todo: make private
//     pub started_at: Instant,
// }
//
// impl Up {
//     pub fn confirmation_duration(&self) -> Duration { self.confirmation_duration }
// }
//
// impl StateBehavior for Up {
//     fn is_connected() -> bool { true }
//     fn duration_in(&self) -> Duration { Instant::now() - self.started_at }
// }
//
// impl From<StateMachine<Joining>> for StateMachine<Up> {
//     fn from(from: StateMachine<Joining>) -> Self {
//         StateMachine {
//             state: Up {
//                 confirmation_duration: Instant::now() - from.state.started_at,
//                 started_at: Instant::now(),
//             }
//         }
//     }
// }
//
// impl From<StateMachine<WeaklyUp>> for StateMachine<Up> {
//     fn from(from: StateMachine<WeaklyUp>) -> Self {
//         let now = Instant::now();
//         StateMachine {
//             state: Up {
//                 confirmation_duration: now - from.state.started_at,
//                 started_at: now,
//             }
//         }
//     }
// }
//
// impl Display for Up {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Up") }
// }
//
// impl PartialEq for Up {
//     fn eq(&self, other: &Self) -> bool { true }
// }
//
//
// #[derive(Debug, Clone, Copy)]
// struct Leaving {
//     confirmation_duration: Duration,
//     up_duration: Duration,
//     pub started_at: Instant,
// }
//
// impl Leaving {
//     pub fn confirmation_duration(&self) -> Duration { self.confirmation_duration }
//     pub fn up_duration(&self) -> Duration { self.up_duration }
// }
//
// impl StateBehavior for Leaving {
//     fn duration_in(&self) -> Duration { Instant::now() - self.started_at }
// }
//
// impl From<StateMachine<Up>> for StateMachine<Leaving> {
//     fn from(from: StateMachine<Up>) -> Self {
//         let now = Instant::now();
//         StateMachine {
//             state: Leaving {
//                 confirmation_duration: from.state.confirmation_duration(),
//                 up_duration: now - from.state.started_at,
//                 started_at: now,
//             }
//         }
//     }
// }
//
//
// impl Display for Leaving {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Leaving") }
// }
//
// impl PartialEq for Leaving {
//     fn eq(&self, other: &Self) -> bool { true }
// }
//
//
// #[derive(Debug, Clone, Copy)]
// struct Exiting {
//     confirmation_duration: Duration,
//     up_duration: Duration,
//     leaving_duration: Duration,
//     pub started_at: Instant,
// }
//
// impl Exiting {
//     pub fn confirmation_duration(&self) -> Duration { self.confirmation_duration }
//     pub fn up_duration(&self) -> Duration { self.up_duration }
//     pub fn leaving_duration(&self) -> Duration { self.leaving_duration }
// }
//
// impl StateBehavior for Exiting {
//     fn duration_in(&self) -> Duration { Instant::now() - self.started_at }
// }
//
// impl From<StateMachine<Leaving>> for StateMachine<Exiting> {
//     fn from(from: StateMachine<Leaving>) -> Self {
//         let now = Instant::now();
//         StateMachine {
//             state: Exiting {
//                 confirmation_duration: from.state.confirmation_duration(),
//                 up_duration: from.state.up_duration(),
//                 leaving_duration: now - from.state.started_at,
//                 started_at: now,
//             }
//         }
//     }
// }
//
//
// impl Display for Exiting {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Exiting") }
// }
//
// impl PartialEq for Exiting {
//     fn eq(&self, other: &Self) -> bool { true }
// }
//
//
// #[derive(Debug, Clone, Copy)]
// struct Down {
//     up_duration: Duration,
//     pub started_at: Instant,
// }
//
// impl Down {
//     pub fn up_duration(&self) -> Duration { self.up_duration }
// }
//
// impl StateBehavior for Down {
//     fn duration_in(&self) -> Duration { Instant::now() - self.started_at }
// }
//
// impl From<StateMachine<Joining>> for StateMachine<Down> {
//     fn from(from: StateMachine<Joining>) -> Self {
//         StateMachine {
//             state: Down {
//                 up_duration: Duration::default(),
//                 started_at: Instant::now(),
//             }
//         }
//     }
// }
//
// impl From<StateMachine<WeaklyUp>> for StateMachine<Down> {
//     fn from(from: StateMachine<WeaklyUp>) -> Self {
//         StateMachine {
//             state: Down {
//                 up_duration: Duration::default(),
//                 started_at: Instant::now(),
//             }
//         }
//     }
// }
//
// impl From<StateMachine<Up>> for StateMachine<Down> {
//     fn from(from: StateMachine<Up>) -> Self {
//         let now = Instant::now();
//         StateMachine {
//             state: Down {
//                 up_duration: now - from.state.started_at,
//                 started_at: now,
//             }
//         }
//     }
// }
//
// impl From<StateMachine<Leaving>> for StateMachine<Down> {
//     fn from(from: StateMachine<Leaving>) -> Self {
//         StateMachine {
//             state: Down {
//                 up_duration: from.state.up_duration(),
//                 started_at: Instant::now(),
//             }
//         }
//     }
// }
//
// impl From<StateMachine<Exiting>> for StateMachine<Down> {
//     fn from(from: StateMachine<Exiting>) -> Self {
//         StateMachine {
//             state: Down {
//                 up_duration: from.state.up_duration(),
//                 started_at: Instant::now(),
//             }
//         }
//     }
// }
//
// impl Display for Down {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Down") }
// }
//
// impl PartialEq for Down {
//     fn eq(&self, other: &Self) -> bool { true }
// }
//
//
// #[derive(Debug, Clone, Copy)]
// struct Removed {
//     up_duration: Duration,
//     exiting_duration: Duration,
//     pub started_at: Instant,
// }
//
// impl Removed {
//     pub fn up_duration(&self) -> Duration { self.up_duration }
//     pub fn exiting_duration(&self) -> Duration { self.exiting_duration }
// }
//
// impl StateBehavior for Removed {
//     fn duration_in(&self) -> Duration { Instant::now() - self.started_at }
// }
//
// impl From<StateMachine<Down>> for StateMachine<Removed> {
//     fn from(from: StateMachine<Down>) -> Self {
//         let now = Instant::now();
//         StateMachine {
//             state: Removed {
//                 up_duration: from.state.up_duration(),
//                 exiting_duration: now - from.state.started_at,
//                 started_at: now,
//             }
//         }
//     }
// }
//
// impl From<StateMachine<Exiting>> for StateMachine<Removed> {
//     fn from(from: StateMachine<Exiting>) -> Self {
//         let now = Instant::now();
//         StateMachine {
//             state: Removed {
//                 up_duration: from.state.up_duration(),
//                 exiting_duration: now - from.state.started_at,
//                 started_at: now,
//             }
//         }
//     }
// }
//
// impl Display for Removed {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Removed") }
// }
//
// impl PartialEq for Removed {
//     fn eq(&self, other: &Self) -> bool { true }
// }
