use crate::ast::*;

pub fn format_program(program: &Program) -> String {
    let mut sections = vec![format!("language {:?};", program.language_version)];
    for (alias, path) in &program.imports {
        sections.push(format!("import {alias} from {path:?};"));
    }
    let visibility = |name: &str| {
        if program.language_version != "0.6" {
            ""
        } else if program.public.contains(name) {
            "public "
        } else {
            "private "
        }
    };
    for (name, ty) in &program.aliases {
        sections.push(format!(
            "{}type {name} = {};",
            visibility(name),
            format_type(ty)
        ));
    }
    for (name, record) in &program.records {
        sections.push(format!(
            "{}record {name} {{ {} }};",
            visibility(name),
            record
                .fields
                .iter()
                .map(|(field, ty)| format!("{field}: {}", format_type(ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for (name, union) in &program.unions {
        sections.push(format!(
            "{}union {name} {{ {} }};",
            visibility(name),
            union
                .variants
                .iter()
                .map(|(variant, fields)| if fields.is_empty() {
                    variant.clone()
                } else {
                    format!(
                        "{variant} {{ {} }}",
                        fields
                            .iter()
                            .map(|(field, ty)| format!("{field}: {}", format_type(ty)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for (name, variants) in &program.enums {
        sections.push(format!(
            "{}enum {name} {{ {} }};",
            visibility(name),
            variants.join(", ")
        ));
    }
    for (name, tool) in &program.tools {
        sections.push(format_contract(
            visibility(name),
            "tool",
            name,
            &tool.params,
            &tool.return_type,
            &tool.effects,
            &tool.idempotency,
            tool.concurrency_group.as_deref(),
            tool.concurrency_limit,
            tool.rate_limit_per_second,
        ));
    }
    for (name, task) in &program.tasks {
        let mut value = format_contract(
            visibility(name),
            "task",
            name,
            &task.params,
            &task.return_type,
            &task.effects,
            &task.idempotency,
            task.concurrency_group.as_deref(),
            task.concurrency_limit,
            task.rate_limit_per_second,
        );
        if task.agent_task {
            value = value.replacen(" {}", " by agent {}", 1);
        }
        sections.push(value);
    }
    for (name, agent) in &program.agents {
        let mut fields = Vec::new();
        if let Some(model) = &agent.model {
            fields.push(format!("model: {model:?}"));
        }
        fields.push(format!("tools: [{}]", agent.tools.join(", ")));
        if !agent.requirements.capabilities.is_empty() {
            fields.push(format!(
                "requires: [{}]",
                agent
                    .requirements
                    .capabilities
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if let Some(value) = agent.requirements.min_context {
            fields.push(format!("min_context: {value}"));
        }
        if let Some(value) = agent.requirements.max_latency_ms {
            fields.push(format!("max_latency_ms: {value}"));
        }
        if let Some(value) = &agent.requirements.quality {
            fields.push(format!("quality: {value:?}"));
        }
        sections.push(format!(
            "{}agent {name} {{ {} }}",
            visibility(name),
            fields.join(", ")
        ));
    }
    for (name, pipeline) in &program.pipelines {
        sections.push(format!("{}{}", visibility(name), format_pipeline(pipeline)));
    }
    for (name, eval) in &program.evals {
        let mut fields = vec![
            format!("pipeline: {}", eval.pipeline),
            format!("dataset: {:?}", eval.dataset),
            format!("trials: {}", eval.trials),
            format!("assert_schema: {}", eval.assert_schema),
            format!("assert_expected: {}", eval.assert_expected),
        ];
        if let Some(value) = &eval.baseline {
            fields.push(format!("baseline: {value:?}"));
        }
        if let Some(value) = eval.max_latency_ms {
            fields.push(format!("max_latency_ms: {value}"));
        }
        if let Some(value) = eval.max_cost_usd {
            fields.push(format!("max_cost_usd: {value}"));
        }
        if let Some(value) = &eval.semantic_grader {
            fields.push(format!("semantic_grader: {value}"));
        }
        sections.push(format!(
            "{}eval {name} {{ {} }}",
            visibility(name),
            fields.join(", ")
        ));
    }
    for test in &program.tests {
        let mut lines = vec![format!("test {:?} {{", test.name)];
        statements(&test.statements, "  ", &mut lines);
        lines.push("}".into());
        sections.push(lines.join("\n"));
    }
    sections.join("\n\n") + "\n"
}

#[allow(clippy::too_many_arguments)]
fn format_contract(
    visibility: &str,
    kind: &str,
    name: &str,
    params: &[Param],
    result: &TypeExpr,
    effects: &std::collections::BTreeSet<String>,
    idempotency: &Idempotency,
    group: Option<&str>,
    limit: Option<u32>,
    rate: Option<u32>,
) -> String {
    let params = params
        .iter()
        .map(|param| format!("{}: {}", param.name, format_type(&param.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut clauses = String::new();
    if !effects.is_empty() {
        clauses.push_str(&format!(
            " effects [{}]",
            effects.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    match idempotency {
        Idempotency::Pure => clauses.push_str(" idempotency pure"),
        Idempotency::Idempotent => clauses.push_str(" idempotency idempotent"),
        Idempotency::KeyedBy(key) => clauses.push_str(&format!(" idempotency keyed_by {key}")),
        Idempotency::NonIdempotent => clauses.push_str(" idempotency non_idempotent"),
        Idempotency::Unspecified => {}
    }
    if let Some(group) = group {
        clauses.push_str(&format!(" concurrency_group {group}"));
    }
    if let Some(limit) = limit {
        clauses.push_str(&format!(" concurrency_limit {limit}"));
    }
    if let Some(rate) = rate {
        clauses.push_str(&format!(" rate_limit {rate}"));
    }
    format!(
        "{visibility}{kind} {name}({params}) -> {}{clauses} {{}}",
        format_type(result)
    )
}

pub fn format_pipeline(pipeline: &PipelineDef) -> String {
    let params = pipeline
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, format_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let effects = pipeline
        .effects
        .as_ref()
        .map(|effects| {
            format!(
                " effects [{}]",
                effects.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        })
        .unwrap_or_default();
    let budget = pipeline
        .budget
        .as_ref()
        .map(format_budget)
        .unwrap_or_default();
    let mut lines = vec![format!(
        "pipeline {}({params}) -> {}{effects}{budget} {{",
        pipeline.name,
        format_type(&pipeline.return_type)
    )];
    statements(&pipeline.statements, "  ", &mut lines);
    lines.push("}".into());
    lines.join("\n")
}

fn statements(items: &[Stmt], indent: &str, lines: &mut Vec<String>) {
    let child = format!("{indent}  ");
    for stmt in items {
        match stmt {
            Stmt::Run(run) => {
                let positional = run.args.keys().all(|name| name.starts_with("__pos_"));
                let args = run
                    .args
                    .iter()
                    .map(|(name, expr)| {
                        if positional {
                            format_expr(expr)
                        } else {
                            format!("{name}: {}", format_expr(expr))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut line = if positional {
                    format!("{indent}let {} = {}({args})", run.target, run.callable)
                } else {
                    format!(
                        "{indent}let {} = run {} with {{ {args} }}",
                        run.target, run.callable
                    )
                };
                if let Some(agent) = &run.agent {
                    line.push_str(&format!(" by {agent}"));
                }
                if run.retries > 0 {
                    line.push_str(&format!(" retries {}", run.retries));
                }
                if run.retry_policy != RetryPolicy::default() {
                    line.push_str(&format!(
                        " backoff {{ initial_ms: {}, max_ms: {}, multiplier: {}, jitter: {} }}",
                        run.retry_policy.initial_ms,
                        run.retry_policy.max_ms,
                        run.retry_policy.multiplier,
                        run.retry_policy.jitter
                    ));
                }
                if !run.retry_on.is_empty() {
                    line.push_str(&format!(
                        " retry_on [{}]",
                        run.retry_on
                            .iter()
                            .map(|(type_name, variant)| format!("{type_name}::{variant}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                match &run.on_fail {
                    OnFail::Abort if run.retries > 0 => line.push_str(" on_fail abort"),
                    OnFail::Abort => {}
                    OnFail::Use(expr) => {
                        line.push_str(&format!(" on_fail use {}", format_expr(expr)))
                    }
                }
                if let Some(timeout) = run.timeout {
                    line.push_str(&format!(" timeout {timeout}"));
                }
                line.push(';');
                lines.push(line);
            }
            Stmt::Approve {
                target,
                approval,
                prompt,
                expires_seconds,
                delegate,
                ..
            } => {
                let mut line = format!("{indent}let {target} = approve {approval} {:?}", prompt);
                if let Some(seconds) = expires_seconds {
                    line.push_str(&format!(" expires {seconds}"));
                }
                if let Some(delegate) = delegate {
                    line.push_str(&format!(" delegate {delegate}"));
                }
                line.push(';');
                lines.push(line);
            }
            Stmt::ParallelMap {
                target,
                binding,
                items,
                run,
                max_concurrency,
                failure_policy,
                ..
            } => {
                let policy = match failure_policy {
                    FailurePolicy::FailFast => "fail_fast",
                    FailurePolicy::CollectAll => "collect_all",
                };
                lines.push(format!("{indent}let {target} = parallel map {binding} in {} max_concurrency {max_concurrency} {policy} {{", format_expr(items)));
                statements(&[Stmt::Run(run.clone())], &child, lines);
                lines.push(format!("{indent}}};"));
            }
            Stmt::Race {
                target, branches, ..
            } => {
                lines.push(format!("{indent}let {target} = race {{"));
                statements(
                    &branches.iter().cloned().map(Stmt::Run).collect::<Vec<_>>(),
                    &child,
                    lines,
                );
                lines.push(format!("{indent}}};"));
            }
            Stmt::Parallel {
                branches,
                max_concurrency,
                ..
            } => {
                let suffix = max_concurrency
                    .map(|n| format!(" max_concurrency {n}"))
                    .unwrap_or_default();
                lines.push(format!("{indent}parallel{suffix} {{"));
                statements(
                    &branches.iter().cloned().map(Stmt::Run).collect::<Vec<_>>(),
                    &child,
                    lines,
                );
                lines.push(format!("{indent}}} join;"));
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                lines.push(format!("{indent}if {} {{", format_expr(condition)));
                statements(then_body, &child, lines);
                if else_body.is_empty() {
                    lines.push(format!("{indent}}}"));
                } else {
                    lines.push(format!("{indent}}} else {{"));
                    statements(else_body, &child, lines);
                    lines.push(format!("{indent}}}"));
                }
            }
            Stmt::IfLet {
                binding,
                option,
                then_body,
                else_body,
                ..
            } => {
                lines.push(format!(
                    "{indent}if let {binding} = {} {{",
                    format_expr(option)
                ));
                statements(then_body, &child, lines);
                if else_body.is_empty() {
                    lines.push(format!("{indent}}}"));
                } else {
                    lines.push(format!("{indent}}} else {{"));
                    statements(else_body, &child, lines);
                    lines.push(format!("{indent}}}"));
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                lines.push(format!("{indent}while {} {{", format_expr(condition)));
                statements(body, &child, lines);
                lines.push(format!("{indent}}}"));
            }
            Stmt::Break(_) => lines.push(format!("{indent}break;")),
            Stmt::Continue(_) => lines.push(format!("{indent}continue;")),
            Stmt::TryCatch {
                try_body,
                error_var,
                structured,
                catch_body,
                ..
            } => {
                lines.push(format!("{indent}try {{"));
                statements(try_body, &child, lines);
                lines.push(format!(
                    "{indent}}} catch {error_var}{} {{",
                    if *structured { ": Failure" } else { "" }
                ));
                statements(catch_body, &child, lines);
                lines.push(format!("{indent}}}"));
            }
            Stmt::Match { value, arms, .. } => {
                lines.push(format!("{indent}match {} {{", format_expr(value)));
                for arm in arms {
                    let bindings = if arm.bindings.is_empty() {
                        String::new()
                    } else {
                        format!(" {{ {} }}", arm.bindings.join(", "))
                    };
                    lines.push(format!(
                        "{child}{}::{}{bindings} => {{",
                        arm.type_name, arm.variant
                    ));
                    statements(&arm.body, &format!("{child}  "), lines);
                    lines.push(format!("{child}}}"));
                }
                lines.push(format!("{indent}}}"));
            }
            Stmt::Assert {
                condition, message, ..
            } => {
                let message = message
                    .as_ref()
                    .map(|m| format!(", {}", serde_json::to_string(m).unwrap()))
                    .unwrap_or_default();
                lines.push(format!(
                    "{indent}assert {}{message};",
                    format_expr(condition)
                ));
            }
            Stmt::Return { expr, .. } => {
                lines.push(format!("{indent}return {};", format_expr(expr)))
            }
        }
    }
}

fn format_budget(b: &ResourceBudget) -> String {
    let mut fields = Vec::new();
    if let Some(v) = b.time_ms {
        fields.push(format!("time_ms: {v}"));
    }
    if let Some(v) = b.tokens {
        fields.push(format!("tokens: {v}"));
    }
    if let Some(v) = b.cost_usd {
        fields.push(format!("cost_usd: {v}"));
    }
    if let Some(v) = b.tool_calls {
        fields.push(format!("tool_calls: {v}"));
    }
    if let Some(v) = b.retries {
        fields.push(format!("retries: {v}"));
    }
    if let Some(v) = b.concurrency {
        fields.push(format!("concurrency: {v}"));
    }
    format!(" budget {{ {} }}", fields.join(", "))
}

fn format_type(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::String => "String".into(),
        TypeExpr::Number => "Number".into(),
        TypeExpr::Bool => "Bool".into(),
        TypeExpr::Failure => "Failure".into(),
        TypeExpr::List(item) => format!("List[{}]", format_type(item)),
        TypeExpr::Option(item) => format!("Option[{}]", format_type(item)),
        TypeExpr::Result(ok, error) => {
            format!("Result[{}, {}]", format_type(ok), format_type(error))
        }
        TypeExpr::Obj(fields) => format!(
            "Obj{{{}}}",
            fields
                .iter()
                .map(|(name, ty)| format!("{name}: {}", format_type(ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Record(name)
        | TypeExpr::Union(name)
        | TypeExpr::Enum(name)
        | TypeExpr::Alias(name) => name.clone(),
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal { value, .. } => serde_json::to_string(value).unwrap(),
        Expr::Ref { parts, .. } => parts.join("."),
        Expr::Binary {
            op, left, right, ..
        } => format!(
            "{} {} {}",
            format_expr(left),
            match op {
                BinaryOp::Add => "+",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
            },
            format_expr(right)
        ),
        Expr::Obj { fields, .. } => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(name, value)| format!("{name}: {}", format_expr(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Record { name, fields, .. } => format!(
            "{name} {{ {} }}",
            fields
                .iter()
                .map(|(field, value)| format!("{field}: {}", format_expr(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Variant {
            type_name,
            variant,
            fields,
            ..
        } => {
            let fields = if fields.is_empty() {
                String::new()
            } else {
                format!(
                    " {{ {} }}",
                    fields
                        .iter()
                        .map(|(field, value)| format!("{field}: {}", format_expr(value)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!("{type_name}::{variant}{fields}")
        }
        Expr::Result { ok, value, .. } => {
            format!("{}({})", if *ok { "Ok" } else { "Err" }, format_expr(value))
        }
        Expr::List { items, .. } => format!(
            "[{}]",
            items.iter().map(format_expr).collect::<Vec<_>>().join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_program;

    #[test]
    fn formats_lowered_workflow_as_pipeline() {
        let source = std::fs::read_to_string("examples/newsletter.agent").unwrap();
        let program = parse_program(&source).unwrap();
        let output = format_pipeline(&program.pipelines["weekly_newsletter"]);
        assert!(output.starts_with("pipeline weekly_newsletter"));
        assert!(output.contains("run find_news"));
        assert!(!output.contains("stage "));
    }
}
