use crate::context::TraceEvent;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventStoreError {
    #[error("event store I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("event store serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("sqlite event store failed: {0}")]
    Sqlite(String),
}

pub trait EventStore: Send + Sync {
    fn append(&self, execution_id: &str, event: &TraceEvent) -> Result<(), EventStoreError>;
    fn load(&self, execution_id: &str) -> Result<Vec<TraceEvent>, EventStoreError>;
    fn prune_before(&self, timestamp_ms: u128) -> Result<u64, EventStoreError>;
}

/// Local durable event store backed by the system SQLite implementation.
///
/// Keeping the boundary behind `EventStore` lets embedders use `rusqlite` or a
/// remote store without making either part of the language semantics.
#[derive(Debug, Clone)]
pub struct SqliteEventStore {
    path: PathBuf,
}

impl SqliteEventStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, EventStoreError> {
        let store = Self { path: path.into() };
        store.exec("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS agl_events (seq INTEGER PRIMARY KEY AUTOINCREMENT, execution_id TEXT NOT NULL, kind TEXT NOT NULL, timestamp_ms TEXT NOT NULL, fields_json TEXT NOT NULL); CREATE INDEX IF NOT EXISTS agl_events_execution ON agl_events(execution_id, seq); PRAGMA user_version=1;")?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn exec(&self, sql: &str) -> Result<String, EventStoreError> {
        let output = Command::new("sqlite3")
            .arg("-batch")
            .arg(&self.path)
            .arg(sql)
            .output()?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(EventStoreError::Sqlite(
                String::from_utf8_lossy(&output.stderr).trim().into(),
            ))
        }
    }
}

impl EventStore for SqliteEventStore {
    fn append(&self, execution_id: &str, event: &TraceEvent) -> Result<(), EventStoreError> {
        let fields = serde_json::to_string(&event.fields)?;
        self.exec(&format!("INSERT INTO agl_events(execution_id,kind,timestamp_ms,fields_json) VALUES('{}','{}','{}','{}');", sql(execution_id), sql(&event.kind), event.timestamp_ms, sql(&fields)))?;
        Ok(())
    }

    fn load(&self, execution_id: &str) -> Result<Vec<TraceEvent>, EventStoreError> {
        let output = self.exec(&format!("SELECT json_object('kind',kind,'timestamp_ms',CAST(timestamp_ms AS INTEGER),'fields',json(fields_json)) FROM agl_events WHERE execution_id='{}' ORDER BY seq;", sql(execution_id)))?;
        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(EventStoreError::from))
            .collect()
    }

    fn prune_before(&self, timestamp_ms: u128) -> Result<u64, EventStoreError> {
        let output = self.exec(&format!("DELETE FROM agl_events WHERE CAST(timestamp_ms AS INTEGER) < {timestamp_ms}; SELECT changes();"))?;
        Ok(output
            .lines()
            .last()
            .and_then(|line| line.parse().ok())
            .unwrap_or(0))
    }
}

fn sql(value: &str) -> String {
    value.replace('\'', "''")
}
