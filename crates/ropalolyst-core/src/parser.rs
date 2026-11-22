use crate::{EventConfidence, EventKind, GameEvent, LogTimestamp, TransmissionKind};

/// 解析单行 EE.log。只识别有静态资源/脚本证据支持的签名；不把传讯请求误判为动作完成。
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

fn instance_id(line: &str, prefix: &str) -> Option<u8> {
    let start = line.find(prefix)? + prefix.len();
    let digits: String = line[start..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn transmission(line: &str) -> Option<(TransmissionKind, EventConfidence)> {
    let (suffix, kind, confidence) = if line.contains("AladV/DShieldDownAladV") {
        ("DShieldDownAladV", TransmissionKind::ShieldHint, EventConfidence::Confirmed)
    } else if line.contains("Natah/DOperRemoveShield0190Natah") {
        ("DOperRemoveShield0190Natah", TransmissionKind::FirstShieldDown, EventConfidence::Confirmed)
    } else if line.contains("AladV/DCapaChargeAladV") {
        ("DCapaChargeAladV", TransmissionKind::ConduitChargeReminder, EventConfidence::Confirmed)
    } else if line.contains("AladV/DShootWeakPntsAladV") {
        ("DShootWeakPntsAladV", TransmissionKind::WeakpointHint, EventConfidence::Confirmed)
    } else if line.contains("AladV/DRopaLandPlatAladV") {
        ("DRopaLandPlatAladV", TransmissionKind::FinalWeakpointHint, EventConfidence::Confirmed)
    } else if line.contains("AladV/DMntRopa0400AladV") {
        ("DMntRopa0400AladV", TransmissionKind::FinalMountAttempt, EventConfidence::Confirmed)
    } else if line.contains("AladV/DRopaStunAladV") {
        ("DRopaStunAladV", TransmissionKind::StunHint, EventConfidence::Confirmed)
    } else if line.contains("Natah/DHitStunRopaNatah") {
        ("DHitStunRopaNatah", TransmissionKind::LaserHit, EventConfidence::Confirmed)
    } else if line.contains("AladV/DLaserMissAladV") {
        ("DLaserMissAladV", TransmissionKind::LaserMiss, EventConfidence::Confirmed)
    } else if line.contains("Natah/DRopaKillLaserNatah") {
        ("DRopaKillLaserNatah", TransmissionKind::LaserKill, EventConfidence::StrongCandidate)
    } else {
        return None;
    };
    let _ = suffix;
    Some((kind, confidence))
}

fn classify(line: &str) -> Option<(EventKind, EventConfidence)> {
    if line.contains("TRANSMISSION: Received Transmission:") {
        if let Some((kind, confidence)) = transmission(line) {
            return Some((EventKind::BossTransmission { kind }, confidence));
        }
    }

    if line.contains("changing state from ") && line.contains(" to SS_STARTED") {
        Some((EventKind::MissionStarted, EventConfidence::Confirmed))
    } else if line.contains("Mission Succeeded") {
        Some((EventKind::MissionSucceeded, EventConfidence::Confirmed))
    } else if line.contains("Mission Failed") {
        Some((EventKind::MissionFailed, EventConfidence::StrongCandidate))
    } else if line.contains("changing state from SS_STARTED to SS_ENDING") {
        Some((EventKind::MissionEnding, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Start")
        && line.contains("RopalolystGasArenaIntro01_cin.fbx")
    {
        Some((EventKind::BossArenaIntroStarted, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Stop")
        && line.contains("LotusCinematic21")
    {
        Some((EventKind::BossArenaIntroEnded, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Start")
        && line.contains("TowerCrashA_cin.fbx")
    {
        // CrashCinematic0/1/2 对应地图中的电塔 1/2/3；这是塔 ID，不是撞塔轮次。
        let tower_id = instance_id(line, "CrashCinematic")?.checked_add(1)?;
        Some((EventKind::BossCrashStarted { tower_id, cycle: 0 }, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Stop")
        && line.contains("CrashCinematic")
    {
        let tower_id = instance_id(line, "CrashCinematic")?.checked_add(1)?;
        Some((EventKind::BossCrashCinematicStopped { tower_id }, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Start")
        && line.contains("ResetCinematic") && line.contains("TowerCrashEnd_cin.fbx")
    {
        let cinematic_id = instance_id(line, "ResetCinematic")?;
        Some((EventKind::BossTowerResetStarted { cinematic_id }, EventConfidence::StrongCandidate))
    } else if line.contains("Cinematic ") && line.contains(" Stop")
        && line.contains("ResetCinematic")
    {
        let cinematic_id = instance_id(line, "ResetCinematic")?;
        if cinematic_id == 1 {
            Some((EventKind::BossLandingEnded { cinematic_id }, EventConfidence::StrongCandidate))
        } else {
            Some((EventKind::BossTowerResetEnded { cinematic_id }, EventConfidence::StrongCandidate))
        }
    } else if line.contains("Cinematic ") && line.contains(" Start")
        && line.contains("ResetCinematic1") && line.contains("TennoLand_cin.fbx")
    {
        Some((EventKind::BossLandingStarted { cinematic_id: 1 }, EventConfidence::StrongCandidate))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_start_and_end_are_distinct_transitions() {
        let start = parse_log_line("42.312 Net [Info]: GameRulesImpl - changing state from SS_WAITING_FOR_PLAYERS to SS_STARTED").unwrap();
        assert_eq!(start.kind, EventKind::MissionStarted);
        let end = parse_log_line("208.392 Net [Info]: GameRulesImpl - changing state from SS_STARTED to SS_ENDING").unwrap();
        assert_eq!(end.kind, EventKind::MissionEnding);
    }

    #[test]
    fn tower_identity_is_not_the_crash_cycle() {
        for (instance, tower_id) in [(0, 1), (1, 2), (2, 3)] {
            let line = format!("98.866 Sys [Info]: Cinematic /Layer8/Layer9/CrashCinematic{instance} Start, 1 scene(s), first: /Lotus/Animations/Sentient/Ropalolyst/Riding/TowerCrashA_cin.fbx");
            let event = parse_log_line(&line).unwrap();
            assert_eq!(event.kind, EventKind::BossCrashStarted { tower_id });
        }
    }

    #[test]
    fn reset_cinematic_one_is_the_landing_animation() {
        let reset = parse_log_line("101.540 Sys [Info]: Cinematic /Layer8/Layer9/ResetCinematic0 Start, 1 scene(s), first: /Lotus/Animations/Sentient/Ropalolyst/Riding/TowerCrashEnd_cin.fbx").unwrap();
        assert_eq!(reset.kind, EventKind::BossTowerResetStarted { cinematic_id: 0 });
        let landing = parse_log_line("189.223 Sys [Info]: Cinematic /Layer8/Layer9/ResetCinematic1 Start, 1 scene(s), first: /Lotus/Animations/Sentient/Ropalolyst/Riding/TennoLand_cin.fbx").unwrap();
        assert_eq!(landing.kind, EventKind::BossLandingStarted { cinematic_id: 1 });
        let landing_end = parse_log_line("194.223 Sys [Info]: Cinematic /Layer8/Layer9/ResetCinematic1 Stop(stopFromSkipped=0)").unwrap();
        assert_eq!(landing_end.kind, EventKind::BossLandingEnded { cinematic_id: 1 });
    }

    #[test]
    fn transmission_events_keep_their_script_semantics() {
        let charge = parse_log_line("20.0 Sys [Info]: TRANSMISSION: Received Transmission: /Lotus/Sounds/Dialog/GasCityRopaFight/AladV/DCapaChargeAladV").unwrap();
        assert_eq!(charge.kind, EventKind::BossTransmission { kind: TransmissionKind::ConduitChargeReminder });
        let shield_hint = parse_log_line("21.0 Sys [Info]: TRANSMISSION: Received Transmission: /Lotus/Sounds/Dialog/GasCityRopaFight/AladV/DShieldDownAladV").unwrap();
        assert_eq!(shield_hint.kind, EventKind::BossTransmission { kind: TransmissionKind::ShieldHint });
        let kill = parse_log_line("22.0 Sys [Info]: TRANSMISSION: Received Transmission: /Lotus/Sounds/Dialog/GasCityRopaFight/Natah/DRopaKillLaserNatah").unwrap();
        assert_eq!(kill.kind, EventKind::BossTransmission { kind: TransmissionKind::LaserKill });
        assert_eq!(kill.confidence, EventConfidence::StrongCandidate);
    }

    #[test]
    fn unknown_transmissions_and_unrelated_lines_are_ignored() {
        assert!(parse_log_line("12.0 unrelated diagnostic").is_none());
        assert!(parse_log_line("20.0 Sys [Info]: TRANSMISSION: Received Transmission: /unknown/path").is_none());
    }
}
