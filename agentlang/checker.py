from __future__ import annotations

from dataclasses import dataclass

from .ast import (
    AssertStmt,
    BinaryExpr,
    BreakStmt,
    ContinueStmt,
    EnumType,
    Expr,
    IfLetStmt,
    IfStmt,
    ListExpr,
    ListType,
    LiteralExpr,
    ObjExpr,
    ObjType,
    OptionType,
    ParallelStmt,
    PipelineDef,
    PrimitiveType,
    Program,
    RefExpr,
    ReturnStmt,
    RunStmt,
    Span,
    Stmt,
    TryCatchStmt,
    TypeExpr,
    WhileStmt,
)


class TypeCheckError(ValueError):
    pass


def _span_prefix(span: Span | None) -> str:
    if span is None:
        return ""
    return f"at {span.line}:{span.col}: "


@dataclass(frozen=True)
class _NullType(TypeExpr):
    pass


def check_program(program: Program) -> None:
    _check_tools(program)
    _check_enums(program)
    for pipeline in program.pipelines.values():
        _check_pipeline(program, pipeline)
    for test_block in program.test_blocks:
        _check_test_block(program, test_block)


def _check_tools(program: Program) -> None:
    declared_tools = set(program.tools)
    for agent in program.agents.values():
        for tool_name in agent.tools:
            if tool_name not in declared_tools:
                raise TypeCheckError(
                    f"Agent '{agent.name}' references unknown tool '{tool_name}'."
                )


def _check_enums(program: Program) -> None:
    for enum_def in program.enum_types.values():
        if len(enum_def.variants) == 0:
            raise TypeCheckError(f"Enum '{enum_def.name}' must have at least one variant.")


def _check_test_block(program: Program, test_block) -> None:
    env: dict[str, TypeExpr] = {}
    # Create a synthetic pipeline for type checking purposes
    synthetic = PipelineDef(
        name=f"__test_{test_block.name}",
        params=(),
        return_type=PrimitiveType("String"),  # Doesn't matter, tests don't require return
        statements=test_block.statements,
    )
    _check_block(program, synthetic, test_block.statements, env, require_return=False)


def _check_pipeline(program: Program, pipeline: PipelineDef) -> None:
    env: dict[str, TypeExpr] = {param.name: param.type_expr for param in pipeline.params}
    saw_return = _check_block(program, pipeline, pipeline.statements, env)
    if not saw_return:
        raise TypeCheckError(f"Pipeline '{pipeline.name}' is missing a return statement.")


