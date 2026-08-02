use serde::Serialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct TraceEvent {
    pub kind: String,
    pub timestamp_ms: u128,
    pub fields: Value,
}
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    events: Arc<Mutex<Vec<TraceEvent>>>,
}
impl ExecutionContext {
    pub fn record(&self, kind: impl Into<String>, fields: Value) {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        self.events.lock().unwrap().push(TraceEvent {
            kind: kind.into(),
            timestamp_ms,
            fields,
        })
    }
    pub fn events(&self) -> Vec<TraceEvent> {
        self.events.lock().unwrap().clone()
    }
}
