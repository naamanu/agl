use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct TraceEvent {
    pub kind: String,
    pub timestamp_ms: u128,
    pub fields: Value,
}
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    events: Arc<Mutex<Vec<TraceEvent>>>,
    next_id: Arc<AtomicU64>,
    sleep_on_retry: bool,
    jitter_seed: u64,
}
impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            sleep_on_retry: true,
            jitter_seed: 0xA61,
        }
    }
}
impl ExecutionContext {
    /// A deterministic, non-sleeping context for conformance and evaluation.
    pub fn deterministic(seed: u64) -> Self {
        Self {
            sleep_on_retry: false,
            jitter_seed: seed,
            ..Self::default()
        }
    }

    pub fn next_id(&self, prefix: &str) -> String {
        format!(
            "{prefix}-{:016x}",
            self.next_id.fetch_add(1, Ordering::Relaxed)
        )
    }

    pub fn retry_delay_ms(
        &self,
        initial_ms: u64,
        max_ms: u64,
        multiplier: f64,
        jitter: f64,
        retry: u32,
        invocation_id: &str,
    ) -> u64 {
        let exponential = (initial_ms as f64 * multiplier.powi(retry.saturating_sub(1) as i32))
            .min(max_ms as f64);
        let mut hash = self.jitter_seed ^ u64::from(retry);
        for byte in invocation_id.bytes() {
            hash = hash
                .wrapping_mul(1_099_511_628_211)
                .wrapping_add(u64::from(byte));
        }
        let unit = (hash as f64 / u64::MAX as f64) * 2.0 - 1.0;
        (exponential * (1.0 + unit * jitter)).max(0.0).round() as u64
    }

    pub fn wait_retry(&self, delay_ms: u64) {
        if self.sleep_on_retry && delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
    }
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
