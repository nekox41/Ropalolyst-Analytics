//! Domain and analysis engine for Ropalolyst Analytics.
//!
//! This crate deliberately contains no UI code. Log parsing, mission segmentation,
//! normalized events, encounter state, and timing will live here.

pub mod event;
pub mod mission;

pub use event::{EventKind, GameEvent, LogTimestamp};
pub use mission::{MissionOutcome, MissionSession, MissionStatus};
