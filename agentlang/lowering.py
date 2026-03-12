from __future__ import annotations

from dataclasses import replace

from .ast import (
    BinaryExpr,
    BreakStmt,
    Expr,
    IfStmt,
    ListExpr,
    LiteralExpr,
    ObjExpr,
    ObjType,
    ParallelStmt,
    Param,
    PipelineDef,
    PrimitiveType,
    Program,
    RefExpr,
    ReturnStmt,
    RunStmt,
    Stmt,
    TaskDef,
    WorkflowDef,
    WorkflowReturnStep,
    WorkflowReviewStep,
    WorkflowStageStep,
    WhileStmt,
)


class LoweringError(ValueError):
    pass


def lower_program(program: Program) -> Program:
    if not program.workflows:
        return program

    tasks = dict(program.tasks)
    if any(isinstance(step, WorkflowReviewStep) for workflow in program.workflows.values() for step in workflow.steps):
        _ensure_countdown_task(tasks)

    pipelines = dict(program.pipelines)
    for workflow in program.workflows.values():
        if workflow.name in pipelines:
            raise LoweringError(f"Workflow '{workflow.name}' conflicts with an existing pipeline.")
        pipelines[workflow.name] = _lower_workflow(program, workflow)

    return replace(program, tasks=tasks, pipelines=pipelines)


def format_pipeline(pipeline: PipelineDef) -> str:
    lines = [
        f"pipeline {pipeline.name}({', '.join(_format_param(param) for param in pipeline.params)}) -> "
        f"{_format_type(pipeline.return_type)} {{"
    ]
    lines.extend(_format_statements(pipeline.statements, indent="  "))
    lines.append("}")
    return "\n".join(lines)


def _lower_workflow(program: Program, workflow: WorkflowDef) -> PipelineDef:
    alias_bindings = {param.name: param.name for param in workflow.params}
    alias_types = {param.name: param.type_expr for param in workflow.params}
    consumed: set[str] = set()
    statements: list[Stmt] = []
    temp_index = 0
    saw_return = False

    for step in workflow.steps:
        if isinstance(step, WorkflowStageStep):
            task = program.tasks.get(step.task_name)
            if task is None:
                raise LoweringError(
                    f"Workflow '{workflow.name}' references unknown task '{step.task_name}'."
                )
            if step.agent_name not in program.agents:
                raise LoweringError(
                    f"Workflow '{workflow.name}' references unknown agent '{step.agent_name}'."
                )
            args = _bind_positional_args(workflow, task, step.args, alias_bindings, consumed)
            statements.append(
                RunStmt(
                    target=step.target,
                    task_name=task.name,
                    args=args,
                    agent_name=step.agent_name,
                    retries=0,
                    on_fail="abort",
                    fallback_expr=None,
                )
            )
            alias_bindings[step.target] = step.target
            alias_types[step.target] = task.return_type
            consumed.discard(step.target)
            continue

        if isinstance(step, WorkflowReviewStep):
            source_var = _resolve_alias(step.source, alias_bindings, consumed, workflow.name)
            source_type = alias_types.get(step.source)
            if not isinstance(source_type, ObjType):
                raise LoweringError(
                    f"Workflow '{workflow.name}' can only review object-shaped artifacts; "
                    f"'{step.source}' has type {source_type}."
                )

            if step.reviewer_agent not in program.agents:
                raise LoweringError(
                    f"Workflow '{workflow.name}' references unknown agent '{step.reviewer_agent}'."
                )
            if step.reviser_agent not in program.agents:
                raise LoweringError(
                    f"Workflow '{workflow.name}' references unknown agent '{step.reviser_agent}'."
                )

            review_task_name = f"review_{step.target}"
            review_task = program.tasks.get(review_task_name)
            if review_task is None:
                raise LoweringError(
                    f"Workflow '{workflow.name}' could not infer review task '{review_task_name}'."
                )
            revise_task = program.tasks.get(step.revise_task_name)
            if revise_task is None:
                raise LoweringError(
                    f"Workflow '{workflow.name}' references unknown revise task '{step.revise_task_name}'."
                )
            if revise_task.return_type != source_type:
                raise LoweringError(
                    f"Workflow '{workflow.name}' requires revise task '{revise_task.name}' "
                    f"to return the same type as '{step.source}'."
                )

            review_var = f"__{step.target}_review"
            remaining_var = f"__{step.target}_remaining"
            alias_bindings[review_var] = review_var
            alias_bindings[remaining_var] = remaining_var
            review_args = _auto_bind_task_args(
                workflow,
                task=review_task,
                alias_bindings=alias_bindings,
                consumed=consumed,
                sources=[
                    _workflow_param_bindings(workflow),
                    _object_field_bindings(step.source, source_type),
                ],
            )
            statements.append(
                RunStmt(
                    target=review_var,
                    task_name=review_task.name,
                    args=review_args,
                    agent_name=step.reviewer_agent,
                    retries=0,
                    on_fail="abort",
                    fallback_expr=None,
                )
            )
            statements.append(
                RunStmt(
                    target=remaining_var,
                    task_name="countdown",
                    args={"current": LiteralExpr(value=step.max_rounds + 1)},
                    agent_name=None,
                    retries=0,
                    on_fail="abort",
                    fallback_expr=None,
                )
            )

            loop_body: list[Stmt] = [
                IfStmt(
                    condition=RefExpr(parts=[remaining_var, "done"]),
                    then_statements=[BreakStmt()],
                    else_statements=None,
                )
            ]

            revise_args = _auto_bind_task_args(
                workflow,
                task=revise_task,
                alias_bindings=alias_bindings,
                consumed=consumed,
                sources=[
                    _workflow_param_bindings(workflow),
                    _object_field_bindings(step.source, source_type),
                    {
                        "approved": RefExpr(parts=[review_var, "approved"]),
                        "feedback": RefExpr(parts=[review_var, "feedback"]),
                    },
                ],
            )
            loop_body.append(
                RunStmt(
                    target=source_var,
                    task_name=revise_task.name,
                    args=revise_args,
                    agent_name=step.reviser_agent,
                    retries=0,
                    on_fail="abort",
                    fallback_expr=None,
                )
            )
            loop_body.append(
                RunStmt(
                    target=review_var,
                    task_name=review_task.name,
                    args=_auto_bind_task_args(
                        workflow,
                        task=review_task,
                        alias_bindings=alias_bindings,
                        consumed=consumed,
                        sources=[
                            _workflow_param_bindings(workflow),
                            _object_field_bindings(step.source, source_type),
                        ],
                    ),
                    agent_name=step.reviewer_agent,
                    retries=0,
                    on_fail="abort",
                    fallback_expr=None,
                )
            )
            loop_body.append(
                RunStmt(
                    target=remaining_var,
                    task_name="countdown",
                    args={"current": RefExpr(parts=[remaining_var, "next"])},
                    agent_name=None,
                    retries=0,
                    on_fail="abort",
                    fallback_expr=None,
                )
            )
            statements.append(
                WhileStmt(
                    condition=BinaryExpr(
                        op="==",
                        left=RefExpr(parts=[review_var, "approved"]),
                        right=LiteralExpr(value=False),
                    ),
                    statements=loop_body,
                )
            )

            consumed.add(step.source)
            alias_bindings.pop(step.source, None)
            alias_types.pop(step.source, None)
            alias_bindings[step.target] = source_var
            alias_types[step.target] = source_type
            continue

        if isinstance(step, WorkflowReturnStep):
            statements.append(
                ReturnStmt(
                    expr=_rewrite_expr(step.expr, alias_bindings, consumed, workflow.name)
                )
            )
            saw_return = True
            continue

        raise LoweringError(f"Unsupported workflow step '{type(step).__name__}'.")

    if not saw_return:
        raise LoweringError(f"Workflow '{workflow.name}' is missing a return statement.")

    return PipelineDef(
        name=workflow.name,
        params=workflow.params,
        return_type=workflow.return_type,
        statements=statements,
    )


