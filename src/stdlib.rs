use crate::adapters::tools::{ToolRegistry, default_tool_registry, tool_definitions, type_schema};
use crate::adapters::{AnthropicClient, CompletionRequest, ModelClient, OpenAiClient};
use crate::ast::{Program, TaskDef, TypeExpr};
use crate::runtime::Registry;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterMode {
    Mock,
    OpenAi,
    Anthropic,
}

impl std::str::FromStr for AdapterMode {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mock" => Ok(Self::Mock),
            "live" | "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            _ => Err(format!("unknown adapter mode '{value}'")),
        }
    }
}

pub fn registry_for(
    program: &Program,
    mode: AdapterMode,
    trace_live: bool,
) -> Result<Registry, String> {
    let tools = default_tool_registry(Duration::from_secs(15)).map_err(|e| e.to_string())?;
    registry_for_with_tools(program, mode, trace_live, tools)
}

pub fn registry_for_with_tools(
    program: &Program,
    mode: AdapterMode,
    trace_live: bool,
    tools: ToolRegistry,
) -> Result<Registry, String> {
    let mut registry = default_registry(program);
    if mode == AdapterMode::Mock {
        return Ok(registry);
    }
    let client: Arc<dyn ModelClient> = match mode {
        AdapterMode::OpenAi => {
            let key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| "OPENAI_API_KEY is required for --adapter openai".to_string())?;
            Arc::new(OpenAiClient::new(key).map_err(|e| e.to_string())?)
        }
        AdapterMode::Anthropic => {
            let key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| "ANTHROPIC_API_KEY is required for --adapter anthropic".to_string())?;
            Arc::new(AnthropicClient::new(key).map_err(|e| e.to_string())?)
        }
        AdapterMode::Mock => unreachable!(),
    };
    let tools = Arc::new(tools);
    let live_names = ["research", "draft", "compare", "respond", "llm_complete"];
    for task in program
        .tasks
        .values()
        .filter(|task| task.agent_task || live_names.contains(&task.name.as_str()))
    {
        register_live_task(
            &mut registry,
            program,
            task,
            mode,
            client.clone(),
            tools.clone(),
            trace_live,
        );
    }
    Ok(registry)
}

pub fn default_registry(program: &Program) -> Registry {
    let mut r = Registry::default();
    r.register("research",|a,agent|Ok(json!({"notes":format!("[{}] key points for '{}'",agent.unwrap_or("default-agent"),s(a,"topic"))})));
    r.register("draft",|a,agent|Ok(json!({"article":format!("[{}] Draft article:\n{}",agent.unwrap_or("default-agent"),s(a,"notes"))})));
    r.register("compare",|a,agent|Ok(json!({"decision":format!("[{}] Option A vs B\nA: {}\nB: {}",agent.unwrap_or("default-agent"),s(a,"note_a"),s(a,"note_b"))})));
    r.register("extract_intent", |a, _| {
        let m = s(a, "message").to_lowercase();
        let intent = if m.contains("refund") {
            "billing"
        } else if m.contains("bug") || m.contains("error") {
            "technical"
        } else {
            "general"
        };
        let urgency = if ["urgent", "asap", "down"].iter().any(|x| m.contains(x)) {
            "high"
        } else {
            "normal"
        };
        Ok(json!({"intent":intent,"urgency":urgency}))
    });
    r.register("route", |a, _| {
        let (i, u) = (s(a, "intent"), s(a, "urgency"));
        Ok(json!({"queue":format!("{}-{}",i,if u=="high"{"priority"}else{"standard"})}))
    });
    r.register("respond",|a,agent|Ok(json!({"reply":format!("[{}] Routed as {} to {}.",agent.unwrap_or("default-agent"),s(a,"intent"),s(a,"queue"))})));
    let attempts: Arc<Mutex<BTreeMap<String, u64>>> = Default::default();
    let shared = attempts.clone();
    r.register("flaky_fetch",move|a,agent|{let key=s(a,"key").to_string();let failures=a.get("failures_before_success").and_then(Value::as_u64).unwrap_or(0);let mut m=shared.lock().unwrap();let n=m.entry(key.clone()).or_default();if *n<failures{*n+=1;Err(format!("transient failure for key '{key}' ({n}/{failures})"))}else{Ok(json!({"data":format!("[{}] fetched payload for {key}",agent.unwrap_or("default-agent"))}))}});
    r.register("llm_complete", |a, agent| {
        Ok(json!({"text":format!("[{}] {}",agent.unwrap_or("default-agent"),s(a,"prompt"))}))
    });
    r.register("countdown", |a, _| {
        let n = (a.get("current").and_then(Value::as_f64).unwrap_or(0.0) - 1.0).max(0.0);
        Ok(json!({"next":number(n),"done":n<=0.0}))
    });
    r.register("merge_drafts", |a, _| {
        let (a1, b) = (s(a, "draft_a"), s(a, "draft_b"));
        let sections: Vec<_> = [a1, b].into_iter().filter(|x| !x.is_empty()).collect();
        let total = a
            .get("word_count_a")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            + a.get("word_count_b")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
        Ok(json!({"article":if b.is_empty(){a1.into()}else{format!("{a1}\n\n{b}")},"sections":sections,"total_words":number(total)}))
    });
    r.register("fallback_enrich", |a, _| {
        Ok(json!({"extra":format!("[fallback enrichment for '{}']",s(a,"query"))}))
    });
    for task in program.tasks.values().filter(|t| t.agent_task) {
        let ty = task.return_type.clone();
        let label = task.name.clone();
        let enums = program.enums.clone();
        r.register(task.name.clone(), move |args, agent| {
            Ok(mock(
                &ty,
                &format!("{} via {}", label, agent.unwrap_or("default-agent")),
                args,
                &enums,
            ))
        })
    }
    r
}

