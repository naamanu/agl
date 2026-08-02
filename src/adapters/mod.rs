//! Live model adapter contracts and provider implementations.

mod anthropic;
mod openai;
pub mod tools;

pub use anthropic::AnthropicClient;
pub use openai::OpenAiClient;

use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub type ToolExecutor<'a> =
    dyn Fn(&str, &serde_json::Map<String, Value>) -> Result<Value, AdapterError> + Send + Sync + 'a;

#[derive(Debug, Clone)]
pub struct CompletionRequest<'a> {
    pub model: &'a str,
    pub prompt: &'a str,
    pub system: Option<&'a str>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AdapterError {
    #[error("{provider} HTTP {status}: {detail}")]
    Http {
        provider: &'static str,
        status: u16,
        detail: String,
    },
    #[error("{0} network error: {1}")]
    Network(&'static str, String),
    #[error("{0} response was not valid JSON: {1}")]
    InvalidJson(&'static str, String),
    #[error("{0} response contained no text output")]
    MissingText(&'static str),
    #[error("{provider} tool call for '{tool}' returned {problem}")]
    InvalidToolCall {
        provider: &'static str,
        tool: String,
        problem: String,
    },
    #[error("{0} tool-calling loop exceeded max_round_trips")]
    ToolLoopLimit(&'static str),
    #[error("unknown tool '{0}'")]
    UnknownTool(String),
    #[error("tool '{tool}' failed: {detail}")]
    Tool { tool: String, detail: String },
}

pub trait ModelClient: Send + Sync {
    fn complete(&self, request: CompletionRequest<'_>) -> Result<String, AdapterError>;
    fn complete_with_tools(
        &self,
        request: CompletionRequest<'_>,
        tools: &[Value],
        call_tool: &ToolExecutor<'_>,
        max_round_trips: usize,
    ) -> Result<String, AdapterError>;
}

pub trait JsonTransport: Send + Sync {
    fn post(
        &self,
        provider: &'static str,
        url: &str,
        headers: &BTreeMap<String, String>,
        payload: &Value,
    ) -> Result<Value, AdapterError>;
}

#[derive(Clone)]
pub(crate) struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    pub(crate) fn new(timeout: Duration) -> Result<Self, AdapterError> {
        Client::builder()
            .timeout(timeout)
            .user_agent("AGL/0.2 (+https://nanamanu.com/agl)")
            .build()
            .map(|client| Self { client })
            .map_err(|e| AdapterError::Network("HTTP", e.to_string()))
    }
}

impl JsonTransport for ReqwestTransport {
    fn post(
        &self,
        provider: &'static str,
        url: &str,
        headers: &BTreeMap<String, String>,
        payload: &Value,
    ) -> Result<Value, AdapterError> {
        let mut request = self.client.post(url).json(payload);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request
            .send()
            .map_err(|e| AdapterError::Network(provider, e.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|e| AdapterError::Network(provider, e.to_string()))?;
        if !status.is_success() {
            return Err(AdapterError::Http {
                provider,
                status: status.as_u16(),
                detail: body,
            });
        }
        serde_json::from_str(&body).map_err(|e| AdapterError::InvalidJson(provider, e.to_string()))
    }
}

pub(crate) fn default_transport(timeout: Duration) -> Result<Arc<dyn JsonTransport>, AdapterError> {
    Ok(Arc::new(ReqwestTransport::new(timeout)?))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    pub(crate) struct FakeTransport {
        responses: Mutex<VecDeque<Value>>,
        pub(crate) requests: Mutex<Vec<Value>>,
    }

    impl FakeTransport {
        pub(crate) fn new(responses: Vec<Value>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses.into()),
                requests: Mutex::new(Vec::new()),
            })
        }
    }

    impl JsonTransport for FakeTransport {
        fn post(
            &self,
            _provider: &'static str,
            _url: &str,
            _headers: &BTreeMap<String, String>,
            payload: &Value,
        ) -> Result<Value, AdapterError> {
            self.requests.lock().unwrap().push(payload.clone());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| AdapterError::Network("fake", "no queued response".into()))
        }
    }
}
