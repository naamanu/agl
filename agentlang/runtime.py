from __future__ import annotations

import copy
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any, Callable

from .ast import (
    BinaryExpr,
    BreakStmt,
    ContinueStmt,
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
    WhileStmt,
)

TaskHandler = Callable[[dict[str, Any], str | None], Any]
ToolHandler = Callable[[dict[str, Any]], Any]


class RuntimeError(ValueError):
    pass


@dataclass
class _PipelineReturned(Exception):
    value: Any


class _LoopBreak(Exception):
    pass


class _LoopContinue(Exception):
    pass


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


def execute_tool(
    program: Program,
    tool_name: str,
    args: dict[str, Any],
    tool_registry: dict[str, ToolHandler],
) -> Any:
    tool = program.tools.get(tool_name)
    if tool is None:
        raise RuntimeError(f"Unknown tool '{tool_name}'.")

    handler = tool_registry.get(tool_name)
    if handler is None:
        raise RuntimeError(f"No runtime handler registered for tool '{tool_name}'.")

    expected_params = {param.name: param.type_expr for param in tool.params}
    provided_params = set(args)

    missing = set(expected_params) - provided_params
    extra = provided_params - set(expected_params)
    if missing:
        raise RuntimeError(f"Tool '{tool_name}' missing args: {sorted(missing)}.")
    if extra:
        raise RuntimeError(f"Tool '{tool_name}' received unknown args: {sorted(extra)}.")

    for param_name, expected_type in expected_params.items():
        value = args[param_name]
        if not _is_value_assignable(value, expected_type):
            raise RuntimeError(
                f"Tool '{tool_name}' arg '{param_name}' has invalid value {value!r} "
                f"for type {expected_type}."
            )

    result = handler(copy.deepcopy(args))
    if not _is_value_assignable(result, tool.return_type):
        raise RuntimeError(
            f"Tool '{tool_name}' returned invalid value {result!r} "
            f"for type {tool.return_type}."
        )
    return result


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

        if isinstance(stmt, WhileStmt):
            while True:
                condition = _eval_expr(stmt.condition, env)
                if not isinstance(condition, bool):
                    raise RuntimeError("While condition did not evaluate to Bool.")
                if not condition:
                    break
                try:
                    _execute_block(program, stmt.statements, env, task_registry, max_workers)
                except _LoopContinue:
                    continue
                except _LoopBreak:
                    break
            continue

        if isinstance(stmt, BreakStmt):
            raise _LoopBreak()

        if isinstance(stmt, ContinueStmt):
            raise _LoopContinue()

        if isinstance(stmt, ReturnStmt):
            raise _PipelineReturned(_eval_expr(stmt.expr, env))

        raise RuntimeError(f"Unsupported statement in runtime: {type(stmt).__name__}")

    return None


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
            task_label = _format_task_label(stmt.task_name, stmt.agent_name)
            raise RuntimeError(
                f"{task_label} failed after {max_attempts} attempts. "
                f"Last error: {_format_exception_detail(exc)}"
            ) from exc

        if not _is_value_assignable(result, task.return_type):
            raise RuntimeError(
                f"Task '{stmt.task_name}' returned invalid value {result!r} "
                f"for type {task.return_type}."
            )
        return result

    raise RuntimeError(f"{_format_task_label(stmt.task_name, stmt.agent_name)} failed.") from last_error


def _format_task_label(task_name: str, agent_name: str | None) -> str:
    if agent_name is None:
        return f"Task '{task_name}'"
    return f"Task '{task_name}' by agent '{agent_name}'"


def _format_exception_detail(exc: Exception) -> str:
    message = str(exc).strip()
    if not message:
        return type(exc).__name__
    return f"{type(exc).__name__}: {message}"


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
