from __future__ import annotations

import copy
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any, Callable

from .ast import (
    BinaryExpr,
    Expr,
    IfLetStmt,
    IfStmt,
    ListExpr,
    ListType,
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
)

TaskHandler = Callable[[dict[str, Any], str | None], Any]


class RuntimeError(ValueError):
    pass


@dataclass
class _PipelineReturned(Exception):
    value: Any


def execute_pipeline(
    program: Program,
    pipeline_name: str,
    inputs: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    max_workers: int = 8,
) -> Any:
    if max_workers < 1:
        raise RuntimeError("max_workers must be greater than 0")

    pipeline = program.pipelines.get(pipeline_name)
    if pipeline is None:
        raise RuntimeError(f"Unknown pipeline '{pipeline_name}'.")
    _validate_pipeline_inputs(pipeline, inputs)

    env: dict[str, Any] = dict(inputs)
    try:
        _execute_block(
            program=program,
            statements=pipeline.statements,
            env=env,
            task_registry=task_registry,
            max_workers=max_workers,
        )
    except _PipelineReturned as returned:
        if not _is_value_assignable(returned.value, pipeline.return_type):
            raise RuntimeError(
                f"Pipeline '{pipeline.name}' returned invalid value {returned.value!r} "
                f"for type {pipeline.return_type}."
            )
        return returned.value

    raise RuntimeError(f"Pipeline '{pipeline_name}' completed without return.")


def _execute_block(
    program: Program,
    statements: list[Stmt],
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    max_workers: int,
) -> None:
    for stmt in statements:
        if isinstance(stmt, RunStmt):
            env[stmt.target] = _execute_run_stmt(program, stmt, env, task_registry)
            continue

        if isinstance(stmt, ParallelStmt):
            _execute_parallel(program, stmt, env, task_registry, max_workers)
            continue

        if isinstance(stmt, IfStmt):
            condition = _eval_expr(stmt.condition, env)
            if not isinstance(condition, bool):
                raise RuntimeError("If condition did not evaluate to Bool.")
            branch = stmt.then_statements if condition else (stmt.else_statements or [])
            _execute_block(program, branch, env, task_registry, max_workers)
            continue

        if isinstance(stmt, IfLetStmt):
            option_value = _eval_expr(stmt.option_expr, env)
            if option_value is None:
                branch = stmt.else_statements or []
                _execute_block(program, branch, env, task_registry, max_workers)
            else:
                previous = env.get(stmt.binding)
                had_previous = stmt.binding in env
                env[stmt.binding] = option_value
                try:
                    _execute_block(program, stmt.then_statements, env, task_registry, max_workers)
                finally:
                    if had_previous:
                        env[stmt.binding] = previous
                    else:
                        del env[stmt.binding]
            continue

        if isinstance(stmt, ReturnStmt):
            raise _PipelineReturned(_eval_expr(stmt.expr, env))

        raise RuntimeError(f"Unsupported statement in runtime: {type(stmt).__name__}")


def _execute_parallel(
    program: Program,
    stmt: ParallelStmt,
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    max_workers: int,
) -> None:
    snapshot = dict(env)
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        future_pairs = []
        for branch in stmt.branches:
            future = executor.submit(_execute_run_stmt, program, branch, snapshot, task_registry)
            future_pairs.append((branch.target, future))
        for target, future in future_pairs:
            env[target] = future.result()


