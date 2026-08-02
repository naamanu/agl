use crate::ast::*;
use crate::context::ExecutionContext;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

pub type Handler =
    Arc<dyn Fn(&BTreeMap<String, Value>, Option<&str>) -> Result<Value, String> + Send + Sync>;
#[derive(Clone, Default)]
pub struct Registry {
    handlers: BTreeMap<String, Handler>,
}
impl Registry {
    pub fn register<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&BTreeMap<String, Value>, Option<&str>) -> Result<Value, String>
            + Send
            + Sync
            + 'static,
    {
        self.handlers.insert(name.into(), Arc::new(f));
    }
    pub fn contains(&self, n: &str) -> bool {
        self.handlers.contains_key(n)
    }

    pub fn extend(&mut self, other: Registry) {
        self.handlers.extend(other.handlers);
    }
}

#[derive(Debug, Error)]
#[error("[AGL3001] {message}")]
pub struct ExecutionError {
    pub kind: &'static str,
    pub message: String,
    pub operation: Option<String>,
    pub retryable: bool,
}
impl ExecutionError {
    pub const fn code(&self) -> &'static str {
        "AGL3001"
    }

    pub fn as_value(&self) -> Value {
        json!({
            "kind": self.kind,
            "message": self.message,
            "operation": self.operation,
            "retryable": self.retryable,
        })
    }
}
enum Flow {
    Next,
    Return(Value),
    Break,
    Continue,
}

pub fn execute_pipeline(
    program: &Program,
    name: &str,
    inputs: BTreeMap<String, Value>,
    registry: &Registry,
    context: &ExecutionContext,
) -> Result<Value, ExecutionError> {
    execute(program, name, inputs, registry, context, 0)
}
fn execute(
    program: &Program,
    name: &str,
    inputs: BTreeMap<String, Value>,
    registry: &Registry,
    context: &ExecutionContext,
    depth: usize,
) -> Result<Value, ExecutionError> {
    if depth > 128 {
        return Err(fail("pipeline recursion limit exceeded"));
    }
    let p = program
        .pipelines
        .get(name)
        .ok_or_else(|| fail(format!("unknown pipeline '{name}'")))?;
    validate_args(&p.params, &inputs, program)?;
    let mut env = inputs;
    context.record("pipeline_start", json!({"pipeline":name}));
    match block(&p.statements, program, &mut env, registry, context, depth)? {
        Flow::Return(v) => {
            if !value_matches(&v, &p.return_type, program) {
                return Err(fail(format!(
                    "pipeline '{name}' returned a value incompatible with {:?}",
                    p.return_type
                )));
            }
            context.record("pipeline_end", json!({"pipeline":name,"result":v}));
            Ok(v)
        }
        _ => Err(fail(format!(
            "pipeline '{name}' completed without returning"
        ))),
    }
}
pub fn run_tests(
    program: &Program,
    registry: &Registry,
    context: &ExecutionContext,
) -> Vec<(String, Result<(), ExecutionError>)> {
    program
        .tests
        .iter()
        .map(|t| {
            let mut env = BTreeMap::new();
            let r = block(&t.statements, program, &mut env, registry, context, 0).map(|_| ());
            (t.name.clone(), r)
        })
        .collect()
}

