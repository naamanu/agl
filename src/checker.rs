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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisWarning {
    pub code: &'static str,
    pub message: String,
    pub line: usize,
    pub col: usize,
}

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
        if p.language_version != "0.2" && !block_terminates(&x.statements) {
            return Err(at(
                x.statements
                    .last()
                    .map(Stmt::span)
                    .unwrap_or(Span { line: 1, col: 1 }),
                format!("pipeline '{}' does not return on every path", x.name),
            ));
        }
    }
    for x in &p.tests {
        let mut env = Env::new();
        block(&x.statements, p, &mut env, false)?;
    }
    for eval in p.evals.values() {
        if !p.pipelines.contains_key(&eval.pipeline) {
            return Err(at(
                Span { line: 1, col: 1 },
                format!(
                    "eval '{}' references unknown pipeline '{}'",
                    eval.name, eval.pipeline
                ),
            ));
        }
        if let Some(grader) = &eval.semantic_grader {
            let Some(pipeline) = p.pipelines.get(grader) else {
                return Err(at(
                    Span { line: 1, col: 1 },
                    format!(
                        "eval '{}' references unknown semantic grader pipeline '{grader}'",
                        eval.name
                    ),
                ));
            };
            if resolve(&pipeline.return_type, p) != TypeExpr::Bool {
                return Err(at(
                    Span { line: 1, col: 1 },
                    format!("semantic grader '{grader}' must return Bool"),
                ));
            }
        }
    }
    validate_contracts(p)?;
    let inferred = infer_program_effects(p);
    for pipeline in p.pipelines.values() {
        if let Some(declared) = &pipeline.effects {
            let missing: Vec<_> = inferred[&pipeline.name]
                .difference(declared)
                .cloned()
                .collect();
            if !missing.is_empty() {
                return Err(at(
                    pipeline
                        .statements
                        .first()
                        .map(Stmt::span)
                        .unwrap_or(Span { line: 1, col: 1 }),
                    format!(
                        "pipeline '{}' does not permit inferred effects {missing:?}",
                        pipeline.name
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_contracts(program: &Program) -> Result<(), CheckError> {
    for task in program.tasks.values() {
        validate_contract(
            "task",
            &task.name,
            &task.params,
            &task.effects,
            &task.idempotency,
            &task.concurrency_group,
            task.concurrency_limit,
            task.rate_limit_per_second,
            program,
        )?;
    }
    for tool in program.tools.values() {
        validate_contract(
            "tool",
            &tool.name,
            &tool.params,
            &tool.effects,
            &tool.idempotency,
            &tool.concurrency_group,
            tool.concurrency_limit,
            tool.rate_limit_per_second,
            program,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Mirrors the declaration contract being validated.
fn validate_contract(
    kind: &str,
    name: &str,
    params: &[Param],
    effects: &std::collections::BTreeSet<String>,
    idempotency: &Idempotency,
    concurrency_group: &Option<String>,
    concurrency_limit: Option<u32>,
    rate_limit_per_second: Option<u32>,
    program: &Program,
) -> Result<(), CheckError> {
    let span = Span { line: 1, col: 1 };
    if concurrency_group.is_none()
        && (concurrency_limit.is_some() || rate_limit_per_second.is_some())
    {
        return Err(at(
            span,
            format!("{kind} '{name}' must declare concurrency_group before limits"),
        ));
    }
    if matches!(idempotency, Idempotency::Pure) && !effects.is_empty() {
        return Err(at(
            span,
            format!("pure {kind} '{name}' cannot declare effects"),
        ));
    }
    if let Idempotency::KeyedBy(parameter) = idempotency {
        let Some(param) = params.iter().find(|param| &param.name == parameter) else {
            return Err(at(
                span,
                format!("{kind} '{name}' has no idempotency parameter '{parameter}'"),
            ));
        };
        if resolve(&param.ty, program) != TypeExpr::String {
            return Err(at(
                span,
                format!("idempotency key '{parameter}' on {kind} '{name}' must be String"),
            ));
        }
    }
    Ok(())
}

pub fn infer_program_effects(
    program: &Program,
) -> BTreeMap<String, std::collections::BTreeSet<String>> {
    let mut effects: BTreeMap<_, _> = program
        .pipelines
        .iter()
        .map(|(name, pipeline)| {
            let mut direct = std::collections::BTreeSet::new();
            collect_direct_effects(&pipeline.statements, program, &mut direct);
            (name.clone(), direct)
        })
        .collect();
    for _ in 0..=program.pipelines.len() {
        let previous = effects.clone();
        let mut changed = false;
        for (name, pipeline) in &program.pipelines {
            let mut callees = std::collections::BTreeSet::new();
            collect_pipeline_calls(&pipeline.statements, program, &mut callees);
            let target = effects.get_mut(name).unwrap();
            for callee in callees {
                if let Some(inherited) = previous.get(&callee) {
                    let before = target.len();
                    target.extend(inherited.iter().cloned());
                    changed |= target.len() != before;
                }
            }
        }
        if !changed {
            break;
        }
    }
    effects
}

fn collect_direct_effects(
    statements: &[Stmt],
    program: &Program,
    effects: &mut std::collections::BTreeSet<String>,
) {
    if contains_approval(statements) {
        effects.insert("human".into());
    }
    visit_runs(statements, &mut |run| {
        let Some(task) = program.tasks.get(&run.callable) else {
            return;
        };
        effects.extend(task.effects.iter().cloned());
        if task.agent_task || run.agent.is_some() {
            effects.insert("model".into());
        }
        if let Some(agent) = run.agent.as_ref().and_then(|name| program.agents.get(name)) {
            for tool in &agent.tools {
                if let Some(tool) = program.tools.get(tool) {
                    effects.extend(tool.effects.iter().cloned());
                }
            }
        }
    });
}

fn contains_approval(statements: &[Stmt]) -> bool {
    statements.iter().any(|statement| match statement {
        Stmt::Approve { .. } => true,
        Stmt::If {
            then_body,
            else_body,
            ..
        }
        | Stmt::IfLet {
            then_body,
            else_body,
            ..
        } => contains_approval(then_body) || contains_approval(else_body),
        Stmt::While { body, .. } => contains_approval(body),
        Stmt::TryCatch {
            try_body,
            catch_body,
            ..
        } => contains_approval(try_body) || contains_approval(catch_body),
        Stmt::Match { arms, .. } => arms.iter().any(|arm| contains_approval(&arm.body)),
        _ => false,
    })
}

fn collect_pipeline_calls(
    statements: &[Stmt],
    program: &Program,
    calls: &mut std::collections::BTreeSet<String>,
) {
    visit_runs(statements, &mut |run| {
        if program.pipelines.contains_key(&run.callable) {
            calls.insert(run.callable.clone());
        }
    });
}

fn visit_runs(statements: &[Stmt], visitor: &mut impl FnMut(&RunStmt)) {
    for statement in statements {
        match statement {
            Stmt::Run(run) => visitor(run),
            Stmt::Parallel { branches, .. } => branches.iter().for_each(&mut *visitor),
            Stmt::ParallelMap { run, .. } => visitor(run),
            Stmt::Race { branches, .. } => branches.iter().for_each(&mut *visitor),
            Stmt::If {
                then_body,
                else_body,
                ..
            }
            | Stmt::IfLet {
                then_body,
                else_body,
                ..
            } => {
                visit_runs(then_body, visitor);
                visit_runs(else_body, visitor);
            }
            Stmt::While { body, .. } => visit_runs(body, visitor),
            Stmt::TryCatch {
                try_body,
                catch_body,
                ..
            } => {
                visit_runs(try_body, visitor);
                visit_runs(catch_body, visitor);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    visit_runs(&arm.body, visitor);
                }
            }
            Stmt::Approve { .. }
            | Stmt::Assert { .. }
            | Stmt::Return { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_) => {}
        }
    }
}

pub fn analyze_program(program: &Program) -> Vec<AnalysisWarning> {
    let mut warnings = Vec::new();
    for pipeline in program.pipelines.values() {
        analyze_block(&pipeline.statements, &mut warnings);
    }
    warnings
}

fn analyze_block(statements: &[Stmt], warnings: &mut Vec<AnalysisWarning>) {
    let mut terminated = false;
    for (index, statement) in statements.iter().enumerate() {
        if terminated {
            let span = statement.span();
            warnings.push(AnalysisWarning {
                code: "AGLW2002",
                message: "unreachable statement".into(),
                line: span.line,
                col: span.col,
            });
        }
        match statement {
            Stmt::Run(run) => {
                warn_unused(&run.target, run.span, &statements[index + 1..], warnings)
            }
            Stmt::Approve { target, span, .. } => {
                warn_unused(target, *span, &statements[index + 1..], warnings)
            }
            Stmt::Parallel { branches, .. } => {
                for run in branches {
                    warn_unused(&run.target, run.span, &statements[index + 1..], warnings);
                }
            }
            Stmt::ParallelMap { target, span, .. } | Stmt::Race { target, span, .. } => {
                warn_unused(target, *span, &statements[index + 1..], warnings);
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                analyze_block(then_body, warnings);
                analyze_block(else_body, warnings);
            }
            Stmt::IfLet {
                binding,
                then_body,
                else_body,
                span,
                ..
            } => {
                if !block_references(then_body, binding) {
                    warnings.push(AnalysisWarning {
                        code: "AGLW2001",
                        message: format!("unused option binding '{binding}'"),
                        line: span.line,
                        col: span.col,
                    });
                }
                analyze_block(then_body, warnings);
                analyze_block(else_body, warnings);
            }
            Stmt::While { body, .. } => analyze_block(body, warnings),
            Stmt::TryCatch {
                try_body,
                error_var,
                catch_body,
                span,
                ..
            } => {
                analyze_block(try_body, warnings);
                if !block_references(catch_body, error_var) {
                    warnings.push(AnalysisWarning {
                        code: "AGLW2001",
                        message: format!("unused error binding '{error_var}'"),
                        line: span.line,
                        col: span.col,
                    });
                }
                analyze_block(catch_body, warnings);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    for binding in &arm.bindings {
                        if !block_references(&arm.body, binding) {
                            warnings.push(AnalysisWarning {
                                code: "AGLW2001",
                                message: format!("unused match binding '{binding}'"),
                                line: arm.span.line,
                                col: arm.span.col,
                            });
                        }
                    }
                    analyze_block(&arm.body, warnings);
                }
            }
            Stmt::Assert { .. } | Stmt::Return { .. } | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
        terminated |= statement_terminates(statement);
    }
}

fn warn_unused(name: &str, span: Span, remaining: &[Stmt], warnings: &mut Vec<AnalysisWarning>) {
    if !block_references(remaining, name) {
        warnings.push(AnalysisWarning {
            code: "AGLW2001",
            message: format!("unused binding '{name}'"),
            line: span.line,
            col: span.col,
        });
    }
}

fn block_references(statements: &[Stmt], name: &str) -> bool {
    statements.iter().any(|statement| match statement {
        Stmt::Run(run) => run.args.values().any(|expr| expr_references(expr, name)),
        Stmt::Parallel { branches, .. } => branches
            .iter()
            .flat_map(|run| run.args.values())
            .any(|expr| expr_references(expr, name)),
        Stmt::ParallelMap { items, run, .. } => {
            expr_references(items, name)
                || run.args.values().any(|expr| expr_references(expr, name))
        }
        Stmt::Race { branches, .. } => branches
            .iter()
            .flat_map(|run| run.args.values())
            .any(|expr| expr_references(expr, name)),
        Stmt::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            expr_references(condition, name)
                || block_references(then_body, name)
                || block_references(else_body, name)
        }
        Stmt::IfLet {
            option,
            then_body,
            else_body,
            ..
        } => {
            expr_references(option, name)
                || block_references(then_body, name)
                || block_references(else_body, name)
        }
        Stmt::While {
            condition, body, ..
        } => expr_references(condition, name) || block_references(body, name),
        Stmt::TryCatch {
            try_body,
            catch_body,
            ..
        } => block_references(try_body, name) || block_references(catch_body, name),
        Stmt::Match { value, arms, .. } => {
            expr_references(value, name) || arms.iter().any(|arm| block_references(&arm.body, name))
        }
        Stmt::Assert { condition, .. } => expr_references(condition, name),
        Stmt::Return { expr, .. } => expr_references(expr, name),
        Stmt::Approve { .. } | Stmt::Break(_) | Stmt::Continue(_) => false,
    })
}

fn expr_references(expression: &Expr, name: &str) -> bool {
    match expression {
        Expr::Ref { parts, .. } => parts.first().is_some_and(|part| part == name),
        Expr::Binary { left, right, .. } => {
            expr_references(left, name) || expr_references(right, name)
        }
        Expr::Obj { fields, .. } | Expr::Record { fields, .. } | Expr::Variant { fields, .. } => {
            fields.values().any(|value| expr_references(value, name))
        }
        Expr::Result { value, .. } => expr_references(value, name),
        Expr::List { items, .. } => items.iter().any(|value| expr_references(value, name)),
        Expr::Literal { .. } => false,
    }
}

fn block_terminates(statements: &[Stmt]) -> bool {
    statements.iter().any(statement_terminates)
}

fn statement_terminates(statement: &Stmt) -> bool {
    match statement {
        Stmt::Return { .. } | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If {
            then_body,
            else_body,
            ..
        } => !else_body.is_empty() && block_terminates(then_body) && block_terminates(else_body),
        Stmt::IfLet {
            then_body,
            else_body,
            ..
        } => !else_body.is_empty() && block_terminates(then_body) && block_terminates(else_body),
        Stmt::TryCatch {
            try_body,
            catch_body,
            ..
        } => block_terminates(try_body) && block_terminates(catch_body),
        Stmt::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|arm| block_terminates(&arm.body))
        }
        _ => false,
    }
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
            Stmt::Approve { target, .. } => {
                env.insert(target.clone(), TypeExpr::Bool);
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
            Stmt::ParallelMap {
                target,
                binding,
                items,
                run: mapped,
                ..
            } => {
                if p.pipelines.contains_key(&mapped.callable) {
                    return Err(at(
                        mapped.span,
                        "parallel map currently requires a direct task call",
                    ));
                }
                let item = match infer(items, env, p)? {
                    TypeExpr::List(inner) => *inner,
                    actual => {
                        return Err(at(
                            items.span(),
                            format!("parallel map requires List, got {actual:?}"),
                        ));
                    }
                };
                let mut branch = env.clone();
                branch.insert(binding.clone(), item);
                let output = run(mapped, p, &branch)?;
                env.insert(target.clone(), TypeExpr::List(Box::new(output)));
            }
            Stmt::Race {
                target, branches, ..
            } => {
                if branches
                    .iter()
                    .any(|branch| p.pipelines.contains_key(&branch.callable))
                {
                    return Err(at(s.span(), "race currently requires direct task calls"));
                }
                let mut outputs = branches.iter().map(|branch| run(branch, p, env));
                let first = outputs.next().expect("parser requires race branches")?;
                for output in outputs {
                    let output = output?;
                    if !assignable(&output, &first) || !assignable(&first, &output) {
                        return Err(at(s.span(), "race branches must return the same type"));
                    }
                }
                env.insert(target.clone(), first);
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
                structured,
                catch_body,
                ..
            } => {
                let (mut a, mut b) = (env.clone(), env.clone());
                b.insert(
                    error_var.clone(),
                    if *structured {
                        TypeExpr::Failure
                    } else {
                        TypeExpr::String
                    },
                );
                returns.extend(block(try_body, p, &mut a, in_loop)?);
                returns.extend(block(catch_body, p, &mut b, in_loop)?);
                *env = common(a, b)
            }
            Stmt::Match { value, arms, .. } => {
                let matched = resolve(&infer(value, env, p)?, p);
                let (type_name, variants) = match matched {
                    TypeExpr::Union(name) => {
                        let variants = p.unions[&name].variants.clone();
                        (name, variants)
                    }
                    TypeExpr::Result(ok, error) => (
                        "Result".into(),
                        BTreeMap::from([
                            ("Ok".into(), BTreeMap::from([("value".into(), *ok)])),
                            ("Err".into(), BTreeMap::from([("error".into(), *error)])),
                        ]),
                    ),
                    other => {
                        return Err(at(
                            value.span(),
                            format!("match requires a union or Result, got {other:?}"),
                        ));
                    }
                };
                let mut seen = std::collections::BTreeSet::new();
                let mut branch_envs = Vec::new();
                for arm in arms {
                    if arm.type_name != type_name {
                        return Err(at(
                            arm.span,
                            format!("match arm uses '{}', expected '{type_name}'", arm.type_name),
                        ));
                    }
                    let fields = variants.get(&arm.variant).ok_or_else(|| {
                        at(
                            arm.span,
                            format!(
                                "unknown variant '{type_name}::{}'{}",
                                arm.variant,
                                suggestion(&arm.variant, variants.keys())
                            ),
                        )
                    })?;
                    if !seen.insert(arm.variant.clone()) {
                        return Err(at(
                            arm.span,
                            format!(
                                "unreachable duplicate match arm '{}::{}'",
                                type_name, arm.variant
                            ),
                        ));
                    }
                    let expected: std::collections::BTreeSet<_> = fields.keys().collect();
                    let actual: std::collections::BTreeSet<_> = arm.bindings.iter().collect();
                    if actual != expected {
                        return Err(at(
                            arm.span,
                            format!(
                                "match bindings for '{}::{}' do not match fields",
                                type_name, arm.variant
                            ),
                        ));
                    }
                    let mut branch = env.clone();
                    let shadowed: Vec<_> = arm
                        .bindings
                        .iter()
                        .map(|binding| (binding.clone(), branch.get(binding).cloned()))
                        .collect();
                    for binding in &arm.bindings {
                        branch.insert(binding.clone(), resolve(&fields[binding], p));
                    }
                    returns.extend(block(&arm.body, p, &mut branch, in_loop)?);
                    for (binding, previous) in shadowed {
                        if let Some(ty) = previous {
                            branch.insert(binding, ty);
                        } else {
                            branch.remove(&binding);
                        }
                    }
                    branch_envs.push(branch);
                }
                let missing: Vec<_> = variants
                    .keys()
                    .filter(|variant| !seen.contains(*variant))
                    .cloned()
                    .collect();
                if !missing.is_empty() {
                    return Err(at(
                        s.span(),
                        format!("non-exhaustive match; missing {missing:?}"),
                    ));
                }
                if let Some(first) = branch_envs.into_iter().reduce(common) {
                    *env = first;
                }
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
            format!(
                "unknown task or pipeline '{}'{}",
                r.callable,
                suggestion(&r.callable, p.tasks.keys().chain(p.pipelines.keys()))
            ),
        ));
    };
    if is_pipeline
        && (r.agent.is_some()
            || r.retries > 0
            || !r.retry_on.is_empty()
            || !matches!(r.on_fail, OnFail::Abort)
            || r.timeout.is_some())
    {
        return Err(at(
            r.span,
            "pipeline calls do not support by/retries/retry_on/on_fail/timeout",
        ));
    }
    if r.retry_policy != RetryPolicy::default() && r.retries == 0 {
        return Err(at(r.span, "backoff requires a positive retries budget"));
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
        return Err(at(
            r.span,
            format!("unknown agent '{a}'{}", suggestion(a, p.agents.keys())),
        ));
    }
    if r.retries > 0
        && let Some(task) = p.tasks.get(&r.callable)
    {
        if matches!(task.idempotency, Idempotency::NonIdempotent)
            || (task.effects.contains("external_write") && !safe_to_retry(&task.idempotency))
        {
            return Err(at(
                r.span,
                format!(
                    "task '{}' cannot be retried safely; declare idempotent or keyed idempotency",
                    task.name
                ),
            ));
        }
        if let Some(agent) = r.agent.as_ref().and_then(|name| p.agents.get(name)) {
            for tool_name in &agent.tools {
                let tool = &p.tools[tool_name];
                if tool.effects.contains("external_write") && !safe_to_retry(&tool.idempotency) {
                    return Err(at(
                        r.span,
                        format!(
                            "task '{}' cannot be retried because agent tool '{}' performs an unsafe external write",
                            task.name, tool.name
                        ),
                    ));
                }
            }
        }
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
    if !r.retry_on.is_empty() {
        if r.retries == 0 {
            return Err(at(r.span, "retry_on requires a positive retries budget"));
        }
        let error_union = match &result {
            TypeExpr::Result(_, error) => match error.as_ref() {
                TypeExpr::Union(name) => name,
                _ => {
                    return Err(at(
                        r.span,
                        "retry_on requires Result[T, E] where E is a union",
                    ));
                }
            },
            _ => {
                return Err(at(
                    r.span,
                    "retry_on is only valid for tasks returning Result[T, E]",
                ));
            }
        };
        let variants = &p.unions[error_union].variants;
        let mut seen = std::collections::BTreeSet::new();
        for (type_name, variant) in &r.retry_on {
            if type_name != error_union || !variants.contains_key(variant) {
                return Err(at(
                    r.span,
                    format!(
                        "retry_on selector '{type_name}::{variant}' is not a variant of '{error_union}'"
                    ),
                ));
            }
            if !seen.insert((type_name, variant)) {
                return Err(at(r.span, "duplicate retry_on selector"));
            }
        }
    }
    if let OnFail::Use(e) = &r.on_fail {
        let fallback = infer(e, env, p)?;
        need(&fallback, &result, e.span())?
    }
    Ok(result)
}
fn safe_to_retry(idempotency: &Idempotency) -> bool {
    matches!(
        idempotency,
        Idempotency::Pure | Idempotency::Idempotent | Idempotency::KeyedBy(_)
    )
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
            let mut t = env.get(&parts[0]).cloned().ok_or_else(|| {
                at(
                    *span,
                    format!(
                        "unknown reference '{}'{}",
                        parts[0],
                        suggestion(&parts[0], env.keys())
                    ),
                )
            })?;
            for field in &parts[1..] {
                t = match resolve(&t, p) {
                    TypeExpr::Obj(fs) => fs.get(field).cloned().ok_or_else(|| {
                        at(
                            *span,
                            format!("unknown field '{field}'{}", suggestion(field, fs.keys())),
                        )
                    })?,
                    TypeExpr::Record(name) => {
                        let fields = &p.records[&name].fields;
                        fields.get(field).cloned().ok_or_else(|| {
                            at(
                                *span,
                                format!(
                                    "unknown field '{field}'{}",
                                    suggestion(field, fields.keys())
                                ),
                            )
                        })?
                    }
                    TypeExpr::Failure => failure_fields().get(field).cloned().ok_or_else(|| {
                        at(
                            *span,
                            format!(
                                "unknown Failure field '{field}'{}",
                                suggestion(field, failure_fields().keys())
                            ),
                        )
                    })?,
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
        Expr::Record { name, fields, span } => {
            let definition = p
                .records
                .get(name)
                .ok_or_else(|| at(*span, format!("unknown record '{name}'")))?;
            if fields.keys().collect::<Vec<_>>() != definition.fields.keys().collect::<Vec<_>>() {
                return Err(at(
                    *span,
                    format!("record '{name}' constructor fields do not match its declaration"),
                ));
            }
            for (field, expected) in &definition.fields {
                let value = &fields[field];
                need(&infer(value, env, p)?, &resolve(expected, p), value.span())?;
            }
            Ok(TypeExpr::Record(name.clone()))
        }
        Expr::Variant {
            type_name,
            variant,
            fields,
            span,
        } => {
            let union = p
                .unions
                .get(type_name)
                .ok_or_else(|| at(*span, format!("unknown union '{type_name}'")))?;
            let expected = union.variants.get(variant).ok_or_else(|| {
                at(
                    *span,
                    format!(
                        "unknown variant '{type_name}::{variant}'{}",
                        suggestion(variant, union.variants.keys())
                    ),
                )
            })?;
            if fields.keys().collect::<Vec<_>>() != expected.keys().collect::<Vec<_>>() {
                return Err(at(
                    *span,
                    format!("variant '{type_name}::{variant}' fields do not match"),
                ));
            }
            for (field, expected) in expected {
                let value = &fields[field];
                need(&infer(value, env, p)?, &resolve(expected, p), value.span())?;
            }
            Ok(TypeExpr::Union(type_name.clone()))
        }
        Expr::Result { ok, value, .. } => {
            let value = Box::new(infer(value, env, p)?);
            let unknown = Box::new(TypeExpr::Alias("<infer>".into()));
            Ok(if *ok {
                TypeExpr::Result(value, unknown)
            } else {
                TypeExpr::Result(unknown, value)
            })
        }
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
        TypeExpr::Result(ok, error) => {
            TypeExpr::Result(Box::new(resolve(ok, p)), Box::new(resolve(error, p)))
        }
        TypeExpr::Obj(fs) => {
            TypeExpr::Obj(fs.iter().map(|(k, v)| (k.clone(), resolve(v, p))).collect())
        }
        _ => t.clone(),
    }
}
fn assignable(a: &TypeExpr, e: &TypeExpr) -> bool {
    if matches!(a, TypeExpr::Alias(name) if name == "<infer>")
        || matches!(e, TypeExpr::Alias(name) if name == "<infer>")
    {
        return true;
    }
    if a == e {
        return true;
    }
    match (a, e) {
        (TypeExpr::Option(x), TypeExpr::Option(y)) => {
            x.as_ref() == &TypeExpr::Alias("<null>".into()) || assignable(x, y)
        }
        (TypeExpr::List(x), TypeExpr::List(y)) => assignable(x, y),
        (TypeExpr::Result(a_ok, a_error), TypeExpr::Result(e_ok, e_error)) => {
            assignable(a_ok, e_ok) && assignable(a_error, e_error)
        }
        (TypeExpr::Obj(a), TypeExpr::Obj(e)) => e
            .iter()
            .all(|(k, v)| a.get(k).is_some_and(|x| assignable(x, v))),
        (TypeExpr::Record(_), TypeExpr::Obj(_)) => false,
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

fn suggestion<'a>(value: &str, candidates: impl Iterator<Item = &'a String>) -> String {
    candidates
        .map(|candidate| (levenshtein(value, candidate), candidate))
        .min_by_key(|(distance, _)| *distance)
        .filter(|(distance, _)| *distance <= 2.max(value.len() / 3))
        .map(|(_, candidate)| format!("; did you mean '{candidate}'?"))
        .unwrap_or_default()
}

fn failure_fields() -> BTreeMap<String, TypeExpr> {
    BTreeMap::from([
        ("kind".into(), TypeExpr::String),
        ("message".into(), TypeExpr::String),
        (
            "operation".into(),
            TypeExpr::Option(Box::new(TypeExpr::String)),
        ),
        ("retryable".into(), TypeExpr::Bool),
    ])
}

fn levenshtein(left: &str, right: &str) -> usize {
    let mut previous: Vec<_> = (0..=right.chars().count()).collect();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_char) in right.chars().enumerate() {
            current.push(
                (current[right_index] + 1)
                    .min(previous[right_index + 1] + 1)
                    .min(previous[right_index] + usize::from(left_char != right_char)),
            );
        }
        previous = current;
    }
    previous.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_program;

    #[test]
    fn reports_unused_and_unreachable_code() {
        let program = parse_program(
            r#"
            pipeline main(input: String) -> String {
              let unused = helper(input);
              return input;
              return "unreachable";
            }
            pipeline helper(value: String) -> String { return value; }
            "#,
        )
        .unwrap();
        check_program(&program).unwrap();
        let warnings = analyze_program(&program);
        assert!(warnings.iter().any(|warning| warning.code == "AGLW2001"));
        assert!(warnings.iter().any(|warning| warning.code == "AGLW2002"));
    }

    #[test]
    fn suggests_close_names() {
        let program = parse_program(
            "pipeline main(value: Obj{message: String}) -> String { return value.mesage; }",
        )
        .unwrap();
        let error = check_program(&program).unwrap_err();
        assert!(error.message.contains("did you mean 'message'?"));
    }
}
