from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any, Callable

from .ast import (
    BinaryExpr,
    Expr,
    IfStmt,
    ListExpr,
    LiteralExpr,
    ObjExpr,
    ParallelStmt,
    Program,
    RefExpr,
    ReturnStmt,
    RunStmt,
    Stmt,
)

TaskHandler = Callable[[dict[str, Any], str | None], Any]


class AgentRuntimeError(ValueError):
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
    pipeline = program.pipelines.get(pipeline_name)
    if pipeline is None:
        raise AgentRuntimeError(f"Unknown pipeline '{pipeline_name}'.")

    env: dict[str, Any] = dict(inputs)
    try:
        _execute_block(
            statements=pipeline.statements,
            env=env,
            task_registry=task_registry,
            max_workers=max_workers,
        )
    except _PipelineReturned as returned:
        return returned.value

    raise AgentRuntimeError(f"Pipeline '{pipeline_name}' completed without return.")


def _execute_block(
    statements: tuple[Stmt, ...],
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    max_workers: int,
) -> None:
    for stmt in statements:
        if isinstance(stmt, RunStmt):
            env[stmt.target] = _execute_run_stmt(stmt, env, task_registry)
            continue

        if isinstance(stmt, ParallelStmt):
            _execute_parallel(stmt, env, task_registry, max_workers)
            continue

        if isinstance(stmt, IfStmt):
            condition = _eval_expr(stmt.condition, env)
            if not isinstance(condition, bool):
                raise AgentRuntimeError("If condition did not evaluate to Bool.")
            branch: tuple[Stmt, ...] = (
                stmt.then_statements if condition
                else (stmt.else_statements if stmt.else_statements is not None else ())
            )
            _execute_block(branch, env, task_registry, max_workers)
            continue

        if isinstance(stmt, ReturnStmt):
            raise _PipelineReturned(_eval_expr(stmt.expr, env))

        raise AgentRuntimeError(f"Unsupported statement in runtime: {type(stmt).__name__}")


def _execute_parallel(
    stmt: ParallelStmt,
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    max_workers: int,
) -> None:
    snapshot = dict(env)
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        future_pairs = []
        for branch in stmt.branches:
            future = executor.submit(_execute_run_stmt, branch, snapshot, task_registry)
            future_pairs.append((branch.target, future))
        for target, future in future_pairs:
            env[target] = future.result()


def _execute_run_stmt(
    stmt: RunStmt, env: dict[str, Any], task_registry: dict[str, TaskHandler]
) -> Any:
    handler = task_registry.get(stmt.task_name)
    if handler is None:
        raise AgentRuntimeError(f"No runtime handler registered for task '{stmt.task_name}'.")

    bound_args = {name: _eval_expr(expr, env) for name, expr in stmt.args}
    max_attempts = stmt.retries + 1

    for attempt in range(max_attempts):
        try:
            return handler(bound_args, stmt.agent_name)
        except Exception as exc:  # noqa: BLE001 - workflow policy decides error handling
            if attempt < max_attempts - 1:
                continue
            if stmt.on_fail == "use":
                if stmt.fallback_expr is None:
                    raise AgentRuntimeError(
                        f"Task '{stmt.task_name}' has on_fail use without fallback expression."
                    ) from exc
                return _eval_expr(stmt.fallback_expr, env)
            raise AgentRuntimeError(
                f"Task '{stmt.task_name}' failed after {max_attempts} attempts."
            ) from exc


def _eval_expr(expr: Expr, env: dict[str, Any]) -> Any:
    if isinstance(expr, RefExpr):
        if expr.parts[0] not in env:
            raise AgentRuntimeError(f"Unknown variable '{expr.parts[0]}'.")
        value = env[expr.parts[0]]
        for field in expr.parts[1:]:
            if not isinstance(value, dict) or field not in value:
                raise AgentRuntimeError(f"Cannot resolve field access '{'.'.join(expr.parts)}'.")
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
        raise AgentRuntimeError(f"Unsupported operator '{expr.op}'.")

    if isinstance(expr, ObjExpr):
        return {name: _eval_expr(value, env) for name, value in expr.fields}

    if isinstance(expr, ListExpr):
        return [_eval_expr(item, env) for item in expr.items]

    if isinstance(expr, LiteralExpr):
        return expr.value

    raise AgentRuntimeError(f"Unsupported expression in runtime: {type(expr).__name__}")