def _execute_run_stmt(
    program: Program,
    stmt: RunStmt,
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
) -> Any:
    task = program.tasks.get(stmt.task_name)
    if task is None:
        raise RuntimeError(f"Unknown task '{stmt.task_name}'.")

    handler = task_registry.get(stmt.task_name)
    if handler is None:
        raise RuntimeError(f"No runtime handler registered for task '{stmt.task_name}'.")

    bound_args = {name: _eval_expr(expr, env) for name, expr in stmt.args.items()}
    max_attempts = stmt.retries + 1
    last_error: Exception | None = None

    for attempt in range(max_attempts):
        try:
            # Handlers receive isolated argument copies so task-side mutation
            # cannot affect pipeline environment or sibling parallel branches.
            result = handler(copy.deepcopy(bound_args), stmt.agent_name)
        except Exception as exc:  # noqa: BLE001 - workflow policy decides error handling
            last_error = exc
            if attempt < max_attempts - 1:
                continue
            if stmt.on_fail == "use":
                if stmt.fallback_expr is None:
                    raise RuntimeError(
                        f"Task '{stmt.task_name}' has on_fail use without fallback expression."
                    ) from exc
                fallback = _eval_expr(stmt.fallback_expr, env)
                if not _is_value_assignable(fallback, task.return_type):
                    raise RuntimeError(
                        f"Task '{stmt.task_name}' fallback produced invalid value "
                        f"{fallback!r} for type {task.return_type}."
                    ) from exc
                return fallback
            raise RuntimeError(
                f"Task '{stmt.task_name}' failed after {max_attempts} attempts."
            ) from exc

        if not _is_value_assignable(result, task.return_type):
            raise RuntimeError(
                f"Task '{stmt.task_name}' returned invalid value {result!r} "
                f"for type {task.return_type}."
            )
        return result

    raise RuntimeError(f"Task '{stmt.task_name}' failed.") from last_error


def _eval_expr(expr: Expr, env: dict[str, Any]) -> Any:
    if isinstance(expr, RefExpr):
        if expr.parts[0] not in env:
            raise RuntimeError(f"Unknown variable '{expr.parts[0]}'.")
        value = env[expr.parts[0]]
        for field in expr.parts[1:]:
            if not isinstance(value, dict) or field not in value:
                raise RuntimeError(f"Cannot resolve field access '{'.'.join(expr.parts)}'.")
            value = value[field]
        return value

    if isinstance(expr, BinaryExpr):
        left = _eval_expr(expr.left, env)
        right = _eval_expr(expr.right, env)
        if expr.op == "+":
            return left + right
        if expr.op == "==":
            return left == right
        if expr.op == "!=":
            return left != right
        raise RuntimeError(f"Unsupported operator '{expr.op}'.")

    if isinstance(expr, ObjExpr):
        return {name: _eval_expr(value, env) for name, value in expr.fields.items()}

    if isinstance(expr, ListExpr):
        return [_eval_expr(item, env) for item in expr.items]

    return expr.value


def _validate_pipeline_inputs(pipeline: PipelineDef, inputs: dict[str, Any]) -> None:
    expected = {param.name: param.type_expr for param in pipeline.params}
    provided = set(inputs)
    missing = set(expected) - provided
    extra = provided - set(expected)

    if missing:
        raise RuntimeError(
            f"Pipeline '{pipeline.name}' missing inputs: {sorted(missing)}."
        )
    if extra:
        raise RuntimeError(
            f"Pipeline '{pipeline.name}' received unknown inputs: {sorted(extra)}."
        )

    for name, expected_type in expected.items():
        value = inputs[name]
        if not _is_value_assignable(value, expected_type):
            raise RuntimeError(
                f"Pipeline '{pipeline.name}' input '{name}' has invalid value {value!r} "
                f"for type {expected_type}."
            )


def _is_value_assignable(value: Any, expected: TypeExpr) -> bool:
    if isinstance(expected, PrimitiveType):
        if expected.name == "String":
            return isinstance(value, str)
        if expected.name == "Number":
            return isinstance(value, (int, float)) and not isinstance(value, bool)
        if expected.name == "Bool":
            return isinstance(value, bool)
        return False

    if isinstance(expected, ListType):
        if not isinstance(value, list):
            return False
        return all(_is_value_assignable(item, expected.item_type) for item in value)

    if isinstance(expected, OptionType):
        if value is None:
            return True
        return _is_value_assignable(value, expected.item_type)

    if isinstance(expected, ObjType):
        if not isinstance(value, dict):
            return False
        if set(value) != set(expected.fields):
            return False
        return all(
            _is_value_assignable(value[field], field_type)
            for field, field_type in expected.fields.items()
        )

    return False
