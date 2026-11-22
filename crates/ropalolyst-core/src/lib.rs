//! Domain and analysis engine for Ropalolyst Analytics.
//!
//! The core owns log discovery, incremental ingestion, parsing, and mission state.
//! The terminal UI consumes typed events and snapshots; it never parses EE.log.

pub mod event;
pub mod ingest;
pub mod mission;
pub mod parser;

pub use event::{EventConfidence, EventKind, GameEvent, LogTimestamp};
pub use ingest::{LogTailer, default_log_path};
pub use mission::{MissionOutcome, MissionSession, MissionStatus};
pub use parser::parse_log_line;