def _workflow_param_bindings(workflow: WorkflowDef) -> dict[str, Expr]:
    return {param.name: RefExpr(parts=[param.name]) for param in workflow.params}


def _object_field_bindings(var_name: str, obj_type: ObjType) -> dict[str, Expr]:
    return {field: RefExpr(parts=[var_name, field]) for field in obj_type.fields}


def _bind_positional_args(
    workflow: WorkflowDef,
    task: TaskDef,
    raw_args: list[Expr],
    alias_bindings: dict[str, str],
    consumed: set[str],
) -> dict[str, Expr]:
    if len(raw_args) != len(task.params):
        raise LoweringError(
            f"Workflow '{workflow.name}' stage calling '{task.name}' expected "
            f"{len(task.params)} args, got {len(raw_args)}."
        )
    return {
        param.name: _rewrite_expr(expr, alias_bindings, consumed, workflow.name)
        for param, expr in zip(task.params, raw_args, strict=True)
    }


def _auto_bind_task_args(
    workflow: WorkflowDef,
    task: TaskDef,
    alias_bindings: dict[str, str],
    consumed: set[str],
    sources: list[dict[str, Expr]],
) -> dict[str, Expr]:
    resolved: dict[str, Expr] = {}
    available: dict[str, Expr] = {}
    for source in sources:
        available.update(
            {
                name: _rewrite_expr(expr, alias_bindings, consumed, workflow.name)
                for name, expr in source.items()
            }
        )
    for param in task.params:
        expr = available.get(param.name)
        if expr is None:
            raise LoweringError(
                f"Workflow '{workflow.name}' could not auto-bind parameter '{param.name}' "
                f"for task '{task.name}'."
            )
        resolved[param.name] = expr
    return resolved


