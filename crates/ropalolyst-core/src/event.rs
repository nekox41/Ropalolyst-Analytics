use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Warframe EE.log 中的进程相对时间，单位为秒。
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LogTimestamp(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventConfidence {
    Confirmed,
    StrongCandidate,
    Experimental,
}

/// 已从传讯配置确认的资源语义。它们是脚本请求传讯的信号，不一定等价于动作完成。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransmissionKind {
    ShieldHint,
    FirstShieldDown,
    ConduitChargeReminder,
    WeakpointHint,
    FinalWeakpointHint,
    FinalMountAttempt,
    StunHint,
    LaserHit,
    LaserMiss,
    LaserKill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    MissionStarted,
    MissionSucceeded,
    MissionFailed,
    MissionEnding,
    BossArenaIntroStarted,
    BossArenaIntroEnded,
    BossCrashStarted { tower_id: u8, cycle: u8 },
    BossCrashCinematicStopped { tower_id: u8 },
    BossTowerResetStarted { cinematic_id: u8 },
    BossTowerResetEnded { cinematic_id: u8 },
    BossLandingStarted { cinematic_id: u8 },
    BossLandingEnded { cinematic_id: u8 },
    BossTransmission { kind: TransmissionKind },
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent {
    pub timestamp: LogTimestamp,
    pub kind: EventKind,
    pub confidence: EventConfidence,
    /// 原始日志行，仅用于内存诊断；持久化时会清除。
    pub source: String,
    /// 读取到事件的墙上时钟，不用于成绩计时。
    pub observed_at: DateTime<Utc>,
}

impl GameEvent {
    pub fn new(
        timestamp: LogTimestamp,
        kind: EventKind,
        confidence: EventConfidence,
        source: impl Into<String>,
    ) -> Self {
        Self {
            timestamp,
            kind,
            confidence,
            source: source.into(),
            observed_at: Utc::now(),
        }
    }
}