def _check_block(
    program: Program,
    pipeline: PipelineDef,
    statements: tuple[Stmt, ...] | list[Stmt],
    env: dict[str, TypeExpr],
    in_loop: bool = False,
    require_return: bool = True,
) -> bool:
    saw_return = False
    for stmt in statements:
        if isinstance(stmt, RunStmt):
            env[stmt.target] = _check_run_stmt(program, stmt, env)
            continue

        if isinstance(stmt, ParallelStmt):
            new_bindings: dict[str, TypeExpr] = {}
            for branch in stmt.branches:
                if branch.target in new_bindings:
                    raise TypeCheckError(
                        f"{_span_prefix(branch.span)}Duplicate target '{branch.target}' inside parallel block."
                    )
                if branch.target in env:
                    raise TypeCheckError(
                        f"{_span_prefix(branch.span)}Parallel target '{branch.target}' shadows an existing variable."
                    )
                new_bindings[branch.target] = _check_run_stmt(program, branch, env)
            env.update(new_bindings)
            continue

        if isinstance(stmt, IfStmt):
            cond_type = _infer_expr_type(stmt.condition, env, program)
            if not _is_assignable(cond_type, PrimitiveType("Bool")):
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}If condition must be Bool, got {cond_type} in pipeline '{pipeline.name}'."
                )
            then_env = dict(env)
            then_return = _check_block(
                program,
                pipeline,
                stmt.then_statements,
                then_env,
                in_loop=in_loop,
            )
            if stmt.else_statements is None:
                merged = _common_bindings(env, then_env, env)
                env.clear()
                env.update(merged)
                continue

            else_env = dict(env)
            else_return = _check_block(
                program,
                pipeline,
                stmt.else_statements,
                else_env,
                in_loop=in_loop,
            )
            merged = _common_bindings(env, then_env, else_env)
            env.clear()
            env.update(merged)
            saw_return = saw_return or (then_return and else_return)
            continue

        if isinstance(stmt, IfLetStmt):
            option_type = _infer_expr_type(stmt.option_expr, env, program)
            if not isinstance(option_type, OptionType):
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}If-let expression must have Option type, got {option_type} "
                    f"in pipeline '{pipeline.name}'."
                )
            if stmt.binding in env:
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}If-let binding '{stmt.binding}' shadows an existing variable."
                )

            then_env = dict(env)
            then_env[stmt.binding] = option_type.item_type
            then_return = _check_block(
                program,
                pipeline,
                stmt.then_statements,
                then_env,
                in_loop=in_loop,
            )
            if stmt.else_statements is None:
                merged = _common_bindings(env, then_env, env)
                env.clear()
                env.update(merged)
                continue

            else_env = dict(env)
            else_return = _check_block(
                program,
                pipeline,
                stmt.else_statements,
                else_env,
                in_loop=in_loop,
            )
            merged = _common_bindings(env, then_env, else_env)
            env.clear()
            env.update(merged)
            saw_return = saw_return or (then_return and else_return)
            continue

        if isinstance(stmt, WhileStmt):
            cond_type = _infer_expr_type(stmt.condition, env, program)
            if not _is_assignable(cond_type, PrimitiveType("Bool")):
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}While condition must be Bool, got {cond_type} in pipeline '{pipeline.name}'."
                )
            loop_env = dict(env)
            _check_block(
                program,
                pipeline,
                stmt.statements,
                loop_env,
                in_loop=True,
            )
            merged = _common_bindings(env, loop_env, env)
            env.clear()
            env.update(merged)
            continue

        if isinstance(stmt, TryCatchStmt):
            try_env = dict(env)
            try_return = _check_block(
                program, pipeline, stmt.try_body, try_env, in_loop=in_loop,
            )
            catch_env = dict(env)
            catch_env[stmt.error_var] = PrimitiveType("String")
            catch_return = _check_block(
                program, pipeline, stmt.catch_body, catch_env, in_loop=in_loop,
            )
            merged = _common_bindings(env, try_env, catch_env)
            env.clear()
            env.update(merged)
            saw_return = saw_return or (try_return and catch_return)
            continue

        if isinstance(stmt, AssertStmt):
            cond_type = _infer_expr_type(stmt.condition, env, program)
            if not _is_assignable(cond_type, PrimitiveType("Bool")):
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}Assert condition must be Bool, got {cond_type}."
                )
            continue

        if isinstance(stmt, BreakStmt):
            if not in_loop:
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}'break' is only valid inside while loops in pipeline '{pipeline.name}'."
                )
            continue

        if isinstance(stmt, ContinueStmt):
            if not in_loop:
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}'continue' is only valid inside while loops in pipeline '{pipeline.name}'."
                )
            continue

        if isinstance(stmt, ReturnStmt):
            actual = _infer_expr_type(stmt.expr, env, program)
            if not _is_assignable(actual, pipeline.return_type):
                raise TypeCheckError(
                    f"{_span_prefix(stmt.span)}Pipeline '{pipeline.name}' returns {actual}, expected {pipeline.return_type}."
                )
            saw_return = True
            continue

        raise TypeCheckError(f"Unsupported statement: {type(stmt).__name__}")

    return saw_return


def _common_bindings(
    base_env: dict[str, TypeExpr],
    then_env: dict[str, TypeExpr],
    else_env: dict[str, TypeExpr],
) -> dict[str, TypeExpr]:
    merged: dict[str, TypeExpr] = dict(base_env)
    shared_names = set(then_env) & set(else_env)
    for name in shared_names:
        then_type = then_env[name]
        else_type = else_env[name]
        if _is_assignable(then_type, else_type) and _is_assignable(else_type, then_type):
            merged[name] = then_type
        elif name in base_env:
            del merged[name]
    return merged


def _check_run_stmt(program: Program, stmt: RunStmt, env: dict[str, TypeExpr]) -> TypeExpr:
    # Check if target is a pipeline (pipeline-calls-pipeline)
    task = program.tasks.get(stmt.task_name)
    pipeline_target = program.pipelines.get(stmt.task_name)

    if task is None and pipeline_target is None:
        raise TypeCheckError(f"{_span_prefix(stmt.span)}Unknown task '{stmt.task_name}'.")

    if task is not None:
        return _check_run_stmt_task(program, stmt, env, task)
    else:
        assert pipeline_target is not None
        return _check_run_stmt_pipeline(program, stmt, env, pipeline_target)


