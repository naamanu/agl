use crate::ast::*;
use crate::context::ExecutionContext;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use thiserror::Error;

pub type Handler =
    Arc<dyn Fn(&BTreeMap<String, Value>, Option<&str>) -> Result<Value, String> + Send + Sync>;
type ContextualHandler = Arc<
    dyn Fn(&BTreeMap<String, Value>, Option<&str>, &Invocation) -> Result<TaskOutput, String>
        + Send
        + Sync,
>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub estimated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskOutput {
    pub value: Value,
    pub usage: Usage,
}

impl TaskOutput {
    pub fn new(value: Value) -> Self {
        Self {
            value,
            usage: Usage::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invocation {
    pub execution_id: String,
    pub invocation_id: String,
    pub idempotency_key: String,
    pub attempt: u32,
}

#[derive(Clone, Default)]
pub struct Registry {
    handlers: BTreeMap<String, ContextualHandler>,
}
impl Registry {
    pub fn register<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&BTreeMap<String, Value>, Option<&str>) -> Result<Value, String>
            + Send
            + Sync
            + 'static,
    {
        self.handlers.insert(
            name.into(),
            Arc::new(move |args, agent, _| f(args, agent).map(TaskOutput::new)),
        );
    }

    pub fn register_contextual<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&BTreeMap<String, Value>, Option<&str>, &Invocation) -> Result<TaskOutput, String>
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

    /// Builds deterministic handlers from previously recorded `task_result` events.
    pub fn from_trace(events: &[crate::context::TraceEvent]) -> Self {
        let mut recorded: BTreeMap<String, VecDeque<Value>> = BTreeMap::new();
        for event in events.iter().filter(|event| event.kind == "task_result") {
            if let (Some(task), Some(value)) = (
                event.fields.get("task").and_then(Value::as_str),
                event.fields.get("result"),
            ) {
                recorded
                    .entry(task.into())
                    .or_default()
                    .push_back(value.clone());
            }
        }
        let mut registry = Self::default();
        for (task, values) in recorded {
            let values = Arc::new(Mutex::new(values));
            registry.register(task, move |_, _| {
                values
                    .lock()
                    .unwrap()
                    .pop_front()
                    .ok_or_else(|| "replay exhausted recorded results".into())
            });
        }
        registry
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

#[derive(Clone)]
struct BudgetState {
    limits: Option<ResourceBudget>,
    started: Instant,
    calls: Arc<AtomicU64>,
    retries: Arc<AtomicU64>,
    tokens: Arc<AtomicU64>,
    cost: Arc<Mutex<f64>>,
}

impl BudgetState {
    fn new(limits: Option<ResourceBudget>) -> Self {
        Self {
            limits,
            started: Instant::now(),
            calls: Arc::new(AtomicU64::new(0)),
            retries: Arc::new(AtomicU64::new(0)),
            tokens: Arc::new(AtomicU64::new(0)),
            cost: Arc::new(Mutex::new(0.0)),
        }
    }
    fn check_time(&self) -> Result<(), ExecutionError> {
        if let Some(limit) = self.limits.as_ref().and_then(|b| b.time_ms)
            && self.started.elapsed().as_millis() > u128::from(limit)
        {
            return Err(budget_failure(
                "time_ms",
                self.started.elapsed().as_millis() as f64,
                limit as f64,
            ));
        }
        Ok(())
    }
    fn reserve_call(&self) -> Result<(), ExecutionError> {
        let used = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(limit) = self.limits.as_ref().and_then(|b| b.tool_calls)
            && used > limit
        {
            return Err(budget_failure("tool_calls", used as f64, limit as f64));
        }
        self.check_time()
    }
    fn reserve_retry(&self) -> Result<(), ExecutionError> {
        let used = self.retries.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(limit) = self.limits.as_ref().and_then(|b| b.retries)
            && used > limit
        {
            return Err(budget_failure("retries", used as f64, limit as f64));
        }
        self.check_time()
    }
    fn consume(&self, usage: &Usage) -> Result<(), ExecutionError> {
        let increment = usage.input_tokens + usage.output_tokens;
        let tokens = self.tokens.fetch_add(increment, Ordering::SeqCst) + increment;
        if let Some(limit) = self.limits.as_ref().and_then(|b| b.tokens)
            && tokens > limit
        {
            return Err(budget_failure("tokens", tokens as f64, limit as f64));
        }
        let mut cost = self.cost.lock().unwrap();
        *cost += usage.cost_usd;
        if let Some(limit) = self.limits.as_ref().and_then(|b| b.cost_usd)
            && *cost > limit
        {
            return Err(budget_failure("cost_usd", *cost, limit));
        }
        self.check_time()
    }
    fn check_concurrency(&self, requested: usize) -> Result<(), ExecutionError> {
        if let Some(limit) = self.limits.as_ref().and_then(|b| b.concurrency)
            && requested as u64 > limit
        {
            return Err(budget_failure(
                "concurrency",
                requested as f64,
                limit as f64,
            ));
        }
        Ok(())
    }
}

pub fn execute_pipeline(
    program: &Program,
    name: &str,
    inputs: BTreeMap<String, Value>,
    registry: &Registry,
    context: &ExecutionContext,
) -> Result<Value, ExecutionError> {
    let execution_id = context.next_id("exec");
    execute(program, name, inputs, registry, context, 0, &execution_id)
}
fn execute(
    program: &Program,
    name: &str,
    inputs: BTreeMap<String, Value>,
    registry: &Registry,
    context: &ExecutionContext,
    depth: usize,
    execution_id: &str,
) -> Result<Value, ExecutionError> {
    if depth > 128 {
        return Err(fail("pipeline recursion limit exceeded"));
    }
    let p = program
        .pipelines
        .get(name)
        .ok_or_else(|| fail(format!("unknown pipeline '{name}'")))?;
    validate_args(&p.params, &inputs, program)?;
    let budget = BudgetState::new(p.budget.clone());
    let mut env = inputs;
    context.record(
        "pipeline_start",
        json!({"pipeline":name,"execution_id":execution_id,"budget":p.budget}),
    );
    match block(
        &p.statements,
        program,
        &mut env,
        registry,
        context,
        depth,
        execution_id,
        &budget,
    )? {
        Flow::Return(v) => {
            if !value_matches(&v, &p.return_type, program) {
                return Err(fail(format!(
                    "pipeline '{name}' returned a value incompatible with {:?}",
                    p.return_type
                )));
            }
            context.record(
                "pipeline_end",
                json!({"pipeline":name,"execution_id":execution_id,"result":v}),
            );
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
            let execution_id = context.next_id("test");
            let budget = BudgetState::new(None);
            let r = block(
                &t.statements,
                program,
                &mut env,
                registry,
                context,
                0,
                &execution_id,
                &budget,
            )
            .map(|_| ());
            (t.name.clone(), r)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)] // Explicit interpreter state keeps recursive control flow readable.
fn block(
    xs: &[Stmt],
    p: &Program,
    env: &mut BTreeMap<String, Value>,
    reg: &Registry,
    ctx: &ExecutionContext,
    depth: usize,
    execution_id: &str,
    budget: &BudgetState,
) -> Result<Flow, ExecutionError> {
    for s in xs {
        budget.check_time()?;
        match s {
            Stmt::Run(r) => {
                let v = run(r, p, env, reg, ctx, depth, execution_id, budget)?;
                env.insert(r.target.clone(), v);
            }
            Stmt::Parallel {
                branches,
                max_concurrency,
                ..
            } => {
                let snapshot = env.clone();
                let width = max_concurrency.unwrap_or(branches.len().max(1));
                budget.check_concurrency(width)?;
                for chunk in branches.chunks(width) {
                    let mut handles = Vec::new();
                    for r in chunk.iter().cloned() {
                        let (p, e, g, c, execution_id, budget) = (
                            p.clone(),
                            snapshot.clone(),
                            reg.clone(),
                            ctx.clone(),
                            execution_id.to_owned(),
                            budget.clone(),
                        );
                        handles.push(std::thread::spawn(move || {
                            run(&r, &p, &e, &g, &c, depth, &execution_id, &budget)
                                .map(|v| (r.target, v))
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
                match block(body, p, env, reg, ctx, depth, execution_id, budget)? {
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
                    match block(else_body, p, env, reg, ctx, depth, execution_id, budget)? {
                        Flow::Next => {}
                        x => return Ok(x),
                    }
                } else {
                    let old = env.insert(binding.clone(), v);
                    let f = block(then_body, p, env, reg, ctx, depth, execution_id, budget)?;
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
                    match block(body, p, env, reg, ctx, depth, execution_id, budget)? {
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
                if let Err(e) = block(try_body, p, env, reg, ctx, depth, execution_id, budget) {
                    env.insert(
                        error_var.clone(),
                        if *structured {
                            e.as_value()
                        } else {
                            Value::String(e.to_string())
                        },
                    );
                    let f = block(catch_body, p, env, reg, ctx, depth, execution_id, budget)?;
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
                let flow = block(&arm.body, p, env, reg, ctx, depth, execution_id, budget)?;
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
#[allow(clippy::too_many_arguments)] // Invocation execution needs the enclosing runtime scope.
fn run(
    r: &RunStmt,
    p: &Program,
    env: &BTreeMap<String, Value>,
    reg: &Registry,
    ctx: &ExecutionContext,
    depth: usize,
    execution_id: &str,
    budget: &BudgetState,
) -> Result<Value, ExecutionError> {
    let args = r
        .args
        .iter()
        .map(|(k, e)| Ok((k.clone(), eval(e, env)?)))
        .collect::<Result<BTreeMap<_, _>, ExecutionError>>()?;
    if let Some(x) = p.pipelines.get(&r.callable) {
        validate_args(&x.params, &args, p)?;
        return execute(p, &r.callable, args, reg, ctx, depth + 1, execution_id);
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
    let invocation_id = ctx.next_id("invoke");
    let idempotency_key = match &task.idempotency {
        Idempotency::KeyedBy(parameter) => args
            .get(parameter)
            .and_then(Value::as_str)
            .unwrap_or(&invocation_id)
            .to_owned(),
        _ => invocation_id.clone(),
    };
    let mut last = None;
    let mut attempts_made = 0;
    for attempt in 1..=attempts {
        budget.reserve_call()?;
        let invocation = Invocation {
            execution_id: execution_id.to_owned(),
            invocation_id: invocation_id.clone(),
            idempotency_key: idempotency_key.clone(),
            attempt,
        };
        attempts_made = attempt;
        ctx.record(
            "task_attempt",
            json!({"task":r.callable,"attempt":attempt,"agent":r.agent,"execution_id":execution_id,"invocation_id":invocation_id,"idempotency_key":idempotency_key}),
        );
        let result: Result<TaskOutput, ExecutionError> = if let Some(seconds) = r.timeout {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            let (handler, owned_args, agent, invocation) =
                (h.clone(), args.clone(), r.agent.clone(), invocation.clone());
            std::thread::spawn(move || {
                let _ = sender.send(handler(&owned_args, agent.as_deref(), &invocation));
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
            h(&args, r.agent.as_deref(), &invocation)
                .map_err(|message| failure("task", message, Some(r.callable.clone()), true))
        };
        match result {
            Ok(output) => {
                budget.consume(&output.usage)?;
                ctx.record("provider_usage", json!({"task":r.callable,"execution_id":execution_id,"invocation_id":invocation_id,"attempt":attempt,"usage":output.usage}));
                let v = output.value;
                ctx.record("task_result", json!({"task":r.callable,"execution_id":execution_id,"invocation_id":invocation_id,"attempt":attempt,"result":v}));
                if !value_matches(&v, &task.return_type, p) {
                    return Err(fail(format!(
                        "task '{}' returned incompatible value",
                        r.callable
                    )));
                }
                if attempt < attempts && retryable_typed_error(&v, &r.retry_on) {
                    budget.reserve_retry()?;
                    let delay_ms = ctx.retry_delay_ms(
                        r.retry_policy.initial_ms,
                        r.retry_policy.max_ms,
                        r.retry_policy.multiplier,
                        r.retry_policy.jitter,
                        attempt,
                        &invocation_id,
                    );
                    ctx.record(
                        "task_retry",
                        json!({"task":r.callable,"attempt":attempt,"reason":"typed_error","execution_id":execution_id,"invocation_id":invocation_id,"idempotency_key":idempotency_key,"delay_ms":delay_ms}),
                    );
                    ctx.wait_retry(delay_ms);
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
                if attempt < attempts {
                    budget.reserve_retry()?;
                    let delay_ms = ctx.retry_delay_ms(
                        r.retry_policy.initial_ms,
                        r.retry_policy.max_ms,
                        r.retry_policy.multiplier,
                        r.retry_policy.jitter,
                        attempt,
                        &invocation_id,
                    );
                    ctx.record("task_retry", json!({"task":r.callable,"attempt":attempt,"reason":"failure","execution_id":execution_id,"invocation_id":invocation_id,"idempotency_key":idempotency_key,"delay_ms":delay_ms}));
                    ctx.wait_retry(delay_ms);
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

fn budget_failure(resource: &str, used: f64, limit: f64) -> ExecutionError {
    failure(
        "budget",
        format!("{resource} budget exhausted: used {used}, limit {limit}"),
        Some(resource.into()),
        false,
    )
}