fn block(
    xs: &[Stmt],
    p: &Program,
    env: &mut BTreeMap<String, Value>,
    reg: &Registry,
    ctx: &ExecutionContext,
    depth: usize,
) -> Result<Flow, ExecutionError> {
    for s in xs {
        match s {
            Stmt::Run(r) => {
                let v = run(r, p, env, reg, ctx, depth)?;
                env.insert(r.target.clone(), v);
            }
            Stmt::Parallel {
                branches,
                max_concurrency,
                ..
            } => {
                let snapshot = env.clone();
                let width = max_concurrency.unwrap_or(branches.len().max(1));
                for chunk in branches.chunks(width) {
                    let mut handles = Vec::new();
                    for r in chunk.iter().cloned() {
                        let (p, e, g, c) = (p.clone(), snapshot.clone(), reg.clone(), ctx.clone());
                        handles.push(std::thread::spawn(move || {
                            run(&r, &p, &e, &g, &c, depth).map(|v| (r.target, v))
                        }))
                    }
                    for h in handles {
                        let (k, v) = h.join().map_err(|_| fail("parallel branch panicked"))??;
                        env.insert(k, v);
                    }
                }
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let body = if truthy(&eval(condition, env)?) {
                    then_body
                } else {
                    else_body
                };
                match block(body, p, env, reg, ctx, depth)? {
                    Flow::Next => {}
                    x => return Ok(x),
                }
            }
            Stmt::IfLet {
                binding,
                option,
                then_body,
                else_body,
                ..
            } => {
                let v = eval(option, env)?;
                if v.is_null() {
                    match block(else_body, p, env, reg, ctx, depth)? {
                        Flow::Next => {}
                        x => return Ok(x),
                    }
                } else {
                    let old = env.insert(binding.clone(), v);
                    let f = block(then_body, p, env, reg, ctx, depth)?;
                    if let Some(x) = old {
                        env.insert(binding.clone(), x);
                    } else {
                        env.remove(binding);
                    }
                    if !matches!(f, Flow::Next) {
                        return Ok(f);
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                while truthy(&eval(condition, env)?) {
                    match block(body, p, env, reg, ctx, depth)? {
                        Flow::Next | Flow::Continue => {}
                        Flow::Break => break,
                        x => return Ok(x),
                    }
                }
            }
            Stmt::Break(_) => return Ok(Flow::Break),
            Stmt::Continue(_) => return Ok(Flow::Continue),
            Stmt::TryCatch {
                try_body,
                error_var,
                structured,
                catch_body,
                ..
            } => {
                if let Err(e) = block(try_body, p, env, reg, ctx, depth) {
                    env.insert(
                        error_var.clone(),
                        if *structured {
                            e.as_value()
                        } else {
                            Value::String(e.to_string())
                        },
                    );
                    let f = block(catch_body, p, env, reg, ctx, depth)?;
                    if !matches!(f, Flow::Next) {
                        return Ok(f);
                    }
                }
            }
            Stmt::Match { value, arms, .. } => {
                let matched = eval(value, env)?;
                let object = matched
                    .as_object()
                    .ok_or_else(|| fail("match value is not a tagged object"))?;
                let type_name = object
                    .get("$type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| fail("match value has no $type tag"))?;
                let variant = object
                    .get("$variant")
                    .and_then(Value::as_str)
                    .ok_or_else(|| fail("match value has no $variant tag"))?;
                let arm = arms
                    .iter()
                    .find(|arm| arm.type_name == type_name && arm.variant == variant)
                    .ok_or_else(|| fail(format!("no match arm for '{type_name}::{variant}'")))?;
                let mut previous = Vec::new();
                for binding in &arm.bindings {
                    let value = object.get(binding).cloned().ok_or_else(|| {
                        fail(format!(
                            "variant '{type_name}::{variant}' has no '{binding}'"
                        ))
                    })?;
                    previous.push((binding.clone(), env.insert(binding.clone(), value)));
                }
                let flow = block(&arm.body, p, env, reg, ctx, depth)?;
                for (binding, old) in previous {
                    if let Some(value) = old {
                        env.insert(binding, value);
                    } else {
                        env.remove(&binding);
                    }
                }
                if !matches!(flow, Flow::Next) {
                    return Ok(flow);
                }
            }
            Stmt::Assert {
                condition, message, ..
            } => {
                if !truthy(&eval(condition, env)?) {
                    return Err(failure(
                        "assertion",
                        message.clone().unwrap_or_else(|| "assertion failed".into()),
                        None,
                        false,
                    ));
                }
            }
            Stmt::Return { expr, .. } => return Ok(Flow::Return(eval(expr, env)?)),
        }
    }
    Ok(Flow::Next)
}
fn run(
    r: &RunStmt,
    p: &Program,
    env: &BTreeMap<String, Value>,
    reg: &Registry,
    ctx: &ExecutionContext,
    depth: usize,
) -> Result<Value, ExecutionError> {
    let args = r
        .args
        .iter()
        .map(|(k, e)| Ok((k.clone(), eval(e, env)?)))
        .collect::<Result<BTreeMap<_, _>, ExecutionError>>()?;
    if let Some(x) = p.pipelines.get(&r.callable) {
        validate_args(&x.params, &args, p)?;
        return execute(p, &r.callable, args, reg, ctx, depth + 1);
    }
    let task = p
        .tasks
        .get(&r.callable)
        .ok_or_else(|| fail(format!("unknown task '{}'", r.callable)))?;
    validate_args(&task.params, &args, p)?;
    let h = reg
        .handlers
        .get(&r.callable)
        .ok_or_else(|| fail(format!("no handler registered for task '{}'", r.callable)))?;
    let attempts = r.retries + 1;
    let mut last = None;
    let mut attempts_made = 0;
    for attempt in 1..=attempts {
        attempts_made = attempt;
        ctx.record(
            "task_attempt",
            json!({"task":r.callable,"attempt":attempt,"agent":r.agent}),
        );
        let result: Result<Value, ExecutionError> = if let Some(seconds) = r.timeout {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            let (handler, owned_args, agent) = (h.clone(), args.clone(), r.agent.clone());
            std::thread::spawn(move || {
                let _ = sender.send(handler(&owned_args, agent.as_deref()));
            });
            match receiver.recv_timeout(std::time::Duration::from_secs_f64(seconds)) {
                Ok(result) => result
                    .map_err(|message| failure("task", message, Some(r.callable.clone()), true)),
                Err(_) => Err(failure(
                    "timeout",
                    format!("timed out after {seconds}s"),
                    Some(r.callable.clone()),
                    false,
                )),
            }
        } else {
            h(&args, r.agent.as_deref())
                .map_err(|message| failure("task", message, Some(r.callable.clone()), true))
        };
        match result {
            Ok(v) => {
                if !value_matches(&v, &task.return_type, p) {
                    return Err(fail(format!(
                        "task '{}' returned incompatible value",
                        r.callable
                    )));
                }
                if attempt < attempts && retryable_typed_error(&v, &r.retry_on) {
                    ctx.record(
                        "task_retry",
                        json!({"task":r.callable,"attempt":attempt,"reason":"typed_error"}),
                    );
                    continue;
                }
                return Ok(v);
            }
            Err(error) => {
                let retryable = error.retryable;
                last = Some(error);
                if !retryable {
                    break;
                }
            }
        }
    }
    match &r.on_fail {
        OnFail::Use(e) => eval(e, env),
        OnFail::Abort => {
            let last = last.unwrap_or_else(|| {
                failure(
                    "task",
                    "task failed without an error",
                    Some(r.callable.clone()),
                    false,
                )
            });
            Err(failure(
                last.kind,
                format!(
                    "task '{}' failed after {attempts_made} attempt(s): {}",
                    r.callable, last.message
                ),
                Some(r.callable.clone()),
                last.retryable,
            ))
        }
    }
}
fn retryable_typed_error(value: &Value, selectors: &[(String, String)]) -> bool {
    if selectors.is_empty()
        || value.get("$type").and_then(Value::as_str) != Some("Result")
        || value.get("$variant").and_then(Value::as_str) != Some("Err")
    {
        return false;
    }
    let Some(error) = value.get("error") else {
        return false;
    };
    let (Some(type_name), Some(variant)) = (
        error.get("$type").and_then(Value::as_str),
        error.get("$variant").and_then(Value::as_str),
    ) else {
        return false;
    };
    selectors
        .iter()
        .any(|selector| selector.0 == type_name && selector.1 == variant)
}
fn eval(e: &Expr, env: &BTreeMap<String, Value>) -> Result<Value, ExecutionError> {
    match e {
        Expr::Literal { value, .. } => Ok(value.clone()),
        Expr::Ref { parts, .. } => {
            let mut v = env
                .get(&parts[0])
                .ok_or_else(|| fail(format!("unknown reference '{}'", parts[0])))?;
            for f in &parts[1..] {
                v = v
                    .get(f)
                    .ok_or_else(|| fail(format!("missing field '{f}'")))?
            }
            Ok(v.clone())
        }
        Expr::Obj { fields, .. } | Expr::Record { fields, .. } => Ok(Value::Object(
            fields
                .iter()
                .map(|(k, v)| Ok((k.clone(), eval(v, env)?)))
                .collect::<Result<Map<_, _>, ExecutionError>>()?,
        )),
        Expr::Variant {
            type_name,
            variant,
            fields,
            ..
        } => {
            let mut object = Map::from_iter([
                ("$type".into(), Value::String(type_name.clone())),
                ("$variant".into(), Value::String(variant.clone())),
            ]);
            for (field, expression) in fields {
                object.insert(field.clone(), eval(expression, env)?);
            }
            Ok(Value::Object(object))
        }
        Expr::Result { ok, value, .. } => Ok(json!({
            "$type": "Result",
            "$variant": if *ok { "Ok" } else { "Err" },
            if *ok { "value" } else { "error" }: eval(value, env)?,
        })),
        Expr::List { items, .. } => Ok(Value::Array(
            items
                .iter()
                .map(|x| eval(x, env))
                .collect::<Result<_, _>>()?,
        )),
        Expr::Binary {
            op, left, right, ..
        } => {
            let (a, b) = (eval(left, env)?, eval(right, env)?);
            match op {
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                BinaryOp::Add => match (a, b) {
                    (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                    (Value::Number(a), Value::Number(b)) => {
                        let sum = a.as_f64().unwrap() + b.as_f64().unwrap();
                        Ok(if sum.fract() == 0.0 {
                            Value::from(sum as i64)
                        } else {
                            Value::from(sum)
                        })
                    }
                    _ => Err(fail("'+' requires two strings or two numbers")),
                },
            }
        }
    }
}
fn validate_args(
    ps: &[Param],
    args: &BTreeMap<String, Value>,
    p: &Program,
) -> Result<(), ExecutionError> {
    for x in ps {
        let v = args
            .get(&x.name)
            .ok_or_else(|| fail(format!("missing argument '{}'", x.name)))?;
        if !value_matches(v, &x.ty, p) {
            return Err(fail(format!(
                "argument '{}' is incompatible with {:?}",
                x.name, x.ty
            )));
        }
    }
    if args.len() != ps.len() {
        return Err(fail("unexpected arguments"));
    }
    Ok(())
}
fn value_matches(v: &Value, t: &TypeExpr, p: &Program) -> bool {
    match t {
        TypeExpr::String => v.is_string(),
        TypeExpr::Number => v.is_number(),
        TypeExpr::Bool => v.is_boolean(),
        TypeExpr::Failure => v.as_object().is_some_and(|object| {
            object.get("kind").is_some_and(Value::is_string)
                && object.get("message").is_some_and(Value::is_string)
                && object
                    .get("operation")
                    .is_some_and(|value| value.is_null() || value.is_string())
                && object.get("retryable").is_some_and(Value::is_boolean)
        }),
        TypeExpr::List(x) => v
            .as_array()
            .is_some_and(|a| a.iter().all(|v| value_matches(v, x, p))),
        TypeExpr::Option(x) => v.is_null() || value_matches(v, x, p),
        TypeExpr::Result(ok, error) => v.as_object().is_some_and(|object| {
            object.get("$type").and_then(Value::as_str) == Some("Result")
                && match object.get("$variant").and_then(Value::as_str) {
                    Some("Ok") => object
                        .get("value")
                        .is_some_and(|value| value_matches(value, ok, p)),
                    Some("Err") => object
                        .get("error")
                        .is_some_and(|value| value_matches(value, error, p)),
                    _ => false,
                }
        }),
        TypeExpr::Obj(fs) => v.as_object().is_some_and(|o| {
            fs.iter()
                .all(|(k, t)| o.get(k).is_some_and(|v| value_matches(v, t, p)))
        }),
        TypeExpr::Record(name) => p.records.get(name).is_some_and(|record| {
            v.as_object().is_some_and(|object| {
                record.fields.iter().all(|(field, ty)| {
                    object
                        .get(field)
                        .is_some_and(|value| value_matches(value, ty, p))
                })
            })
        }),
        TypeExpr::Union(name) => p.unions.get(name).is_some_and(|union| {
            v.as_object().is_some_and(|object| {
                object.get("$type").and_then(Value::as_str) == Some(name)
                    && object
                        .get("$variant")
                        .and_then(Value::as_str)
                        .and_then(|variant| union.variants.get(variant))
                        .is_some_and(|fields| {
                            fields.iter().all(|(field, ty)| {
                                object
                                    .get(field)
                                    .is_some_and(|value| value_matches(value, ty, p))
                            })
                        })
            })
        }),
        TypeExpr::Enum(n) => v
            .as_str()
            .is_some_and(|x| p.enums.get(n).is_some_and(|vs| vs.iter().any(|v| v == x))),
        TypeExpr::Alias(n) => p.aliases.get(n).is_some_and(|t| value_matches(v, t, p)),
    }
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(x) => *x,
        Value::Number(x) => x.as_f64() != Some(0.0),
        Value::String(x) => !x.is_empty(),
        Value::Array(x) => !x.is_empty(),
        Value::Object(x) => !x.is_empty(),
    }
}
fn fail(message: impl Into<String>) -> ExecutionError {
    failure("runtime", message, None, false)
}
fn failure(
    kind: &'static str,
    message: impl Into<String>,
    operation: Option<String>,
    retryable: bool,
) -> ExecutionError {
    ExecutionError {
        kind,
        message: message.into(),
        operation,
        retryable,
    }
}