def _check_run_stmt_task(program, stmt, env, task):
    if stmt.agent_name is not None and stmt.agent_name not in program.agents:
        raise TypeCheckError(f"{_span_prefix(stmt.span)}Unknown agent '{stmt.agent_name}'.")

    if task.execution_mode == "agent" and stmt.agent_name is None:
        raise TypeCheckError(
            f"{_span_prefix(stmt.span)}Agent task '{task.name}' must be run with an explicit agent binding."
        )

    if stmt.retries < 0:
        raise TypeCheckError(f"{_span_prefix(stmt.span)}Retries cannot be negative.")

    if stmt.on_fail not in {"abort", "use"}:
        raise TypeCheckError(f"{_span_prefix(stmt.span)}Unsupported on_fail policy '{stmt.on_fail}'.")

    if stmt.on_fail == "abort" and stmt.fallback_expr is not None:
        raise TypeCheckError(f"{_span_prefix(stmt.span)}Fallback expression is only valid with on_fail use.")

    if stmt.on_fail == "use":
        if stmt.fallback_expr is None:
            raise TypeCheckError(f"{_span_prefix(stmt.span)}on_fail use requires a fallback expression.")
        fallback_type = _infer_expr_type(stmt.fallback_expr, env, program)
        if not _is_assignable(fallback_type, task.return_type):
            raise TypeCheckError(
                f"{_span_prefix(stmt.span)}Fallback type {fallback_type} does not match task '{task.name}' return "
                f"type {task.return_type}."
            )

    expected_params = {param.name: param.type_expr for param in task.params}
    provided_params = set(stmt.args)

    missing = set(expected_params) - provided_params
    extra = provided_params - set(expected_params)

    if missing:
        raise TypeCheckError(f"{_span_prefix(stmt.span)}Task '{task.name}' missing args: {sorted(missing)}")
    if extra:
        raise TypeCheckError(f"{_span_prefix(stmt.span)}Task '{task.name}' received unknown args: {sorted(extra)}")

    for param_name, expected_type in expected_params.items():
        actual_type = _infer_expr_type(stmt.args[param_name], env, program)
        if not _is_assignable(actual_type, expected_type):
            raise TypeCheckError(
                f"{_span_prefix(stmt.span)}Task '{task.name}' arg '{param_name}' has type {actual_type}, "
                f"expected {expected_type}."
            )

    return task.return_type


def _check_run_stmt_pipeline(program, stmt, env, pipeline_target):
    # Pipeline-calls-pipeline: validate args against pipeline params
    if stmt.agent_name is not None:
        raise TypeCheckError(
            f"{_span_prefix(stmt.span)}Cannot use 'by agent' when calling pipeline '{pipeline_target.name}'."
        )

    expected_params = {param.name: param.type_expr for param in pipeline_target.params}
    provided_params = set(stmt.args)

    missing = set(expected_params) - provided_params
    extra = provided_params - set(expected_params)

    if missing:
        raise TypeCheckError(
            f"{_span_prefix(stmt.span)}Pipeline '{pipeline_target.name}' missing args: {sorted(missing)}"
        )
    if extra:
        raise TypeCheckError(
            f"{_span_prefix(stmt.span)}Pipeline '{pipeline_target.name}' received unknown args: {sorted(extra)}"
        )

    for param_name, expected_type in expected_params.items():
        actual_type = _infer_expr_type(stmt.args[param_name], env, program)
        if not _is_assignable(actual_type, expected_type):
            raise TypeCheckError(
                f"{_span_prefix(stmt.span)}Pipeline '{pipeline_target.name}' arg '{param_name}' has type "
                f"{actual_type}, expected {expected_type}."
            )

    return pipeline_target.return_type


def _resolve_type(type_expr: TypeExpr, program: Program) -> TypeExpr:
    """Resolve EnumType to PrimitiveType(String) for assignability checks."""
    if isinstance(type_expr, EnumType):
        return PrimitiveType("String")
    return type_expr


