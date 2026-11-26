//! Domain and analysis engine for Ropalolyst Analytics.
//!
//! The core owns log discovery, incremental ingestion, parsing, and mission state.
//! The terminal UI consumes typed events and snapshots; it never parses EE.log.

pub mod analyzer;
pub mod clock;
pub mod event;
pub mod ingest;
pub mod mission;
pub mod parser;
pub mod storage;
pub mod splits;

pub use analyzer::Analyzer;
pub use clock::{LogClockAnchor, parse_clock_anchor};
pub use event::{EventConfidence, EventKind, GameEvent, LogTimestamp, TransmissionKind};
pub use ingest::{LogTailer, default_log_path};
pub use mission::{MissionOutcome, MissionSession, MissionStatus};
pub use parser::parse_log_line;
pub use storage::{HistoryRecord, RunStore};
pub use splits::{PhaseSplit, average_phase_splits, phase_splits};
