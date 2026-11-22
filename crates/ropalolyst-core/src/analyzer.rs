use crate::{EventKind, GameEvent, MissionSession, MissionStatus};

/// Runtime aggregation state consumed by the dashboard.
#[derive(Debug, Default)]
pub struct Analyzer {
    pub active: Option<MissionSession>,
    pub completed: Vec<MissionSession>,
    pub timeline: Vec<GameEvent>,
    pub last_event: Option<GameEvent>,
}

impl Analyzer {
    pub fn ingest(&mut self, mut event: GameEvent) {
        // 轮次由本次任务中实际出现的顺序决定；塔 ID 由地图 cinematic 实例决定。
        if let (Some(session), EventKind::BossCrashStarted { tower_id, .. }) =
            (self.active.as_ref(), &event.kind)
        {
            let cycle = session.events.iter().filter(|previous| {
                matches!(previous.kind, EventKind::BossCrashStarted { .. })
            }).count() as u8 + 1;
            event.kind = EventKind::BossCrashStarted { tower_id: *tower_id, cycle };
        }
        if event.kind == EventKind::MissionStarted {
            if let Some(mut previous) = self.active.take() {
                if previous.status == MissionStatus::InProgress {
                    previous.status = MissionStatus::Aborted;
                    previous.ended_at = Some(event.timestamp);
                }
                self.completed.push(previous);
            }
            self.active = Some(MissionSession::new(event.timestamp, event.clone()));
        } else if let Some(session) = self.active.as_mut() {
            session.apply(event.clone());
            if session.status != MissionStatus::InProgress {
                self.completed.push(self.active.take().expect("active session exists"));
            }
        }

        self.last_event = Some(event.clone());
        self.timeline.push(event);
        if self.timeline.len() > 100 {
            self.timeline.remove(0);
        }
    }

    pub fn best_success_seconds(&self) -> Option<f64> {
        self.completed
            .iter()
            .filter(|session| session.status == MissionStatus::Succeeded)
            .filter_map(MissionSession::duration_seconds)
            .min_by(f64::total_cmp)
    }

    pub fn average_success_seconds(&self) -> Option<f64> {
        let mut values = self.completed
            .iter()
            .filter(|session| session.status == MissionStatus::Succeeded)
            .filter_map(MissionSession::duration_seconds);
        let (sum, count) = values.by_ref().fold((0.0, 0usize), |(sum, count), value| (sum + value, count + 1));
        (count > 0).then_some(sum / count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventConfidence, LogTimestamp};

    fn event(at: f64, kind: EventKind) -> GameEvent {
        GameEvent::new(LogTimestamp(at), kind, EventConfidence::Confirmed, "fixture")
    }

    #[test]
    fn repeated_mission_starts_split_sessions_and_abort_previous() {
        let mut analyzer = Analyzer::default();
        analyzer.ingest(event(1.0, EventKind::MissionStarted));
        analyzer.ingest(event(5.0, EventKind::MissionStarted));
        assert_eq!(analyzer.completed.len(), 1);
        assert_eq!(analyzer.completed[0].status, MissionStatus::Aborted);
        assert_eq!(analyzer.active.as_ref().unwrap().started_at, LogTimestamp(5.0));
    }

    #[test]
    fn crash_cycle_uses_play_order_and_keeps_tower_identity() {
        let mut analyzer = Analyzer::default();
        analyzer.ingest(event(1.0, EventKind::MissionStarted));
        analyzer.ingest(event(2.0, EventKind::BossCrashStarted { tower_id: 3, cycle: 0 }));
        analyzer.ingest(event(3.0, EventKind::BossCrashStarted { tower_id: 1, cycle: 0 }));
        let session = analyzer.active.as_ref().unwrap();
        assert_eq!(session.events[1].kind, EventKind::BossCrashStarted { tower_id: 3, cycle: 1 });
        assert_eq!(session.events[2].kind, EventKind::BossCrashStarted { tower_id: 1, cycle: 2 });
    }

    #[test]
    fn successful_runs_produce_best_and_average_statistics() {
        let mut analyzer = Analyzer::default();
        for (start, end) in [(1.0, 11.0), (20.0, 35.0)] {
            analyzer.ingest(event(start, EventKind::MissionStarted));
            analyzer.ingest(event(end, EventKind::MissionSucceeded));
        }
        assert_eq!(analyzer.best_success_seconds(), Some(10.0));
        assert_eq!(analyzer.average_success_seconds(), Some(12.5));
    }
}
