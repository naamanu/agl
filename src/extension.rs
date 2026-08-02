use crate::ast::Program;
use crate::event_store::EventStore;
use crate::runtime::{HandlerFailure, Invocation, Registry, TaskOutput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const EXTENSION_API_VERSION: u32 = 1;
pub const PYTHON_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionDescriptor {
    pub api_version: u32,
    pub name: String,
    pub version: String,
}
impl ExtensionDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        if self.api_version == EXTENSION_API_VERSION {
            Ok(())
        } else {
            Err(format!(
                "extension '{}' requires API {}, host provides {}",
                self.name, self.api_version, EXTENSION_API_VERSION
            ))
        }
    }
}

pub trait TaskHandler: Send + Sync {
    fn execute(
        &self,
        args: &BTreeMap<String, Value>,
        agent: Option<&str>,
        invocation: &Invocation,
    ) -> Result<TaskOutput, HandlerFailure>;
}
pub trait ToolHandler: Send + Sync {
    fn execute(
        &self,
        args: &serde_json::Map<String, Value>,
        invocation: &Invocation,
    ) -> Result<Value, HandlerFailure>;
}
pub trait AgentAdapter: Send + Sync {
    fn provider(&self) -> &str;
    fn invoke(
        &self,
        model: &str,
        prompt: &str,
        invocation: &Invocation,
    ) -> Result<TaskOutput, HandlerFailure>;
}
pub trait PolicyResolver: Send + Sync {
    fn authorize(&self, request: &PolicyRequest) -> Result<(), String>;
}
pub trait Grader: Send + Sync {
    fn grade(&self, actual: &Value, expected: Option<&Value>) -> Result<f64, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRequest {
    pub execution_id: String,
    pub operation: String,
    pub effects: Vec<String>,
    pub metadata: Value,
}

pub trait Host {
    fn program(&self) -> &Program;
    fn registry(&mut self) -> &mut Registry;
    fn event_store(&self) -> Option<Arc<dyn EventStore>>;
    fn register_task(&mut self, name: impl Into<String>, handler: Arc<dyn TaskHandler>) {
        self.registry()
            .register_contextual(name, move |args, agent, invocation| {
                handler.execute(args, agent, invocation)
            });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolEnvelope<T> {
    pub protocol: u32,
    pub payload: T,
}
impl<T> ProtocolEnvelope<T> {
    pub fn new(payload: T) -> Self {
        Self {
            protocol: PYTHON_PROTOCOL_VERSION,
            payload,
        }
    }
}
