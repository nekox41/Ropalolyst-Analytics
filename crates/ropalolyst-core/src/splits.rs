use serde::{Deserialize, Serialize};

use crate::{EventKind, MissionSession, MissionStatus, TransmissionKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseSplit {
    pub key: String,
    pub label: String,
    /// 累计时间：相对 MissionStarted。
    pub cumulative_seconds: f64,
    /// 阶段耗时：相对上一个竞速分段点。
    pub split_seconds: f64,
}

/// 五个稳定竞速分段：跑图、三次撞击、结算。
/// 只读取任务状态、Boss 登场动画、撞塔动画和最终处决结果等明确日志。
pub fn phase_splits(session: &MissionSession) -> Vec<PhaseSplit> {
    let mut result = Vec::with_capacity(5);
    let mut previous = 0.0;

    let mut push = |key: &str, label: &str, timestamp: f64| {
        let cumulative = timestamp - session.started_at.0;
        if cumulative >= previous {
            result.push(PhaseSplit {
                key: key.to_owned(),
                label: label.to_owned(),
                cumulative_seconds: cumulative,
                split_seconds: cumulative - previous,
            });
            previous = cumulative;
        }
    };

    for event in &session.events {
        match &event.kind {
            EventKind::BossArenaIntroStarted => {
                push("run", "跑图", event.timestamp.0);
            }
            EventKind::BossCrashStarted { cycle: 1, .. } => {
                push("impact_1", "第一次撞击", event.timestamp.0);
            }
            EventKind::BossCrashStarted { cycle: 2, .. } => {
                push("impact_2", "第二次撞击", event.timestamp.0);
            }
            EventKind::BossCrashStarted { cycle: 3, .. } => {
                push("impact_3", "第三次撞击", event.timestamp.0);
            }
            EventKind::MissionSucceeded
                if session.status == MissionStatus::Succeeded
                    && session.ended_at == Some(event.timestamp)
                    && session.events.iter().any(|previous| {
                        matches!(
                            previous.kind,
                            EventKind::BossTransmission {
                                kind: TransmissionKind::LaserKill
                            }
                        )
                    }) =>
            {
                push("settlement", "结算时间", event.timestamp.0);
            }
            _ => {}
        }
    }
    result
}

/// 成功成绩中相同分段的平均阶段耗时；缺少标记的旧成绩不参与平均。
pub fn average_phase_splits(sessions: &[MissionSession]) -> Vec<(String, f64, usize)> {
    let mut totals: std::collections::BTreeMap<String, (f64, usize)> =
        std::collections::BTreeMap::new();
    for session in sessions.iter().filter(|s| s.status == MissionStatus::Succeeded) {
        for split in phase_splits(session) {
            let entry = totals.entry(split.key).or_insert((0.0, 0));
            entry.0 += split.split_seconds;
            entry.1 += 1;
        }
    }
    totals
        .into_iter()
        .map(|(key, (sum, count))| (key, sum / count as f64, count))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventConfidence, GameEvent, LogTimestamp};

    fn event(at: f64, kind: EventKind) -> GameEvent {
        GameEvent::new(LogTimestamp(at), kind, EventConfidence::Confirmed, "fixture")
    }

    fn completed_run(start: f64, times: [f64; 5]) -> MissionSession {
        let start_event = event(start, EventKind::MissionStarted);
        let mut session = MissionSession::new(start_event.timestamp, start_event);
        session.apply(event(times[0], EventKind::BossArenaIntroStarted));
        session.apply(event(times[1], EventKind::BossCrashStarted { tower_id: 1, cycle: 1 }));
        session.apply(event(times[2], EventKind::BossCrashStarted { tower_id: 2, cycle: 2 }));
        session.apply(event(times[3], EventKind::BossCrashStarted { tower_id: 3, cycle: 3 }));
        session.apply(event(times[4] - 2.0, EventKind::BossTransmission {
            kind: TransmissionKind::LaserKill,
        }));
        session.apply(event(times[4], EventKind::MissionSucceeded));
        session
    }

    #[test]
    fn emits_exactly_five_splits_relative_to_mission_start() {
        let session = completed_run(44.224, [81.951, 112.070, 142.707, 173.100, 208.239]);
        let splits = phase_splits(&session);
        assert_eq!(splits.len(), 5);
        assert_eq!(splits.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(),
            vec!["run", "impact_1", "impact_2", "impact_3", "settlement"]);
        let cumulative = splits.iter().map(|s| s.cumulative_seconds).collect::<Vec<_>>();
        let durations = splits.iter().map(|s| s.split_seconds).collect::<Vec<_>>();
        for (actual, expected) in cumulative.iter().zip([37.727, 67.846, 98.483, 128.876, 164.015]) {
            assert!((actual - expected).abs() < 0.000_001);
        }
        for (actual, expected) in durations.iter().zip([37.727, 30.119, 30.637, 30.393, 35.139]) {
            assert!((actual - expected).abs() < 0.000_001);
        }
    }

    #[test]
    fn early_success_marker_is_not_a_split_or_a_completed_time() {
        let start = event(44.224, EventKind::MissionStarted);
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(event(107.041, EventKind::MissionSucceeded));
        assert_eq!(session.status, MissionStatus::InProgress);
        assert!(phase_splits(&session).is_empty());

        session.apply(event(185.418, EventKind::BossTransmission {
            kind: TransmissionKind::LaserKill,
        }));
        session.apply(event(208.239, EventKind::MissionSucceeded));
        assert_eq!(session.status, MissionStatus::Succeeded);
        let splits = phase_splits(&session);
        assert_eq!(splits.len(), 1);
        assert_eq!(splits[0].key, "settlement");
        assert!((session.duration_seconds().unwrap() - 164.015).abs() < 0.000_001);
    }

    #[test]
    fn average_splits_compare_only_matching_five_stage_markers() {
        let first = completed_run(0.0, [20.0, 50.0, 80.0, 110.0, 150.0]);
        let second = completed_run(0.0, [18.0, 48.0, 78.0, 108.0, 148.0]);
        let averages = average_phase_splits(&[first, second]);
        assert_eq!(averages.len(), 5);
        assert!(averages.iter().any(|(key, average, count)| key == "run" && *average == 19.0 && *count == 2));
    }
}