fn register_live_task(
    registry: &mut Registry,
    program: &Program,
    task: &TaskDef,
    mode: AdapterMode,
    client: Arc<dyn ModelClient>,
    tools: Arc<ToolRegistry>,
    trace_live: bool,
) {
    let (program, task) = (program.clone(), task.clone());
    let task_name = task.name.clone();
    registry.register(task_name.clone(), move |args, agent_name| {
        let agent = agent_name.and_then(|name| program.agents.get(name));
        let model = configured_model(mode).unwrap_or_else(|| {
            agent
                .and_then(|agent| agent.model.as_deref())
                .map(|model| map_model(mode, model))
                .unwrap_or_else(|| default_model(mode).into())
        });
        let schema = type_schema(&program, &task.return_type);
        let prompt = format!(
            "Task name: {}\nTask inputs:\n{}\n\nReturn schema:\n{}\n\nReturn only minified valid JSON on one line, without markdown fences or commentary.",
            task.name,
            serde_json::to_string_pretty(args).unwrap(),
            serde_json::to_string_pretty(&schema).unwrap()
        );
        if trace_live {
            eprintln!(
                "[trace] task={} agent={} model={} start",
                task.name,
                agent_name.unwrap_or("default-agent"),
                model
            );
        }
        let request = CompletionRequest {
            model: &model,
            prompt: &prompt,
            system: Some(
                "You execute typed AGL tasks. Use tools when helpful and satisfy the declared JSON schema exactly.",
            ),
            max_output_tokens: Some(1800),
            reasoning_effort: (mode == AdapterMode::OpenAi).then_some("medium"),
        };
        let definitions = agent
            .map(|agent| tool_definitions(&program, &agent.tools))
            .unwrap_or_default();
        let raw = if definitions.is_empty() {
            client.complete(request)
        } else {
            client.complete_with_tools(
                request,
                &definitions,
                &|name, call_args| {
                    tools
                        .execute(&program, name, call_args)
                        .map_err(|e| crate::adapters::AdapterError::Tool {
                            tool: name.into(),
                            detail: e.to_string(),
                        })
                },
                8,
            )
        }
        .map_err(|e| e.to_string())?;
        parse_model_json(&task_name, &raw)
    });
}

