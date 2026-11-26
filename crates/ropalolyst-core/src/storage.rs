use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::{MissionStatus, PhaseSplit, phase_splits};

use crate::MissionSession;

/// Small local SQLite history. Only normalized run data is stored; raw EE.log
/// lines are never copied into the database.
pub struct RunStore {
    connection: Connection,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryRecord {
    pub id: i64,
    pub name: String,
    pub favorite: bool,
    pub started_utc: Option<String>,
    pub status: MissionStatus,
    pub duration_seconds: Option<f64>,
    pub splits: Vec<PhaseSplit>,
    pub session: MissionSession,
}

impl RunStore {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        if let Some(parent) = path.as_ref().parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|error| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(error))
            })?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=3000;
             CREATE TABLE IF NOT EXISTS runs (
                log_generation TEXT NOT NULL,

                id INTEGER PRIMARY KEY,
                started_at REAL NOT NULL,
                ended_at REAL,
                status TEXT NOT NULL,
                duration_seconds REAL,
                payload_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_runs_status_started ON runs(status, started_at);",
        )?;
        let has_generation: bool = {
            let mut statement = connection.prepare("PRAGMA table_info(runs)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            columns.filter_map(Result::ok).any(|name| name == "log_generation")
        };
        if !has_generation {
            connection.execute(
                "ALTER TABLE runs ADD COLUMN log_generation TEXT NOT NULL DEFAULT 'legacy'",
                [],
            )?;
        }
        connection.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_runs_generation_start ON runs(log_generation, started_at)",
            [],
        )?;
        let mut columns = Vec::new();
        {
            let mut stmt = connection.prepare("PRAGMA table_info(runs)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            for row in rows { columns.push(row?); }
        }
        for (column, sql) in [
            ("name", "ALTER TABLE runs ADD COLUMN name TEXT"),
            ("favorite", "ALTER TABLE runs ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0"),
            ("deleted", "ALTER TABLE runs ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0"),
            ("started_utc", "ALTER TABLE runs ADD COLUMN started_utc TEXT"),
        ] {
            if !columns.iter().any(|existing| existing == column) {
                connection.execute_batch(sql)?;
            }
        }
        connection.execute_batch("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")?;
        Ok(Self { connection })
    }

    pub fn save(&self, session: &MissionSession, log_generation: &str) -> rusqlite::Result<()> {
        self.save_with_utc(session, log_generation, None)
    }

    pub fn save_with_utc(&self, session: &MissionSession, log_generation: &str, started_utc: Option<&str>) -> rusqlite::Result<()> {
        let mut sanitized = session.clone();
        for event in &mut sanitized.events {
            event.source.clear();
        }
        let payload = serde_json::to_string(&sanitized)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.connection.execute(
            "INSERT OR IGNORE INTO runs (log_generation, started_at, ended_at, status, duration_seconds, payload_json, started_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                log_generation,
                session.started_at.0,
                session.ended_at.map(|timestamp| timestamp.0),
                format!("{:?}", session.status),
                session.duration_seconds(),
                payload,
                started_utc,
            ],
        )?;
        Ok(())
    }

    pub fn successful_sessions(&self) -> rusqlite::Result<Vec<MissionSession>> {
        let mut statement = self.connection.prepare(
            "SELECT payload_json FROM runs WHERE status = 'Succeeded' AND deleted = 0 ORDER BY started_at"
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut sessions = Vec::new();
        for row in rows {
            let payload = row?;
            if let Ok(session) = serde_json::from_str::<MissionSession>(&payload) {
                sessions.push(session);
            }
        }
        Ok(sessions)
    }

    pub fn completed_count(&self) -> rusqlite::Result<u64> {
        self.connection.query_row("SELECT COUNT(*) FROM runs WHERE deleted = 0", [], |row| row.get(0))
    }


    /// Completed and interrupted sessions, including records from older CLI versions.
    /// A deletion leaves its unique generation/start key in SQLite as a tombstone.
    pub fn history(&self) -> rusqlite::Result<Vec<HistoryRecord>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, payload_json, name, favorite, started_utc FROM runs WHERE deleted = 0 ORDER BY id DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?, row.get::<_, bool>(3)?,
                row.get::<_, Option<String>>(4)?))
        })?;
        let mut history = Vec::new();
        for row in rows {
            let (id, json, name, favorite, started_utc) = row?;
            let session: MissionSession = match serde_json::from_str(&json) {
                Ok(session) => session,
                Err(_) => continue,
            };
            let status = session.status;
            let duration_seconds = session.duration_seconds();
            history.push(HistoryRecord {
                id,
                name: name.unwrap_or_else(|| format!("Run #{id}")),
                favorite,
                started_utc,
                status,
                duration_seconds,
                splits: phase_splits(&session),
                session,
            });
        }
        Ok(history)
    }

    pub fn rename(&self, id: i64, name: &str) -> rusqlite::Result<()> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(rusqlite::Error::InvalidParameterName("记录名称长度需为 1–100 个字符".into()));
        }
        self.connection.execute("UPDATE runs SET name=?1 WHERE id=?2 AND deleted=0", params![name, id])?;
        Ok(())
    }

    pub fn toggle_favorite(&self, id: i64) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE runs SET favorite=1-favorite WHERE id=?1 AND deleted=0", [id]
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        self.connection.execute("UPDATE runs SET deleted=1 WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.connection.query_row("SELECT value FROM settings WHERE key=?1", [key], |row| row.get(0)).optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key,value],
        )?;
        Ok(())
    }

    pub fn best_success_seconds(&self) -> rusqlite::Result<Option<f64>> {
        Ok(self.successful_sessions()?.iter()
            .filter(|session| session.has_final_kill_evidence())
            .filter_map(MissionSession::duration_seconds)
            .min_by(f64::total_cmp))
    }

    pub fn average_success_seconds(&self) -> rusqlite::Result<Option<f64>> {
        let values: Vec<f64> = self.successful_sessions()?.iter()
            .filter(|session| session.has_final_kill_evidence())
            .filter_map(MissionSession::duration_seconds)
            .collect();
        if values.is_empty() {
            Ok(None)
        } else {
            Ok(Some(values.iter().sum::<f64>() / values.len() as f64))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventConfidence, EventKind, GameEvent, LogTimestamp};



    #[test]
    fn renamed_favorite_record_stays_deleted_after_log_replay() {
        let store = RunStore::open(":memory:").unwrap();
        let start = GameEvent::new(
            LogTimestamp(50.0), EventKind::MissionStarted,
            EventConfidence::Confirmed, "private raw line",
        );
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(GameEvent::new(
            LogTimestamp(150.0),
            EventKind::BossTransmission { kind: crate::TransmissionKind::LaserKill },
            EventConfidence::Confirmed, "private source",
        ));
        session.apply(GameEvent::new(
            LogTimestamp(200.0), EventKind::MissionSucceeded,
            EventConfidence::Confirmed, "private source",
        ));
        store.save_with_utc(&session, "generation", Some("2026-11-22T08:00:00Z")).unwrap();
        let before = store.history().unwrap();
        assert_eq!(before.len(), 1);
        assert!(!before[0].favorite);
        assert_eq!(before[0].started_utc.as_deref(), Some("2026-11-22T08:00:00Z"));
        assert!(!before[0].session.events[0].source.contains("private"));

        store.rename(before[0].id, "最佳成绩").unwrap();
        store.toggle_favorite(before[0].id).unwrap();
        let updated = store.history().unwrap();
        assert_eq!(updated[0].name, "最佳成绩");
        assert!(updated[0].favorite);

        store.delete(before[0].id).unwrap();
        // A historical replay uses the same stable session identity, and
        // cannot resurrect a user's deliberately deleted run.
        store.save(&session, "generation").unwrap();
        assert!(store.history().unwrap().is_empty());
        assert_eq!(store.completed_count().unwrap(), 0);
    }

    #[test]
    fn excludes_legacy_premature_success_from_best_time() {
        let store = RunStore::open(":memory:").unwrap();

        let start = GameEvent::new(
            LogTimestamp(0.0),
            EventKind::MissionStarted,
            EventConfidence::Confirmed,
            "fixture",
        );
        let mut legacy = MissionSession::new(start.timestamp, start);
        legacy.apply(GameEvent::new(
            LogTimestamp(62.817),
            EventKind::MissionSucceeded,
            EventConfidence::Confirmed,
            "fixture",
        ));
        // Simulate an old database row written before final-kill validation.
        legacy.status = crate::MissionStatus::Succeeded;
        legacy.ended_at = Some(LogTimestamp(62.817));
        store.save(&legacy, "legacy").unwrap();

        let start = GameEvent::new(
            LogTimestamp(100.0),
            EventKind::MissionStarted,
            EventConfidence::Confirmed,
            "fixture",
        );
        let mut valid = MissionSession::new(start.timestamp, start);
        valid.apply(GameEvent::new(
            LogTimestamp(200.0),
            EventKind::BossTransmission {
                kind: crate::TransmissionKind::LaserKill,
            },
            EventConfidence::Confirmed,
            "fixture",
        ));
        valid.apply(GameEvent::new(
            LogTimestamp(264.015),
            EventKind::MissionSucceeded,
            EventConfidence::Confirmed,
            "fixture",
        ));
        store.save(&valid, "valid").unwrap();

        assert_eq!(store.best_success_seconds().unwrap(), Some(164.015));
        assert_eq!(store.average_success_seconds().unwrap(), Some(164.015));
    }

    #[test]
    fn stores_completed_session_without_raw_log_dependency() {
        let store = RunStore::open(":memory:").unwrap();
        let start = GameEvent::new(LogTimestamp(2.0), EventKind::MissionStarted, EventConfidence::Confirmed, "sanitized");
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(GameEvent::new(LogTimestamp(12.5), EventKind::MissionSucceeded, EventConfidence::Confirmed, "sanitized"));
        store.save(&session, "test-generation").unwrap();
        store.save(&session, "test-generation").unwrap();
        assert_eq!(store.completed_count().unwrap(), 1);
        let payload: String = store.connection.query_row(
            "SELECT payload_json FROM runs LIMIT 1", [], |row| row.get(0)
        ).unwrap();
        assert!(!payload.contains("sanitized"));
    }
}
