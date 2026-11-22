use crate::{EventConfidence, EventKind, GameEvent, LogTimestamp};

/// Parse the leading process-relative timestamp and classify only signatures
/// documented by the project's research notes. Unknown lines are ignored.
pub fn parse_log_line(line: &str) -> Option<GameEvent> {
    let timestamp = parse_timestamp(line)?;
    let (kind, confidence) = classify(line)?;
    Some(GameEvent::new(timestamp, kind, confidence, line.to_owned()))
}

fn parse_timestamp(line: &str) -> Option<LogTimestamp> {
    let token = line.split_whitespace().next()?;
    let token = token.trim_start_matches('[').trim_end_matches(']');
    token.parse::<f64>().ok().filter(|value| value.is_finite()).map(LogTimestamp)
}

fn classify(line: &str) -> Option<(EventKind, EventConfidence)> {
    if line.contains("SS_STARTED") {
        Some((EventKind::MissionStarted, EventConfidence::Confirmed))
    } else if line.contains("Mission Succeeded") {
        Some((EventKind::MissionSucceeded, EventConfidence::Confirmed))
    } else if line.contains("Mission Failed") {
        Some((EventKind::MissionFailed, EventConfidence::StrongCandidate))
    } else if line.contains("SS_ENDING") {
        Some((EventKind::MissionEnding, EventConfidence::StrongCandidate))
    } else if line.contains("RopalolystGasArenaIntro01_cin.fbx") {
        Some((EventKind::BossArenaIntroStarted, EventConfidence::StrongCandidate))
    } else if line.contains("TowerCrashA_cin.fbx") {
        Some((EventKind::BossCrashStarted { sequence: 1 }, EventConfidence::StrongCandidate))
    } else if line.contains("TowerCrashB_cin.fbx") {
        Some((EventKind::BossCrashStarted { sequence: 2 }, EventConfidence::StrongCandidate))
    } else if line.contains("TowerCrashC_cin.fbx") {
        Some((EventKind::BossCrashStarted { sequence: 3 }, EventConfidence::StrongCandidate))
    } else if line.contains("TowerCrashEnd_cin.fbx") {
        Some((EventKind::BossCrashEnded { sequence: 0 }, EventConfidence::StrongCandidate))
    } else if line.contains("DRopaKillLaserNatah") {
        Some((EventKind::BossDefeatedCandidate, EventConfidence::Experimental))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_confirmed_mission_start() {
        let event = parse_log_line("42.312 GameRules transition SS_STARTED").unwrap();
        assert_eq!(event.timestamp, LogTimestamp(42.312));
        assert_eq!(event.kind, EventKind::MissionStarted);
        assert_eq!(event.confidence, EventConfidence::Confirmed);
    }

    #[test]
    fn parses_success_and_candidate_boss_signals() {
        assert_eq!(
            parse_log_line("191.218 EndOfMatch.lua: Mission Succeeded").unwrap().kind,
            EventKind::MissionSucceeded
        );
        let candidate = parse_log_line("100.0 TowerCrashB_cin.fbx").unwrap();
        assert_eq!(candidate.kind, EventKind::BossCrashStarted { sequence: 2 });
        assert_eq!(candidate.confidence, EventConfidence::StrongCandidate);
    }

    #[test]
    fn ignores_unrecognized_or_timestamp_free_lines() {
        assert!(parse_log_line("this line has no timestamp SS_STARTED").is_none());
        assert!(parse_log_line("12.0 unrelated diagnostic").is_none());
    }
}
