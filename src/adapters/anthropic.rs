use super::{
    AdapterError, CompletionRequest, JsonTransport, ModelClient, ToolExecutor, default_transport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

const PROVIDER: &str = "Anthropic";
pub struct AnthropicClient {
    api_key: String,
    base_url: String,
    transport: Arc<dyn JsonTransport>,
}
impl AnthropicClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self, AdapterError> {
        Self::with_base_url(api_key, "https://api.anthropic.com")
    }
    pub fn with_base_url(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, AdapterError> {
        Ok(Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            transport: default_transport(Duration::from_secs(45))?,
        })
    }
    #[cfg(test)]
    pub(crate) fn with_transport(transport: Arc<dyn JsonTransport>) -> Self {
        Self {
            api_key: "test".into(),
            base_url: "https://example.test".into(),
            transport,
        }
    }
    fn message(&self, payload: Value) -> Result<Value, AdapterError> {
        self.transport.post(
            PROVIDER,
            &format!("{}/v1/messages", self.base_url.trim_end_matches('/')),
            &BTreeMap::from([
                ("x-api-key".into(), self.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("Content-Type".into(), "application/json".into()),
            ]),
            &payload,
        )
    }
}
impl ModelClient for AnthropicClient {
    fn complete(&self, r: CompletionRequest<'_>) -> Result<String, AdapterError> {
        let mut p = json!({"model":r.model,"messages":[{"role":"user","content":r.prompt}],"max_tokens":r.max_output_tokens.unwrap_or(1024)});
        if let Some(s) = r.system {
            p["system"] = s.into()
        }
        extract_text(&self.message(p)?).ok_or(AdapterError::MissingText(PROVIDER))
    }
    fn complete_with_tools(
        &self,
        r: CompletionRequest<'_>,
        tools: &[Value],
        call_tool: &ToolExecutor<'_>,
        max_round_trips: usize,
    ) -> Result<String, AdapterError> {
        let tools: Vec<_> = tools.iter().map(convert_tool).collect();
        let mut messages = vec![json!({"role":"user","content":r.prompt})];
        let mut response = self.message(message_payload(&r, &messages, &tools))?;
        for _ in 0..max_round_trips {
            let uses = tool_uses(&response);
            if uses.is_empty() {
                return extract_text(&response).ok_or(AdapterError::MissingText(PROVIDER));
            }
            messages.push(json!({"role":"assistant","content":response["content"].clone()}));
            let mut results = Vec::new();
            for u in uses {
                let object = u
                    .input
                    .as_object()
                    .ok_or_else(|| AdapterError::InvalidToolCall {
                        provider: PROVIDER,
                        tool: u.name.clone(),
                        problem: "non-object input".into(),
                    })?;
                let result = call_tool(&u.name, object)?;
                results.push(json!({"type":"tool_result","tool_use_id":u.id,"content":serde_json::to_string(&result).unwrap()}))
            }
            messages.push(json!({"role":"user","content":results}));
            response = self.message(message_payload(&r, &messages, &tools))?;
        }
        Err(AdapterError::ToolLoopLimit(PROVIDER))
    }
}
fn message_payload(r: &CompletionRequest<'_>, messages: &[Value], tools: &[Value]) -> Value {
    let mut p = json!({"model":r.model,"messages":messages,"tools":tools,"max_tokens":r.max_output_tokens.unwrap_or(1024)});
    if let Some(s) = r.system {
        p["system"] = s.into()
    }
    p
}
fn convert_tool(t: &Value) -> Value {
    json!({"name":t.get("name").cloned().unwrap_or(Value::Null),"description":t.get("description").cloned().unwrap_or_else(||Value::String("Execute the declared tool".into())),"input_schema":t.get("parameters").cloned().unwrap_or_else(||json!({"type":"object","properties":{}}))})
}
fn extract_text(v: &Value) -> Option<String> {
    let xs: Vec<_> = v
        .get("content")?
        .as_array()?
        .iter()
        .filter(|x| x.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|x| x.get("text")?.as_str())
        .filter(|x| !x.trim().is_empty())
        .map(str::trim)
        .collect();
    (!xs.is_empty()).then(|| xs.join("\n"))
}
struct ToolUse {
    name: String,
    id: String,
    input: Value,
}
fn tool_uses(v: &Value) -> Vec<ToolUse> {
    v.get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|x| x.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|x| {
            Some(ToolUse {
                name: x.get("name")?.as_str()?.into(),
                id: x.get("id")?.as_str()?.into(),
                input: x.get("input").cloned().unwrap_or_else(|| json!({})),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::tests::FakeTransport;
    #[test]
    fn executes_tool_loop() {
        let fake = FakeTransport::new(vec![
            json!({"content":[{"type":"tool_use","id":"u1","name":"lookup","input":{"q":"rust"}}]}),
            json!({"content":[{"type":"text","text":"done"}]}),
        ]);
        let client = AnthropicClient::with_transport(fake.clone());
        let text = client
            .complete_with_tools(
                CompletionRequest {
                    model: "claude-test",
                    prompt: "go",
                    system: Some("help"),
                    max_output_tokens: None,
                },
                &[json!({"name":"lookup","parameters":{"type":"object"}})],
                &|_, _| Ok(json!({"ok":true})),
                3,
            )
            .unwrap();
        assert_eq!(text, "done");
        assert_eq!(fake.requests.lock().unwrap().len(), 2)
    }
}
