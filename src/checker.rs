use crate::ast::*;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("[AGL2001] {message} at {line}:{col}")]
pub struct CheckError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}
impl CheckError {
    pub const fn code(&self) -> &'static str {
        "AGL2001"
    }
}
type Env = BTreeMap<String, TypeExpr>;

pub fn check_program(p: &Program) -> Result<(), CheckError> {
    for e in p.enums.values() {
        if e.is_empty() {
            return Err(at(
                Span { line: 1, col: 1 },
                "enum must have at least one variant",
            ));
        }
    }
    for a in p.agents.values() {
        for t in &a.tools {
            if !p.tools.contains_key(t) {
                return Err(at(
                    Span { line: 1, col: 1 },
                    format!("agent '{}' references unknown tool '{t}'", a.name),
                ));
            }
        }
    }
    for x in p.pipelines.values() {
        let mut env = x
            .params
            .iter()
            .map(|v| (v.name.clone(), resolve(&v.ty, p)))
            .collect();
        let returns = block(&x.statements, p, &mut env, false)?;
        let expected = resolve(&x.return_type, p);
        if !returns.iter().any(|t| assignable(t, &expected)) {
            return Err(at(
                x.statements
                    .last()
                    .map(Stmt::span)
                    .unwrap_or(Span { line: 1, col: 1 }),
                format!("pipeline '{}' has no compatible return", x.name),
            ));
        }
    }
    for x in &p.tests {
        let mut env = Env::new();
        block(&x.statements, p, &mut env, false)?;
    }
    Ok(())
}

fn block(
    xs: &[Stmt],
    p: &Program,
    env: &mut Env,
    in_loop: bool,
) -> Result<Vec<TypeExpr>, CheckError> {
    let mut returns = Vec::new();
    for s in xs {
        match s {
            Stmt::Run(r) => {
                let ty = run(r, p, env)?;
                env.insert(r.target.clone(), ty);
            }
            Stmt::Parallel {
                branches,
                max_concurrency,
                ..
            } => {
                if max_concurrency == &Some(0) {
                    return Err(at(s.span(), "max_concurrency must be at least 1"));
                }
                let mut names = std::collections::BTreeSet::new();
                for r in branches {
                    if env.contains_key(&r.target) || !names.insert(r.target.clone()) {
                        return Err(at(
                            r.span,
                            format!("parallel target '{}' must be fresh and distinct", r.target),
                        ));
                    }
                    let ty = run(r, p, env)?;
                    env.insert(r.target.clone(), ty);
                }
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                need(
                    &infer(condition, env, p)?,
                    &TypeExpr::Bool,
                    condition.span(),
                )?;
                let (mut a, mut b) = (env.clone(), env.clone());
                returns.extend(block(then_body, p, &mut a, in_loop)?);
                returns.extend(block(else_body, p, &mut b, in_loop)?);
                *env = common(a, b)
            }
            Stmt::IfLet {
                binding,
                option,
                then_body,
                else_body,
                ..
            } => {
                let inner = match infer(option, env, p)? {
                    TypeExpr::Option(x) => *x,
                    x => {
                        return Err(at(
                            option.span(),
                            format!("if let requires Option, got {x:?}"),
                        ));
                    }
                };
                let (mut a, mut b) = (env.clone(), env.clone());
                a.insert(binding.clone(), inner);
                returns.extend(block(then_body, p, &mut a, in_loop)?);
                returns.extend(block(else_body, p, &mut b, in_loop)?);
                *env = common(a, b)
            }
            Stmt::While {
                condition, body, ..
            } => {
                need(
                    &infer(condition, env, p)?,
                    &TypeExpr::Bool,
                    condition.span(),
                )?;
                let mut inner = env.clone();
                returns.extend(block(body, p, &mut inner, true)?)
            }
            Stmt::Break(sp) | Stmt::Continue(sp) => {
                if !in_loop {
                    return Err(at(*sp, "break/continue outside while loop"));
                }
            }
            Stmt::TryCatch {
                try_body,
                error_var,
                catch_body,
                ..
            } => {
                let (mut a, mut b) = (env.clone(), env.clone());
                b.insert(error_var.clone(), TypeExpr::String);
                returns.extend(block(try_body, p, &mut a, in_loop)?);
                returns.extend(block(catch_body, p, &mut b, in_loop)?);
                *env = common(a, b)
            }
            Stmt::Assert { condition, .. } => need(
                &infer(condition, env, p)?,
                &TypeExpr::Bool,
                condition.span(),
            )?,
            Stmt::Return { expr, .. } => returns.push(infer(expr, env, p)?),
        }
    }
    Ok(returns)
}

fn run(r: &RunStmt, p: &Program, env: &Env) -> Result<TypeExpr, CheckError> {
    let (params, ret, agent_task, is_pipeline) = if let Some(t) = p.tasks.get(&r.callable) {
        (&t.params, &t.return_type, t.agent_task, false)
    } else if let Some(x) = p.pipelines.get(&r.callable) {
        (&x.params, &x.return_type, false, true)
    } else {
        return Err(at(
            r.span,
            format!("unknown task or pipeline '{}'", r.callable),
        ));
    };
    if is_pipeline
        && (r.agent.is_some()
            || r.retries > 0
            || !matches!(r.on_fail, OnFail::Abort)
            || r.timeout.is_some())
    {
        return Err(at(
            r.span,
            "pipeline calls do not support by/retries/on_fail/timeout",
        ));
    }
    if agent_task && r.agent.is_none() {
        return Err(at(
            r.span,
            format!("agent task '{}' requires 'by <agent>'", r.callable),
        ));
    }
    if let Some(a) = &r.agent
        && !p.agents.contains_key(a)
    {
        return Err(at(r.span, format!("unknown agent '{a}'")));
    }
    let expected: std::collections::BTreeSet<_> = params.iter().map(|x| x.name.as_str()).collect();
    let actual: std::collections::BTreeSet<_> = r.args.keys().map(String::as_str).collect();
    if expected != actual {
        return Err(at(
            r.span,
            format!(
                "'{}' argument names do not match; expected {:?}, got {:?}",
                r.callable, expected, actual
            ),
        ));
    }
    for x in params {
        let a = infer(&r.args[&x.name], env, p)?;
        let e = resolve(&x.ty, p);
        need(&a, &e, r.args[&x.name].span())?
    }
    let result = resolve(ret, p);
    if let OnFail::Use(e) = &r.on_fail {
        let fallback = infer(e, env, p)?;
        need(&fallback, &result, e.span())?
    }
    Ok(result)
}