def _rewrite_expr(
    expr: Expr,
    alias_bindings: dict[str, str],
    consumed: set[str],
    workflow_name: str,
) -> Expr:
    if isinstance(expr, LiteralExpr):
        return expr

    if isinstance(expr, RefExpr):
        head = expr.parts[0]
        resolved = _resolve_alias(head, alias_bindings, consumed, workflow_name)
        return RefExpr(parts=[resolved, *expr.parts[1:]])

    if isinstance(expr, BinaryExpr):
        return BinaryExpr(
            op=expr.op,
            left=_rewrite_expr(expr.left, alias_bindings, consumed, workflow_name),
            right=_rewrite_expr(expr.right, alias_bindings, consumed, workflow_name),
        )

    if isinstance(expr, ObjExpr):
        return ObjExpr(
            fields={
                key: _rewrite_expr(value, alias_bindings, consumed, workflow_name)
                for key, value in expr.fields.items()
            }
        )

    if isinstance(expr, ListExpr):
        return ListExpr(
            items=[
                _rewrite_expr(item, alias_bindings, consumed, workflow_name)
                for item in expr.items
            ]
        )

    raise LoweringError(f"Unsupported workflow expression '{type(expr).__name__}'.")


def _resolve_alias(
    name: str,
    alias_bindings: dict[str, str],
    consumed: set[str],
    workflow_name: str,
) -> str:
    if name in consumed:
        raise LoweringError(
            f"Workflow '{workflow_name}' references consumed artifact '{name}'."
        )
    resolved = alias_bindings.get(name)
    if resolved is None:
        raise LoweringError(f"Workflow '{workflow_name}' references unknown artifact '{name}'.")
    return resolved


def _ensure_countdown_task(tasks: dict[str, TaskDef]) -> None:
    countdown = tasks.get("countdown")
    expected = TaskDef(
        name="countdown",
        params=[Param(name="current", type_expr=PrimitiveType("Number"))],
        return_type=ObjType(
            fields={
                "next": PrimitiveType("Number"),
                "done": PrimitiveType("Bool"),
            }
        ),
    )
    if countdown is None:
        tasks["countdown"] = expected
        return
    if countdown != expected:
        raise LoweringError(
            "Workflow lowering requires task 'countdown(current: Number) -> "
            "Obj{next: Number, done: Bool}'."
        )


def _format_param(param: Param) -> str:
    return f"{param.name}: {_format_type(param.type_expr)}"


def _format_type(type_expr: object) -> str:
    if isinstance(type_expr, PrimitiveType):
        return type_expr.name
    if isinstance(type_expr, ObjType):
        inner = ", ".join(f"{name}: {_format_type(field_type)}" for name, field_type in type_expr.fields.items())
        return f"Obj{{{inner}}}"
    from .ast import ListType, OptionType

    if isinstance(type_expr, ListType):
        return f"List[{_format_type(type_expr.item_type)}]"
    if isinstance(type_expr, OptionType):
        return f"Option[{_format_type(type_expr.item_type)}]"
    raise LoweringError(f"Unsupported type formatter input: {type_expr!r}")


def _format_statements(statements: list[Stmt], indent: str) -> list[str]:
    lines: list[str] = []
    child_indent = indent + "  "
    for stmt in statements:
        if isinstance(stmt, RunStmt):
            args = ", ".join(f"{name}: {_format_expr(expr)}" for name, expr in stmt.args.items())
            line = f"{indent}let {stmt.target} = run {stmt.task_name} with {{ {args} }}"
            if stmt.agent_name is not None:
                line += f" by {stmt.agent_name}"
            line += ";"
            lines.append(line)
            continue

        if isinstance(stmt, WhileStmt):
            lines.append(f"{indent}while {_format_expr(stmt.condition)} {{")
            lines.extend(_format_statements(stmt.statements, child_indent))
            lines.append(f"{indent}}}")
            continue

        if isinstance(stmt, IfStmt):
            lines.append(f"{indent}if {_format_expr(stmt.condition)} {{")
            lines.extend(_format_statements(stmt.then_statements, child_indent))
            lines.append(f"{indent}}}")
            continue

        if isinstance(stmt, ReturnStmt):
            lines.append(f"{indent}return {_format_expr(stmt.expr)};")
            continue

        if isinstance(stmt, BreakStmt):
            lines.append(f"{indent}break;")
            continue

        if isinstance(stmt, ParallelStmt):
            lines.append(f"{indent}parallel {{")
            lines.extend(_format_statements(stmt.branches, child_indent))
            lines.append(f"{indent}}} join;")
            continue

        raise LoweringError(f"Unsupported statement formatter input: {type(stmt).__name__}")

    return lines


def _format_expr(expr: Expr) -> str:
    if isinstance(expr, LiteralExpr):
        if isinstance(expr.value, str):
            return repr(expr.value).replace("'", '"')
        if expr.value is True:
            return "true"
        if expr.value is False:
            return "false"
        if expr.value is None:
            return "null"
        return str(expr.value)

    if isinstance(expr, RefExpr):
        return ".".join(expr.parts)

    if isinstance(expr, BinaryExpr):
        return f"{_format_expr(expr.left)} {expr.op} {_format_expr(expr.right)}"

    if isinstance(expr, ObjExpr):
        inner = ", ".join(f"{name}: {_format_expr(value)}" for name, value in expr.fields.items())
        return f"{{ {inner} }}"

    if isinstance(expr, ListExpr):
        return f"[{', '.join(_format_expr(item) for item in expr.items)}]"

    raise LoweringError(f"Unsupported expression formatter input: {type(expr).__name__}")
