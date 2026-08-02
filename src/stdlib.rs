use crate::ast::{Program, TypeExpr};
use crate::runtime::Registry;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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
