from __future__ import annotations

from dataclasses import dataclass

from .ast import (
    BinaryExpr,
    BreakStmt,
    ContinueStmt,
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
    Stmt,
    TypeExpr,
    WhileStmt,
)


class TypeCheckError(ValueError):
    pass


@dataclass(frozen=True)
class _NullType(TypeExpr):
    pass


def check_program(program: Program) -> None:
    _check_tools(program)
    for pipeline in program.pipelines.values():
        _check_pipeline(program, pipeline)


def _check_tools(program: Program) -> None:
    declared_tools = set(program.tools)
    for agent in program.agents.values():
        for tool_name in agent.tools:
            if tool_name not in declared_tools:
                raise TypeCheckError(
                    f"Agent '{agent.name}' references unknown tool '{tool_name}'."
                )


def _check_pipeline(program: Program, pipeline: PipelineDef) -> None:
    env: dict[str, TypeExpr] = {param.name: param.type_expr for param in pipeline.params}
    saw_return = _check_block(program, pipeline, pipeline.statements, env)
    if not saw_return:
        raise TypeCheckError(f"Pipeline '{pipeline.name}' is missing a return statement.")


def _check_block(
    program: Program,
    pipeline: PipelineDef,
    statements: list[Stmt],
    env: dict[str, TypeExpr],
    in_loop: bool = False,
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
                        f"Duplicate target '{branch.target}' inside parallel block."
                    )
                if branch.target in env:
                    raise TypeCheckError(
                        f"Parallel target '{branch.target}' shadows an existing variable."
                    )
                new_bindings[branch.target] = _check_run_stmt(program, branch, env)
            env.update(new_bindings)
            continue

        if isinstance(stmt, IfStmt):
            cond_type = _infer_expr_type(stmt.condition, env)
            if not _is_assignable(cond_type, PrimitiveType("Bool")):
                raise TypeCheckError(
                    f"If condition must be Bool, got {cond_type} in pipeline '{pipeline.name}'."
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
                # A return in an if-without-else is path-conditional.
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
            option_type = _infer_expr_type(stmt.option_expr, env)
            if not isinstance(option_type, OptionType):
                raise TypeCheckError(
                    f"If-let expression must have Option type, got {option_type} "
                    f"in pipeline '{pipeline.name}'."
                )
            if stmt.binding in env:
                raise TypeCheckError(
                    f"If-let binding '{stmt.binding}' shadows an existing variable."
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
            cond_type = _infer_expr_type(stmt.condition, env)
            if not _is_assignable(cond_type, PrimitiveType("Bool")):
                raise TypeCheckError(
                    f"While condition must be Bool, got {cond_type} in pipeline '{pipeline.name}'."
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

        if isinstance(stmt, BreakStmt):
            if not in_loop:
                raise TypeCheckError(
                    f"'break' is only valid inside while loops in pipeline '{pipeline.name}'."
                )
            continue

        if isinstance(stmt, ContinueStmt):
            if not in_loop:
                raise TypeCheckError(
                    f"'continue' is only valid inside while loops in pipeline '{pipeline.name}'."
                )
            continue

        if isinstance(stmt, ReturnStmt):
            actual = _infer_expr_type(stmt.expr, env)
            if not _is_assignable(actual, pipeline.return_type):
                raise TypeCheckError(
                    f"Pipeline '{pipeline.name}' returns {actual}, expected {pipeline.return_type}."
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
            # Variable was re-bound with incompatible types in the two branches.
            # Drop it so callers cannot use it with a potentially wrong type.
            del merged[name]
    return merged


def _check_run_stmt(program: Program, stmt: RunStmt, env: dict[str, TypeExpr]) -> TypeExpr:
    task = program.tasks.get(stmt.task_name)
    if task is None:
        raise TypeCheckError(f"Unknown task '{stmt.task_name}'.")

    if stmt.agent_name is not None and stmt.agent_name not in program.agents:
        raise TypeCheckError(f"Unknown agent '{stmt.agent_name}'.")

    if task.execution_mode == "agent" and stmt.agent_name is None:
        raise TypeCheckError(
            f"Agent task '{task.name}' must be run with an explicit agent binding."
        )

    if stmt.retries < 0:
        raise TypeCheckError("Retries cannot be negative.")

    if stmt.on_fail not in {"abort", "use"}:
        raise TypeCheckError(f"Unsupported on_fail policy '{stmt.on_fail}'.")

    if stmt.on_fail == "abort" and stmt.fallback_expr is not None:
        raise TypeCheckError("Fallback expression is only valid with on_fail use.")

    if stmt.on_fail == "use":
        if stmt.fallback_expr is None:
            raise TypeCheckError("on_fail use requires a fallback expression.")
        fallback_type = _infer_expr_type(stmt.fallback_expr, env)
        if not _is_assignable(fallback_type, task.return_type):
            raise TypeCheckError(
                f"Fallback type {fallback_type} does not match task '{task.name}' return "
                f"type {task.return_type}."
            )

    expected_params = {param.name: param.type_expr for param in task.params}
    provided_params = set(stmt.args)

    missing = set(expected_params) - provided_params
    extra = provided_params - set(expected_params)

    if missing:
        raise TypeCheckError(f"Task '{task.name}' missing args: {sorted(missing)}")
    if extra:
        raise TypeCheckError(f"Task '{task.name}' received unknown args: {sorted(extra)}")

    for param_name, expected_type in expected_params.items():
        actual_type = _infer_expr_type(stmt.args[param_name], env)
        if not _is_assignable(actual_type, expected_type):
            raise TypeCheckError(
                f"Task '{task.name}' arg '{param_name}' has type {actual_type}, "
                f"expected {expected_type}."
            )

    return task.return_type


def _infer_expr_type(expr: Expr, env: dict[str, TypeExpr]) -> TypeExpr:
    if isinstance(expr, LiteralExpr):
        if expr.value is None:
            return _NullType()
        if isinstance(expr.value, bool):
            return PrimitiveType("Bool")
        if isinstance(expr.value, int) or isinstance(expr.value, float):
            return PrimitiveType("Number")
        if isinstance(expr.value, str):
            return PrimitiveType("String")
        raise TypeCheckError(f"Unsupported literal value: {expr.value!r}")

    if isinstance(expr, RefExpr):
        root = expr.parts[0]
        current = env.get(root)
        if current is None:
            raise TypeCheckError(f"Unknown variable '{root}'.")
        for field in expr.parts[1:]:
            if not isinstance(current, ObjType):
                raise TypeCheckError(f"Cannot access field '{field}' on non-object type {current}.")
            if field not in current.fields:
                raise TypeCheckError(f"Field '{field}' does not exist on type {current}.")
            current = current.fields[field]
        return current

    if isinstance(expr, ObjExpr):
        return ObjType({name: _infer_expr_type(value, env) for name, value in expr.fields.items()})

    if isinstance(expr, ListExpr):
        if not expr.items:
            raise TypeCheckError("Cannot infer type of empty list literal; provide at least one item.")
        first_type = _infer_expr_type(expr.items[0], env)
        for item in expr.items[1:]:
            item_type = _infer_expr_type(item, env)
            if not (_is_assignable(item_type, first_type) and _is_assignable(first_type, item_type)):
                raise TypeCheckError("List literal items must all share the same type.")
        return ListType(first_type)

    if isinstance(expr, BinaryExpr):
        left = _infer_expr_type(expr.left, env)
        right = _infer_expr_type(expr.right, env)
        if expr.op == "+":
            if isinstance(left, PrimitiveType) and isinstance(right, PrimitiveType):
                if left.name == right.name and left.name in {"String", "Number"}:
                    return left
            raise TypeCheckError(f"Cannot apply + to {left} and {right}.")
        if expr.op in {"==", "!="}:
            if _can_compare(left, right):
                return PrimitiveType("Bool")
            raise TypeCheckError(f"Cannot compare values of types {left} and {right}.")
        raise TypeCheckError(f"Unsupported binary operator '{expr.op}'.")

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
        if set(actual.fields) != set(expected.fields):
            return False
        return all(
            _is_assignable(actual.fields[field], expected.fields[field])
            for field in expected.fields
        )

    return False


def _can_compare(left: TypeExpr, right: TypeExpr) -> bool:
    if _is_assignable(left, right) and _is_assignable(right, left):
        return True
    if isinstance(left, _NullType) and isinstance(right, OptionType):
        return True
    if isinstance(right, _NullType) and isinstance(left, OptionType):
        return True
    return False
