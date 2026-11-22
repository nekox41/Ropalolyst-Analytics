use serde::{Deserialize, Serialize};

use crate::{EventKind, GameEvent, LogTimestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionStatus {
    InProgress,
    Succeeded,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionOutcome {
    Success,
    Failure,
    Abort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionSession {
    pub started_at: LogTimestamp,
    pub ended_at: Option<LogTimestamp>,
    pub status: MissionStatus,
    pub events: Vec<GameEvent>,
}

impl MissionSession {
    pub fn new(started_at: LogTimestamp, start_event: GameEvent) -> Self {
        Self {
            started_at,
            ended_at: None,
            status: MissionStatus::InProgress,
            events: vec![start_event],
        }
    }

    pub fn duration_seconds(&self) -> Option<f64> {
        self.ended_at.map(|end| end.0 - self.started_at.0)
    }

    /// Apply a normalized event to this session.
    /// Success/failure take precedence over a generic ending transition.
    pub fn apply(&mut self, event: GameEvent) {
        match event.kind {
            EventKind::MissionSucceeded => {
                self.status = MissionStatus::Succeeded;
                self.ended_at = Some(event.timestamp);
            }
            EventKind::MissionFailed => {
                self.status = MissionStatus::Failed;
                self.ended_at = Some(event.timestamp);
            }
            EventKind::MissionEnding if self.status == MissionStatus::InProgress => {
                self.status = MissionStatus::Aborted;
                self.ended_at = Some(event.timestamp);
            }
            _ => {}
        }
        self.events.push(event);
    }

    pub fn outcome(&self) -> Option<MissionOutcome> {
        match self.status {
            MissionStatus::InProgress => None,
            MissionStatus::Succeeded => Some(MissionOutcome::Success),
            MissionStatus::Failed => Some(MissionOutcome::Failure),
            MissionStatus::Aborted => Some(MissionOutcome::Abort),
        }
    }
}
