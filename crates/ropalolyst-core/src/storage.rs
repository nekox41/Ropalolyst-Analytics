use std::path::Path;

use rusqlite::{Connection, params};

use crate::MissionSession;

/// Small local SQLite history. Only normalized run data is stored; raw EE.log
/// lines are never copied into the database.
pub struct RunStore {
    connection: Connection,
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
        Ok(Self { connection })
    }

    pub fn save(&self, session: &MissionSession, log_generation: &str) -> rusqlite::Result<()> {
        let mut sanitized = session.clone();
        for event in &mut sanitized.events {
            event.source.clear();
        }
        let payload = serde_json::to_string(&sanitized)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.connection.execute(
            "INSERT OR IGNORE INTO runs (log_generation, started_at, ended_at, status, duration_seconds, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                log_generation,
                session.started_at.0,
                session.ended_at.map(|timestamp| timestamp.0),
                format!("{:?}", session.status),
                session.duration_seconds(),
                payload,
            ],
        )?;
        Ok(())
    }

    pub fn successful_sessions(&self) -> rusqlite::Result<Vec<MissionSession>> {
        let mut statement = self.connection.prepare(
            "SELECT payload_json FROM runs WHERE status = 'Succeeded' ORDER BY started_at"
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
        self.connection.query_row("SELECT COUNT(*) FROM runs", [], |row| row.get(0))
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
