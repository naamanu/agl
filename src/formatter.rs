use crate::ast::*;

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
    let mut lines = vec![format!(
        "pipeline {}({params}) -> {}{effects} {{",
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
                let args = run
                    .args
                    .iter()
                    .map(|(name, expr)| format!("{name}: {}", format_expr(expr)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut line = format!(
                    "{indent}let {} = run {} with {{ {args} }}",
                    run.target, run.callable
                );
                if let Some(agent) = &run.agent {
                    line.push_str(&format!(" by {agent}"));
                }
                if run.retries > 0 {
                    line.push_str(&format!(" retries {}", run.retries));
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
