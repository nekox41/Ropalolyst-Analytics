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
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(error))
            })?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS runs (
                id INTEGER PRIMARY KEY,
                started_at REAL NOT NULL,
                ended_at REAL,
                status TEXT NOT NULL,
                duration_seconds REAL,
                payload_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_runs_status_started ON runs(status, started_at);",
        )?;
        Ok(Self { connection })
    }

    pub fn save(&self, session: &MissionSession) -> rusqlite::Result<()> {
        let payload = serde_json::to_string(session)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.connection.execute(
            "INSERT INTO runs (started_at, ended_at, status, duration_seconds, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session.started_at.0,
                session.ended_at.map(|timestamp| timestamp.0),
                format!("{:?}", session.status),
                session.duration_seconds(),
                payload,
            ],
        )?;
        Ok(())
    }

    pub fn completed_count(&self) -> rusqlite::Result<u64> {
        self.connection.query_row("SELECT COUNT(*) FROM runs", [], |row| row.get(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventConfidence, EventKind, GameEvent, LogTimestamp};

    #[test]
    fn stores_completed_session_without_raw_log_dependency() {
        let store = RunStore::open(":memory:").unwrap();
        let start = GameEvent::new(LogTimestamp(2.0), EventKind::MissionStarted, EventConfidence::Confirmed, "sanitized");
        let mut session = MissionSession::new(start.timestamp, start);
        session.apply(GameEvent::new(LogTimestamp(12.5), EventKind::MissionSucceeded, EventConfidence::Confirmed, "sanitized"));
        store.save(&session).unwrap();
        assert_eq!(store.completed_count().unwrap(), 1);
    }
}