fn parse_model_json(task: &str, raw: &str) -> Result<Value, String> {
    let mut text = raw.trim();
    if text.starts_with("```") {
        text = text
            .split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or(text.trim_start_matches('`'));
    }
    text = text.strip_suffix("```").unwrap_or(text).trim();
    serde_json::from_str(text)
        .or_else(|_| {
            let (start, end) = (text.find('{'), text.rfind('}'));
            match (start, end) {
                (Some(start), Some(end)) if end > start => serde_json::from_str(&text[start..=end]),
                _ => Err(serde_json::Error::io(std::io::Error::other(
                    "no JSON object in response",
                ))),
            }
        })
        .map_err(|_| format!("agent task '{task}' returned non-JSON output: {text:?}"))
}

fn default_model(mode: AdapterMode) -> &'static str {
    match mode {
        AdapterMode::OpenAi => "gpt-5.6-sol",
        AdapterMode::Anthropic => "claude-haiku-4-5-20251001",
        AdapterMode::Mock => "mock",
    }
}

fn map_model(mode: AdapterMode, model: &str) -> String {
    if mode == AdapterMode::OpenAi {
        return match model {
            "gpt-4.1" | "gpt-4o" => "gpt-5.6-sol".into(),
            "gpt-4.1-mini" | "gpt-4o-mini" => "gpt-5.6-luna".into(),
            _ => model.into(),
        };
    }
    if mode == AdapterMode::Mock {
        return model.into();
    }
    match model {
        "gpt-4.1" | "gpt-4o" => "claude-sonnet-4-20250514".into(),
        "gpt-4.1-mini" | "gpt-4o-mini" => "claude-haiku-4-5-20251001".into(),
        _ => model.into(),
    }
}

fn configured_model(mode: AdapterMode) -> Option<String> {
    let name = match mode {
        AdapterMode::OpenAi => "AGL_OPENAI_MODEL",
        AdapterMode::Anthropic => "AGL_ANTHROPIC_MODEL",
        AdapterMode::Mock => return None,
    };
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn s<'a>(a: &'a BTreeMap<String, Value>, k: &str) -> &'a str {
    a.get(k).and_then(Value::as_str).unwrap_or("")
}

fn number(value: f64) -> Value {
    if value.fract() == 0.0 {
        Value::from(value as i64)
    } else {
        Value::from(value)
    }
}
fn mock(
    t: &TypeExpr,
    label: &str,
    args: &BTreeMap<String, Value>,
    enums: &BTreeMap<String, Vec<String>>,
) -> Value {
    match t {
        TypeExpr::String => Value::String(
            if let Some(x) = args
                .get("topic")
                .or_else(|| args.get("query"))
                .and_then(Value::as_str)
            {
                format!("[{label}] {x}")
            } else {
                format!("[{label}]")
            },
        ),
        TypeExpr::Number => json!(0),
        TypeExpr::Bool => Value::Bool(false),
        TypeExpr::List(_) => json!([]),
        TypeExpr::Option(_) => Value::Null,
        TypeExpr::Enum(n) => enums
            .get(n)
            .and_then(|x| x.first())
            .cloned()
            .map(Value::String)
            .unwrap_or(Value::Null),
        TypeExpr::Alias(_) => Value::Null,
        TypeExpr::Obj(fs) => {
            let review = matches!(
                (fs.get("approved"), fs.get("feedback")),
                (Some(TypeExpr::Bool), Some(TypeExpr::String))
            );
            Value::Object(
                fs.iter()
                    .map(|(k, t)| {
                        (
                            k.clone(),
                            if review && k == "approved" {
                                Value::Bool(true)
                            } else if review && k == "feedback" {
                                Value::String("mock approved".into())
                            } else {
                                mock(t, &format!("{label}.{k}"), args, enums)
                            },
                        )
                    })
                    .collect(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_legacy_openai_models_by_tier() {
        assert_eq!(map_model(AdapterMode::OpenAi, "gpt-4.1"), "gpt-5.6-sol");
        assert_eq!(
            map_model(AdapterMode::OpenAi, "gpt-4.1-mini"),
            "gpt-5.6-luna"
        );
        assert_eq!(
            map_model(AdapterMode::OpenAi, "custom-model"),
            "custom-model"
        );
    }
}