fn infer(e: &Expr, env: &Env, p: &Program) -> Result<TypeExpr, CheckError> {
    match e {
        Expr::Literal { value, .. } => Ok(match value {
            serde_json::Value::String(s) => p
                .enums
                .iter()
                .find(|(_, variants)| variants.iter().any(|variant| variant == s))
                .map(|(name, _)| TypeExpr::Enum(name.clone()))
                .unwrap_or(TypeExpr::String),
            serde_json::Value::Number(_) => TypeExpr::Number,
            serde_json::Value::Bool(_) => TypeExpr::Bool,
            serde_json::Value::Null => TypeExpr::Option(Box::new(TypeExpr::Alias("<null>".into()))),
            _ => unreachable!(),
        }),
        Expr::Ref { parts, span } => {
            let mut t = env
                .get(&parts[0])
                .cloned()
                .ok_or_else(|| at(*span, format!("unknown reference '{}'", parts[0])))?;
            for field in &parts[1..] {
                t = match resolve(&t, p) {
                    TypeExpr::Obj(fs) => fs
                        .get(field)
                        .cloned()
                        .ok_or_else(|| at(*span, format!("unknown field '{field}'")))?,
                    x => return Err(at(*span, format!("cannot access field '{field}' on {x:?}"))),
                }
            }
            Ok(t)
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            let (a, b) = (infer(left, env, p)?, infer(right, env, p)?);
            match op {
                BinaryOp::Add => {
                    if a == TypeExpr::String && b == TypeExpr::String {
                        Ok(TypeExpr::String)
                    } else if a == TypeExpr::Number && b == TypeExpr::Number {
                        Ok(TypeExpr::Number)
                    } else {
                        Err(at(*span, "'+' requires two strings or two numbers"))
                    }
                }
                BinaryOp::Eq | BinaryOp::Ne => {
                    if comparable(&a, &b) {
                        Ok(TypeExpr::Bool)
                    } else {
                        Err(at(*span, "incompatible equality operands"))
                    }
                }
            }
        }
        Expr::Obj { fields, .. } => Ok(TypeExpr::Obj(
            fields
                .iter()
                .map(|(k, v)| Ok((k.clone(), infer(v, env, p)?)))
                .collect::<Result<_, CheckError>>()?,
        )),
        Expr::List { items, span } => {
            let Some(first) = items.first() else {
                return Err(at(*span, "cannot infer type of empty list"));
            };
            let t = infer(first, env, p)?;
            for x in &items[1..] {
                need(&infer(x, env, p)?, &t, x.span())?
            }
            Ok(TypeExpr::List(Box::new(t)))
        }
    }
}
fn resolve(t: &TypeExpr, p: &Program) -> TypeExpr {
    match t {
        TypeExpr::Alias(n) => p
            .aliases
            .get(n)
            .map(|x| resolve(x, p))
            .unwrap_or_else(|| t.clone()),
        TypeExpr::List(x) => TypeExpr::List(Box::new(resolve(x, p))),
        TypeExpr::Option(x) => TypeExpr::Option(Box::new(resolve(x, p))),
        TypeExpr::Obj(fs) => {
            TypeExpr::Obj(fs.iter().map(|(k, v)| (k.clone(), resolve(v, p))).collect())
        }
        _ => t.clone(),
    }
}
fn assignable(a: &TypeExpr, e: &TypeExpr) -> bool {
    if a == e {
        return true;
    }
    match (a, e) {
        (TypeExpr::Option(x), TypeExpr::Option(y)) => {
            x.as_ref() == &TypeExpr::Alias("<null>".into()) || assignable(x, y)
        }
        (TypeExpr::List(x), TypeExpr::List(y)) => assignable(x, y),
        (TypeExpr::Obj(a), TypeExpr::Obj(e)) => e
            .iter()
            .all(|(k, v)| a.get(k).is_some_and(|x| assignable(x, v))),
        (TypeExpr::Enum(_), TypeExpr::String) => true,
        _ => false,
    }
}
fn comparable(a: &TypeExpr, b: &TypeExpr) -> bool {
    assignable(a, b)
        || assignable(b, a)
        || matches!(
            (a, b),
            (TypeExpr::Enum(_), TypeExpr::String) | (TypeExpr::String, TypeExpr::Enum(_))
        )
}
fn need(a: &TypeExpr, e: &TypeExpr, sp: Span) -> Result<(), CheckError> {
    if assignable(a, e) {
        Ok(())
    } else {
        Err(at(sp, format!("type mismatch: expected {e:?}, got {a:?}")))
    }
}
fn common(a: Env, b: Env) -> Env {
    a.into_iter()
        .filter_map(|(k, v)| {
            b.get(&k)
                .filter(|w| assignable(&v, w) && assignable(w, &v))
                .map(|_| (k, v))
        })
        .collect()
}
fn at(sp: Span, message: impl Into<String>) -> CheckError {
    CheckError {
        message: message.into(),
        line: sp.line,
        col: sp.col,
    }
}