def _infer_expr_type(expr: Expr, env: dict[str, TypeExpr], program: Program | None = None) -> TypeExpr:
    if isinstance(expr, LiteralExpr):
        if expr.value is None:
            return _NullType()
        if isinstance(expr.value, bool):
            return PrimitiveType("Bool")
        if isinstance(expr.value, int) or isinstance(expr.value, float):
            return PrimitiveType("Number")
        if isinstance(expr.value, str):
            # Check if this string matches an enum variant
            if program is not None:
                for enum_def in program.enum_types.values():
                    if expr.value in enum_def.variants:
                        return EnumType(name=enum_def.name)
            return PrimitiveType("String")
        raise TypeCheckError(f"{_span_prefix(expr.span)}Unsupported literal value: {expr.value!r}")

    if isinstance(expr, RefExpr):
        root = expr.parts[0]
        current = env.get(root)
        if current is None:
            raise TypeCheckError(f"{_span_prefix(expr.span)}Unknown variable '{root}'.")
        for field in expr.parts[1:]:
            resolved = _resolve_type(current, program) if program else current
            if not isinstance(resolved, ObjType):
                raise TypeCheckError(f"{_span_prefix(expr.span)}Cannot access field '{field}' on non-object type {current}.")
            if field not in resolved.fields:
                raise TypeCheckError(f"{_span_prefix(expr.span)}Field '{field}' does not exist on type {current}.")
            current = resolved.fields[field]
        return current

    if isinstance(expr, ObjExpr):
        return ObjType({name: _infer_expr_type(value, env, program) for name, value in expr.fields.items()})

    if isinstance(expr, ListExpr):
        if not expr.items:
            raise TypeCheckError(f"{_span_prefix(expr.span)}Cannot infer type of empty list literal; provide at least one item.")
        first_type = _infer_expr_type(expr.items[0], env, program)
        for item in expr.items[1:]:
            item_type = _infer_expr_type(item, env, program)
            if not (_is_assignable(item_type, first_type) and _is_assignable(first_type, item_type)):
                raise TypeCheckError(f"{_span_prefix(expr.span)}List literal items must all share the same type.")
        return ListType(first_type)

    if isinstance(expr, BinaryExpr):
        left = _infer_expr_type(expr.left, env, program)
        right = _infer_expr_type(expr.right, env, program)
        if expr.op == "+":
            left_r = _resolve_type(left, program) if program else left
            right_r = _resolve_type(right, program) if program else right
            if isinstance(left_r, PrimitiveType) and isinstance(right_r, PrimitiveType):
                if left_r.name == right_r.name and left_r.name in {"String", "Number"}:
                    return left_r
            raise TypeCheckError(f"{_span_prefix(expr.span)}Cannot apply + to {left} and {right}.")
        if expr.op in {"==", "!="}:
            if _can_compare(left, right):
                return PrimitiveType("Bool")
            raise TypeCheckError(f"{_span_prefix(expr.span)}Cannot compare values of types {left} and {right}.")
        raise TypeCheckError(f"{_span_prefix(expr.span)}Unsupported binary operator '{expr.op}'.")

    raise TypeCheckError(f"Unsupported expression: {type(expr).__name__}")


def _is_assignable(actual: TypeExpr, expected: TypeExpr) -> bool:
    if isinstance(actual, _NullType):
        return isinstance(expected, (OptionType, _NullType))

    if isinstance(actual, OptionType) and isinstance(expected, OptionType):
        return _is_assignable(actual.item_type, expected.item_type)

    if isinstance(actual, PrimitiveType) and isinstance(expected, PrimitiveType):
        return actual.name == expected.name

    if isinstance(actual, ListType) and isinstance(expected, ListType):
        return _is_assignable(actual.item_type, expected.item_type)

    if isinstance(actual, ObjType) and isinstance(expected, ObjType):
        if not set(expected.fields) <= set(actual.fields):
            return False
        return all(
            _is_assignable(actual.fields[field], expected.fields[field])
            for field in expected.fields
        )

    # EnumType is assignable to String, and String is assignable to EnumType
    if isinstance(actual, EnumType) and isinstance(expected, PrimitiveType) and expected.name == "String":
        return True
    if isinstance(actual, PrimitiveType) and actual.name == "String" and isinstance(expected, EnumType):
        return True
    if isinstance(actual, EnumType) and isinstance(expected, EnumType):
        return actual.name == expected.name

    return False


def _can_compare(left: TypeExpr, right: TypeExpr) -> bool:
    if _is_assignable(left, right) and _is_assignable(right, left):
        return True
    if isinstance(left, _NullType) and isinstance(right, OptionType):
        return True
    if isinstance(right, _NullType) and isinstance(left, OptionType):
        return True
    # Allow enum comparisons with strings
    if isinstance(left, EnumType) and isinstance(right, PrimitiveType) and right.name == "String":
        return True
    if isinstance(right, EnumType) and isinstance(left, PrimitiveType) and left.name == "String":
        return True
    return False
