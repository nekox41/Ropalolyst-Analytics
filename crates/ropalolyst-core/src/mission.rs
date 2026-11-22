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
        match &event.kind {
            EventKind::MissionSucceeded => {
                // Ropalolyst logs can emit an early, non-terminal "Mission Succeeded"
                // while the encounter is still running. Accept it only after the
                // final laser-kill transmission has been received in this session.
                let final_kill_confirmed = self.events.iter().any(|previous| {
                    matches!(
                        previous.kind,
                        EventKind::BossTransmission {
                            kind: crate::TransmissionKind::LaserKill
                        }
                    )
                });
                if final_kill_confirmed {
                    self.status = MissionStatus::Succeeded;
                    self.ended_at = Some(event.timestamp);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn event(timestamp: f64, kind: EventKind) -> GameEvent {
        GameEvent {
            timestamp: LogTimestamp(timestamp),
            kind,
            confidence: crate::EventConfidence::Confirmed,
            source: "test fixture".into(),
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn success_closes_session_with_exact_log_duration() {
        let start = event(42.312, EventKind::MissionStarted);
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(event(190.0, EventKind::BossTransmission {
            kind: crate::TransmissionKind::LaserKill,
        }));
        session.apply(event(191.218, EventKind::MissionSucceeded));

        assert_eq!(session.status, MissionStatus::Succeeded);
        assert!((session.duration_seconds().unwrap() - 148.906).abs() < 0.000_001);
        assert_eq!(session.events.len(), 2);
    }

    #[test]
    fn generic_ending_closes_an_unfinished_run_as_aborted() {
        let start = event(40.299, EventKind::MissionStarted);
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(event(124.448, EventKind::MissionEnding));

        assert_eq!(session.status, MissionStatus::Aborted);
        assert_eq!(session.duration_seconds(), Some(84.149));
    }

    #[test]
    fn success_is_not_overwritten_by_later_generic_ending() {
        let start = event(1.0, EventKind::MissionStarted);
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(event(9.0, EventKind::BossTransmission {
            kind: crate::TransmissionKind::LaserKill,
        }));
        session.apply(event(10.0, EventKind::MissionSucceeded));
        session.apply(event(11.0, EventKind::MissionEnding));

        assert_eq!(session.status, MissionStatus::Succeeded);
        assert_eq!(session.duration_seconds(), Some(9.0));
    }
}
