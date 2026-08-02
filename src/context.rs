use crate::event_store::{EventStore, EventStoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Condvar;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub kind: String,
    pub timestamp_ms: u128,
    pub fields: Value,
}
#[derive(Clone)]
pub struct ExecutionContext {
    events: Arc<Mutex<Vec<TraceEvent>>>,
    next_id: Arc<AtomicU64>,
    sleep_on_retry: bool,
    jitter_seed: u64,
    execution_id: Option<String>,
    store: Option<Arc<dyn EventStore>>,
    replay: Arc<Mutex<BTreeMap<String, VecDeque<Value>>>>,
    approvals: Arc<BTreeMap<String, ApprovalDecision>>,
    groups: Arc<(Mutex<BTreeMap<String, GroupState>>, Condvar)>,
    secret_values: Arc<Vec<String>>,
    persistence_error: Arc<Mutex<Option<String>>>,
}
impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            sleep_on_retry: true,
            jitter_seed: 0xA61,
            execution_id: None,
            store: None,
            replay: Arc::new(Mutex::new(BTreeMap::new())),
            approvals: Arc::new(BTreeMap::new()),
            groups: Arc::new((Mutex::new(BTreeMap::new()), Condvar::new())),
            secret_values: Arc::new(Vec::new()),
            persistence_error: Arc::new(Mutex::new(None)),
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

    pub fn durable(
        execution_id: impl Into<String>,
        store: Arc<dyn EventStore>,
        resume: bool,
    ) -> Result<Self, EventStoreError> {
        let execution_id = execution_id.into();
        let prior = if resume {
            store.load(&execution_id)?
        } else {
            Vec::new()
        };
        let mut replay: BTreeMap<String, VecDeque<Value>> = BTreeMap::new();
        for event in &prior {
            if event.kind == "task_result"
                && let (Some(id), Some(result)) = (
                    event.fields.get("invocation_id").and_then(Value::as_str),
                    event.fields.get("result"),
                )
            {
                replay
                    .entry(id.into())
                    .or_default()
                    .push_back(result.clone());
            }
        }
        Ok(Self {
            events: Arc::new(Mutex::new(prior)),
            execution_id: Some(execution_id),
            store: Some(store),
            replay: Arc::new(Mutex::new(replay)),
            ..Self::default()
        })
    }

    pub fn with_approvals(mut self, approvals: BTreeMap<String, bool>) -> Self {
        let decided_at_ms = now_ms();
        self.approvals = Arc::new(
            approvals
                .into_iter()
                .map(|(name, approved)| {
                    (
                        name,
                        ApprovalDecision {
                            approved,
                            actor: "host".into(),
                            decided_at_ms,
                        },
                    )
                })
                .collect(),
        );
        self
    }

    pub fn with_secrets(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.secret_values = Arc::new(
            values
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect(),
        );
        self
    }

    pub fn execution_id(&self) -> Option<&str> {
        self.execution_id.as_deref()
    }
    pub fn approval(&self, name: &str) -> Option<&ApprovalDecision> {
        self.approvals.get(name)
    }
    pub fn approval_expired(&self, name: &str, expires_seconds: Option<u64>) -> bool {
        let Some(expires) = expires_seconds else {
            return false;
        };
        let suspended_at = self
            .events
            .lock()
            .unwrap()
            .iter()
            .find(|event| {
                event.kind == "human_suspended"
                    && event.fields.get("approval").and_then(Value::as_str) == Some(name)
            })
            .map(|event| event.timestamp_ms);
        suspended_at
            .is_some_and(|started| now_ms().saturating_sub(started) > u128::from(expires) * 1000)
    }
    pub fn take_replay(&self, invocation_id: &str) -> Option<Value> {
        self.replay
            .lock()
            .unwrap()
            .get_mut(invocation_id)
            .and_then(VecDeque::pop_front)
    }
    pub fn persistence_error(&self) -> Option<String> {
        self.persistence_error.lock().unwrap().clone()
    }

    pub fn acquire_group(
        &self,
        name: &str,
        limit: u32,
        rate_per_second: Option<u32>,
    ) -> GroupPermit {
        let (lock, condition) = &*self.groups;
        let mut groups = lock.lock().unwrap();
        loop {
            let state = groups.entry(name.into()).or_default();
            let rate_ready = !self.sleep_on_retry
                || rate_per_second.is_none_or(|rate| {
                    state.last_start.elapsed()
                        >= std::time::Duration::from_secs_f64(1.0 / f64::from(rate))
                });
            if state.active < limit && rate_ready {
                state.active += 1;
                state.last_start = std::time::Instant::now();
                break;
            }
            if state.active >= limit {
                groups = condition.wait(groups).unwrap();
            } else {
                let wait = rate_per_second
                    .map(|rate| std::time::Duration::from_secs_f64(1.0 / f64::from(rate)))
                    .unwrap_or_default();
                drop(groups);
                if self.sleep_on_retry {
                    std::thread::sleep(wait);
                } else {
                    std::thread::yield_now();
                }
                groups = lock.lock().unwrap();
            }
        }
        GroupPermit {
            name: name.into(),
            groups: self.groups.clone(),
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
    pub fn record(&self, kind: impl Into<String>, mut fields: Value) {
        redact(&mut fields, &self.secret_values);
        let timestamp_ms = now_ms();
        let event = TraceEvent {
            kind: kind.into(),
            timestamp_ms,
            fields,
        };
        self.events.lock().unwrap().push(event.clone());
        if let (Some(store), Some(execution_id)) = (&self.store, &self.execution_id) {
            // The trace remains available even if persistence fails; the explicit
            // event makes the durability fault auditable without panicking.
            if let Err(error) = store.append(execution_id, &event) {
                *self.persistence_error.lock().unwrap() = Some(error.to_string());
                self.events.lock().unwrap().push(TraceEvent {
                    kind: "event_store_error".into(),
                    timestamp_ms,
                    fields: serde_json::json!({"message":error.to_string()}),
                });
            }
        }
    }
    pub fn events(&self) -> Vec<TraceEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalDecision {
    pub approved: bool,
    pub actor: String,
    pub decided_at_ms: u128,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn redact(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(text) => {
            for secret in secrets {
                if text.contains(secret) {
                    *text = text.replace(secret, "[REDACTED]");
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| redact(item, secrets)),
        Value::Object(fields) => {
            for (name, value) in fields {
                if matches!(
                    name.to_ascii_lowercase().as_str(),
                    "api_key" | "authorization" | "password" | "secret" | "token"
                ) {
                    *value = Value::String("[REDACTED]".into());
                } else {
                    redact(value, secrets);
                }
            }
        }
        _ => {}
    }
}

struct GroupState {
    active: u32,
    last_start: std::time::Instant,
}
impl Default for GroupState {
    fn default() -> Self {
        Self {
            active: 0,
            last_start: std::time::Instant::now() - std::time::Duration::from_secs(3600),
        }
    }
}
pub struct GroupPermit {
    name: String,
    groups: Arc<(Mutex<BTreeMap<String, GroupState>>, Condvar)>,
}
impl Drop for GroupPermit {
    fn drop(&mut self) {
        let (lock, condition) = &*self.groups;
        if let Some(state) = lock.lock().unwrap().get_mut(&self.name) {
            state.active = state.active.saturating_sub(1);
        }
        condition.notify_all();
    }
}
