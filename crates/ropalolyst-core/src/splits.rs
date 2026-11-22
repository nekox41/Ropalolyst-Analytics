use serde::{Deserialize, Serialize};

use crate::{EventKind, MissionSession, MissionStatus};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseSplit {
    pub key: String,
    pub label: String,
    /// 累计时间：相对 MissionStarted。
    pub cumulative_seconds: f64,
    /// 阶段耗时：相对上一个有效分段点。
    pub split_seconds: f64,
}

/// 从任务事件中提取可用于竞速的分段点。
/// 只使用动画边界、明确的任务边界和已映射的最终处决结果。
pub fn phase_splits(session: &MissionSession) -> Vec<PhaseSplit> {
    let mut result = Vec::new();
    let mut previous = 0.0;
    let mut reset_count = 0_u8;

    for event in &session.events {
        let label_key = match &event.kind {
            EventKind::BossArenaIntroStarted => Some(("run_to_arena", "跑图结束")),
            EventKind::BossArenaIntroEnded => Some(("arena_intro", "Boss 出场结束")),
            EventKind::BossCrashCinematicStopped { tower_id } => {
                let key = format!("crash_animation_end_tower_{tower_id}");
                let label = format!("电塔 {tower_id} 撞击动画结束");
                let cumulative = event.timestamp.0 - session.started_at.0;
                let split = cumulative - previous;
                if cumulative >= previous {
                    result.push(PhaseSplit { key, label, cumulative_seconds: cumulative, split_seconds: split });
                    previous = cumulative;
                }
                None
            }
            EventKind::BossTowerResetStarted { cinematic_id } => {
                let key = format!("return_center_start_{cinematic_id}");
                let label = "返回中央动画开始".to_owned();
                let cumulative = event.timestamp.0 - session.started_at.0;
                let split = cumulative - previous;
                if cumulative >= previous {
                    result.push(PhaseSplit { key, label, cumulative_seconds: cumulative, split_seconds: split });
                    previous = cumulative;
                }
                None
            }
            EventKind::BossCrashStarted { cycle, tower_id } => {
                let key = format!("crash_{cycle}");
                let label = format!("第 {cycle} 次撞塔（电塔 {tower_id}）");
                let cumulative = event.timestamp.0 - session.started_at.0;
                let split = cumulative - previous;
                if cumulative >= previous {
                    result.push(PhaseSplit { key, label, cumulative_seconds: cumulative, split_seconds: split });
                    previous = cumulative;
                }
                None
            }
            EventKind::BossTowerResetEnded { .. } => {
                reset_count = reset_count.saturating_add(1);
                let key = format!("return_center_{reset_count}");
                let label = format!("第 {reset_count} 次返回中央");
                let cumulative = event.timestamp.0 - session.started_at.0;
                let split = cumulative - previous;
                if cumulative >= previous {
                    result.push(PhaseSplit { key, label, cumulative_seconds: cumulative, split_seconds: split });
                    previous = cumulative;
                }
                None
            }
            EventKind::BossTransmission { kind: crate::TransmissionKind::LaserKill } =>
                Some(("laser_kill", "激光处决结果")),
            EventKind::BossLandingStarted { .. } => Some(("landing_start", "处决后落地开始")),
            EventKind::BossLandingEnded { .. } => Some(("landing_end", "处决后落地结束")),
            EventKind::MissionSucceeded => Some(("mission_success", "任务成功")),
            _ => None,
        };

        if let Some((key, label)) = label_key {
            let cumulative = event.timestamp.0 - session.started_at.0;
            let split = cumulative - previous;
            if cumulative >= previous {
                result.push(PhaseSplit {
                    key: key.to_owned(),
                    label: label.to_owned(),
                    cumulative_seconds: cumulative,
                    split_seconds: split,
                });
                previous = cumulative;
            }
        }
    }
    result
}

/// 返回成功成绩中各分段点的平均阶段耗时。缺少该标记的旧成绩不参与该标记的平均值。
pub fn average_phase_splits(sessions: &[MissionSession]) -> Vec<(String, f64, usize)> {
    let mut totals: std::collections::BTreeMap<String, (f64, usize)> = std::collections::BTreeMap::new();
    for session in sessions.iter().filter(|s| s.status == MissionStatus::Succeeded) {
        for split in phase_splits(session) {
            let entry = totals.entry(split.key).or_insert((0.0, 0));
            entry.0 += split.split_seconds;
            entry.1 += 1;
        }
    }
    totals.into_iter()
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

    #[test]
    fn splits_are_relative_to_mission_start_and_previous_marker() {
        let start = event(100.0, EventKind::MissionStarted);
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(event(120.0, EventKind::BossArenaIntroStarted));
        session.apply(event(135.0, EventKind::BossArenaIntroEnded));
        session.apply(event(180.0, EventKind::BossCrashStarted { tower_id: 2, cycle: 1 }));
        session.apply(event(185.0, EventKind::BossCrashCinematicStopped { tower_id: 2 }));
        session.apply(event(186.0, EventKind::BossTowerResetStarted { cinematic_id: 0 }));
        session.apply(event(192.0, EventKind::BossTowerResetEnded { cinematic_id: 0 }));
        session.apply(event(240.0, EventKind::MissionSucceeded));

        let splits = phase_splits(&session);
        assert_eq!(splits.len(), 7);
        assert_eq!(splits[0].label, "跑图结束");
        assert_eq!(splits[0].cumulative_seconds, 20.0);
        assert_eq!(splits[0].split_seconds, 20.0);
        assert_eq!(splits[2].label, "第 1 次撞塔（电塔 2）");
        assert_eq!(splits[2].cumulative_seconds, 80.0);
        assert_eq!(splits[2].split_seconds, 45.0);
        assert_eq!(splits[3].split_seconds, 5.0);
        assert_eq!(splits[4].split_seconds, 1.0);
        assert_eq!(splits[5].split_seconds, 6.0);
        assert_eq!(splits[6].split_seconds, 48.0);
    }

    #[test]
    fn average_splits_compare_matching_markers_across_successful_runs() {
        let make = |end| {
            let start = event(0.0, EventKind::MissionStarted);
            let mut session = MissionSession::new(start.timestamp, start);
            session.apply(event(end, EventKind::BossArenaIntroStarted));
            session.apply(event(end + 10.0, EventKind::MissionSucceeded));
            session
        };
        let averages = average_phase_splits(&[make(20.0), make(18.0)]);
        assert_eq!(averages, vec![("mission_success".to_owned(), 10.0, 2), ("run_to_arena".to_owned(), 19.0, 2)]);
    }
}
