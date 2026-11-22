use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Timestamp as represented by the source log, in seconds from the log's
/// process-relative clock. Keep the original value for exact split calculations.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LogTimestamp(pub f64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    MissionStarted,
    MissionSucceeded,
    MissionFailed,
    MissionEnding,
    BossArenaIntroStarted,
    BossArenaIntroEnded,
    BossFightStarted,
    BossCrashStarted { sequence: u8 },
    BossCrashEnded { sequence: u8 },
    BossDefeatedCandidate,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent {
    pub timestamp: LogTimestamp,
    pub kind: EventKind,
    /// Original source line or a compact source excerpt for diagnostics.
    pub source: String,
    /// Wall-clock ingestion time; not used for run timing.
    pub observed_at: DateTime<Utc>,
}

impl GameEvent {
    pub fn new(timestamp: LogTimestamp, kind: EventKind, source: impl Into<String>) -> Self {
        Self {
            timestamp,
            kind,
            source: source.into(),
            observed_at: Utc::now(),
        }
    }
}
