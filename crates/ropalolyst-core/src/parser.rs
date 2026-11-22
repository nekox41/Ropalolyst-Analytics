use crate::{EventConfidence, EventKind, GameEvent, LogTimestamp};

/// Parse one Warframe log line. Only event signatures observed in real
/// Ropalolyst logs are recognized; uncertain combat semantics stay candidates.
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

fn cinematic_index(line: &str, prefix: &str) -> Option<u8> {
    let start = line.find(prefix)? + prefix.len();
    let digits: String = line[start..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
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
    } else if line.contains("Cinematic ") && line.contains(" Start")
        && line.contains("RopalolystGasArenaIntro01_cin.fbx")
    {
        Some((EventKind::BossArenaIntroStarted, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Start")
        && line.contains("TowerCrashA_cin.fbx")
    {
        let sequence = cinematic_index(line, "CrashCinematic").map(|n| n.saturating_add(1)).unwrap_or(0);
        Some((EventKind::BossCrashStarted { sequence }, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Stop")
        && line.contains("CrashCinematic")
    {
        let sequence = cinematic_index(line, "CrashCinematic").map(|n| n.saturating_add(1)).unwrap_or(0);
        Some((EventKind::BossCrashCinematicStopped { sequence }, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Start")
        && line.contains("TowerCrashEnd_cin.fbx")
    {
        Some((EventKind::BossTowerResetStarted, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Stop")
        && line.contains("ResetCinematic")
    {
        Some((EventKind::BossTowerResetEnded, EventConfidence::StrongCandidate))
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
        let event = parse_log_line("182.782 Net [Info]: GameRulesImpl - changing state from SS_WAITING_FOR_PLAYERS to SS_STARTED").unwrap();
        assert_eq!(event.timestamp, LogTimestamp(182.782));
        assert_eq!(event.kind, EventKind::MissionStarted);
        assert_eq!(event.confidence, EventConfidence::Confirmed);
    }

    #[test]
    fn parses_real_roparo_tower_crash_cinematics() {
        let first = parse_log_line("273.565 Sys [Info]: Cinematic /Layer8/Layer9/CrashCinematic0 Start, 1 scene(s), first: /Lotus/Animations/Sentient/Ropalolyst/Riding/TowerCrashA_cin.fbx").unwrap();
        assert_eq!(first.kind, EventKind::BossCrashStarted { sequence: 1 });
        let second = parse_log_line("309.369 Sys [Info]: Cinematic /Layer8/Layer9/CrashCinematic1 Start, 1 scene(s), first: /Lotus/Animations/Sentient/Ropalolyst/Riding/TowerCrashA_cin.fbx").unwrap();
        assert_eq!(second.kind, EventKind::BossCrashStarted { sequence: 2 });
        let third = parse_log_line("340.785 Sys [Info]: Cinematic /Layer8/Layer9/CrashCinematic2 Start, 1 scene(s), first: /Lotus/Animations/Sentient/Ropalolyst/Riding/TowerCrashA_cin.fbx").unwrap();
        assert_eq!(third.kind, EventKind::BossCrashStarted { sequence: 3 });
    }

    #[test]
    fn parses_tower_reset_and_crash_stop() {
        let stopped = parse_log_line("276.238 Sys [Info]: Cinematic /Layer8/Layer9/CrashCinematic0 Stop(stopFromSkipped=0)").unwrap();
        assert_eq!(stopped.kind, EventKind::BossCrashCinematicStopped { sequence: 1 });
        let reset = parse_log_line("276.243 Sys [Info]: Cinematic /Layer8/Layer9/ResetCinematic0 Start, 1 scene(s), first: /Lotus/Animations/Sentient/Ropalolyst/Riding/TowerCrashEnd_cin.fbx").unwrap();
        assert_eq!(reset.kind, EventKind::BossTowerResetStarted);
        let reset_end = parse_log_line("281.279 Sys [Info]: Cinematic /Layer8/Layer9/ResetCinematic0 Stop(stopFromSkipped=0)").unwrap();
        assert_eq!(reset_end.kind, EventKind::BossTowerResetEnded);
    }

    #[test]
    fn parses_boss_intro_and_defeat_transmission() {
        let intro = parse_log_line("213.105 Sys [Info]: Cinematic /Layer8/Layer9/Prefab1/LotusCinematic21 Start, 1 scene(s), first: /Lotus/Animations/Cinematics/BossIntro/RopalolystGasArenaIntro01_cin.fbx").unwrap();
        assert_eq!(intro.kind, EventKind::BossArenaIntroStarted);
        let defeat = parse_log_line("353.647 Sys [Info]: TRANSMISSION: Received Transmission: /Lotus/Sounds/Dialog/GasCityRopaFight/Natah/DRopaKillLaserNatah").unwrap();
        assert_eq!(defeat.kind, EventKind::BossDefeatedCandidate);
        assert_eq!(defeat.confidence, EventConfidence::Experimental);
    }

    #[test]
    fn ignores_unrecognized_or_timestamp_free_lines() {
        assert!(parse_log_line("this line has no timestamp SS_STARTED").is_none());
        assert!(parse_log_line("12.0 unrelated diagnostic").is_none());
        assert!(parse_log_line("181.583 Net [Info]: 1 /Lotus/Types/Enemies/Sentients/Ropalolyst/CrashCinematic").is_none());
    }
}
