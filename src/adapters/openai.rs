use super::{
    AdapterError, CompletionRequest, JsonTransport, ModelClient, ToolExecutor, default_transport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

const PROVIDER: &str = "OpenAI";

pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    transport: Arc<dyn JsonTransport>,
}

impl OpenAiClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self, AdapterError> {
        Self::with_base_url(api_key, "https://api.openai.com/v1")
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
            base_url: "https://example.test/v1".into(),
            transport,
        }
    }

    fn response(&self, payload: Value) -> Result<Value, AdapterError> {
        self.transport.post(
            PROVIDER,
            &format!("{}/responses", self.base_url.trim_end_matches('/')),
            &BTreeMap::from([
                ("Authorization".into(), format!("Bearer {}", self.api_key)),
                ("Content-Type".into(), "application/json".into()),
            ]),
            &payload,
        )
    }
}

impl ModelClient for OpenAiClient {
    fn complete(&self, request: CompletionRequest<'_>) -> Result<String, AdapterError> {
        let mut payload =
            json!({"model":request.model,"input":input(request.prompt,request.system)});
        if let Some(max) = request.max_output_tokens {
            payload["max_output_tokens"] = max.into();
        }
        extract_text(&self.response(payload)?).ok_or(AdapterError::MissingText(PROVIDER))
    }

    fn complete_with_tools(
        &self,
        request: CompletionRequest<'_>,
        tools: &[Value],
        call_tool: &ToolExecutor,
        max_round_trips: usize,
    ) -> Result<String, AdapterError> {
        let mut payload = json!({"model":request.model,"input":input(request.prompt,request.system),"tools":tools,"parallel_tool_calls":false});
        if let Some(max) = request.max_output_tokens {
            payload["max_output_tokens"] = max.into();
        }
        let mut response = self.response(payload)?;
        for _ in 0..max_round_trips {
            let calls = function_calls(&response)?;
            if calls.is_empty() {
                return extract_text(&response).ok_or(AdapterError::MissingText(PROVIDER));
            }
            let mut outputs = Vec::new();
            for call in calls {
                let args: Value = serde_json::from_str(&call.arguments).map_err(|_| {
                    AdapterError::InvalidToolCall {
                        provider: PROVIDER,
                        tool: call.name.clone(),
                        problem: "invalid JSON arguments".into(),
                    }
                })?;
                let object = args
                    .as_object()
                    .ok_or_else(|| AdapterError::InvalidToolCall {
                        provider: PROVIDER,
                        tool: call.name.clone(),
                        problem: "non-object arguments".into(),
                    })?;
                let result = call_tool(&call.name, object)?;
                outputs.push(json!({"type":"function_call_output","call_id":call.call_id,"output":serde_json::to_string(&result).unwrap()}));
            }
            let mut next = json!({"model":request.model,"input":outputs,"tools":tools,"previous_response_id":response.get("id").and_then(Value::as_str)});
            if let Some(max) = request.max_output_tokens {
                next["max_output_tokens"] = max.into();
            }
            response = self.response(next)?;
        }
        Err(AdapterError::ToolLoopLimit(PROVIDER))
    }
}

fn input(prompt: &str, system: Option<&str>) -> Vec<Value> {
    let mut items = Vec::new();
    if let Some(system) = system {
        items.push(json!({"role":"system","content":[{"type":"input_text","text":system}]}))
    }
    items.push(json!({"role":"user","content":[{"type":"input_text","text":prompt}]}));
    items
}
fn extract_text(v: &Value) -> Option<String> {
    if let Some(x) = v
        .get("output_text")
        .and_then(Value::as_str)
        .filter(|x| !x.trim().is_empty())
    {
        return Some(x.trim().into());
    }
    let parts: Vec<_> = v
        .get("output")?
        .as_array()?
        .iter()
        .filter_map(|x| x.get("content")?.as_array())
        .flatten()
        .filter_map(|x| x.get("text")?.as_str())
        .filter(|x| !x.trim().is_empty())
        .map(str::trim)
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}
struct FunctionCall {
    name: String,
    call_id: String,
    arguments: String,
}
fn function_calls(v: &Value) -> Result<Vec<FunctionCall>, AdapterError> {
    let mut out = Vec::new();
    for item in v
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            continue;
        }
        let get = |key| item.get(key).and_then(Value::as_str).map(str::to_owned);
        if let (Some(name), Some(call_id), Some(arguments)) =
            (get("name"), get("call_id"), get("arguments"))
        {
            out.push(FunctionCall {
                name,
                call_id,
                arguments,
            })
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::tests::FakeTransport;
    #[test]
    fn extracts_direct_and_nested_text() {
        assert_eq!(extract_text(&json!({"output_text":" hi "})).unwrap(), "hi");
        assert_eq!(
            extract_text(&json!({"output":[{"content":[{"text":"one"},{"text":"two"}]}]})).unwrap(),
            "one\ntwo"
        )
    }
    #[test]
    fn executes_tool_loop() {
        let fake = FakeTransport::new(vec![
            json!({"id":"r1","output":[{"type":"function_call","name":"lookup","call_id":"c1","arguments":"{\"q\":\"rust\"}"}]}),
            json!({"id":"r2","output_text":"done"}),
        ]);
        let client = OpenAiClient::with_transport(fake.clone());
        let text = client
            .complete_with_tools(
                CompletionRequest {
                    model: "gpt-test",
                    prompt: "go",
                    system: None,
                    max_output_tokens: None,
                },
                &[json!({"type":"function","name":"lookup","parameters":{"type":"object"}})],
                &|name, args| Ok(json!({"name":name,"q":args["q"]})),
                3,
            )
            .unwrap();
        assert_eq!(text, "done");
        assert_eq!(fake.requests.lock().unwrap().len(), 2)
    }
}
