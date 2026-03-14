from __future__ import annotations

import copy
import random
import time
from concurrent.futures import ThreadPoolExecutor, TimeoutError as FuturesTimeoutError
from dataclasses import dataclass
from typing import Any, Callable

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
    TryCatchStmt,
    TypeExpr,
    WhileStmt,
)
from .context import ExecutionContext

TaskHandler = Callable[[dict[str, Any], str | None], Any]
ToolHandler = Callable[[dict[str, Any]], Any]


class ExecutionError(ValueError):
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
    ctx: ExecutionContext | None = None,
) -> Any:
    if max_workers < 1:
        raise ExecutionError("max_workers must be greater than 0")

    pipeline = program.pipelines.get(pipeline_name)
    if pipeline is None:
        raise ExecutionError(f"Unknown pipeline '{pipeline_name}'.")
    _validate_pipeline_inputs(pipeline, inputs, program)

    if ctx is not None:
        ctx.record_pipeline_call(pipeline_name, inputs)

    env: dict[str, Any] = dict(inputs)
    try:
        _execute_block(
            program=program,
            statements=pipeline.statements,
            env=env,
            task_registry=task_registry,
            max_workers=max_workers,
            ctx=ctx,
        )
    except _PipelineReturned as returned:
        if not _is_value_assignable(returned.value, pipeline.return_type, program):
            raise ExecutionError(
                f"Pipeline '{pipeline.name}' returned invalid value {returned.value!r} "
                f"for type {pipeline.return_type}."
            )
        return returned.value

    raise ExecutionError(f"Pipeline '{pipeline_name}' completed without return.")


def execute_tool(
    program: Program,
    tool_name: str,
    args: dict[str, Any],
    tool_registry: dict[str, ToolHandler],
) -> Any:
    tool = program.tools.get(tool_name)
    if tool is None:
        raise ExecutionError(f"Unknown tool '{tool_name}'.")

    handler = tool_registry.get(tool_name)
    if handler is None:
        raise ExecutionError(f"No runtime handler registered for tool '{tool_name}'.")

    expected_params = {param.name: param.type_expr for param in tool.params}
    provided_params = set(args)

    missing = set(expected_params) - provided_params
    extra = provided_params - set(expected_params)
    if missing:
        raise ExecutionError(f"Tool '{tool_name}' missing args: {sorted(missing)}.")
    if extra:
        raise ExecutionError(f"Tool '{tool_name}' received unknown args: {sorted(extra)}.")

    for param_name, expected_type in expected_params.items():
        value = args[param_name]
        if not _is_value_assignable(value, expected_type, program):
            raise ExecutionError(
                f"Tool '{tool_name}' arg '{param_name}' has invalid value {value!r} "
                f"for type {expected_type}."
            )

    result = handler(copy.deepcopy(args))
    if not _is_value_assignable(result, tool.return_type, program):
        raise ExecutionError(
            f"Tool '{tool_name}' returned invalid value {result!r} "
            f"for type {tool.return_type}."
        )
    return result


def run_tests(
    program: Program,
    task_registry: dict[str, TaskHandler],
    max_workers: int = 8,
    ctx: ExecutionContext | None = None,
) -> list[dict[str, Any]]:
    """Execute all test blocks in the program. Returns list of {name, passed, error}."""
    results: list[dict[str, Any]] = []
    for test_block in program.test_blocks:
        env: dict[str, Any] = {}
        try:
            _execute_block(
                program=program,
                statements=test_block.statements,
                env=env,
                task_registry=task_registry,
                max_workers=max_workers,
                ctx=ctx,
            )
            results.append({"name": test_block.name, "passed": True, "error": None})
        except _PipelineReturned:
            # Test blocks can return early; treat as pass
            results.append({"name": test_block.name, "passed": True, "error": None})
        except Exception as exc:
            results.append({"name": test_block.name, "passed": False, "error": str(exc)})
    return results


def _execute_block(
    program: Program,
    statements: tuple[Stmt, ...] | list[Stmt],
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    max_workers: int,
    ctx: ExecutionContext | None = None,
) -> None:
    for stmt in statements:
        if isinstance(stmt, RunStmt):
            env[stmt.target] = _execute_run_stmt(program, stmt, env, task_registry, ctx=ctx)
            continue

        if isinstance(stmt, ParallelStmt):
            _execute_parallel(program, stmt, env, task_registry, max_workers, ctx=ctx)
            continue

        if isinstance(stmt, IfStmt):
            condition = _eval_expr(stmt.condition, env)
            if not isinstance(condition, bool):
                raise ExecutionError("If condition did not evaluate to Bool.")
            branch = stmt.then_statements if condition else (stmt.else_statements or [])
            _execute_block(program, branch, env, task_registry, max_workers, ctx=ctx)
            continue

        if isinstance(stmt, IfLetStmt):
            option_value = _eval_expr(stmt.option_expr, env)
            if option_value is None:
                branch = stmt.else_statements or []
                _execute_block(program, branch, env, task_registry, max_workers, ctx=ctx)
            else:
                previous = env.get(stmt.binding)
                had_previous = stmt.binding in env
                env[stmt.binding] = option_value
                try:
                    _execute_block(program, stmt.then_statements, env, task_registry, max_workers, ctx=ctx)
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
                    raise ExecutionError("While condition did not evaluate to Bool.")
                if not condition:
                    break
                try:
                    _execute_block(program, stmt.statements, env, task_registry, max_workers, ctx=ctx)
                except _LoopContinue:
                    continue
                except _LoopBreak:
                    break
            continue

        if isinstance(stmt, TryCatchStmt):
            try:
                _execute_block(program, stmt.try_body, env, task_registry, max_workers, ctx=ctx)
            except (ExecutionError, Exception) as exc:
                if isinstance(exc, (_PipelineReturned, _LoopBreak, _LoopContinue)):
                    raise
                env[stmt.error_var] = str(exc)
                _execute_block(program, stmt.catch_body, env, task_registry, max_workers, ctx=ctx)
            continue

        if isinstance(stmt, AssertStmt):
            condition = _eval_expr(stmt.condition, env)
            if not condition:
                msg = stmt.message or "Assertion failed"
                raise ExecutionError(f"Assertion failed: {msg}")
            continue

        if isinstance(stmt, BreakStmt):
            raise _LoopBreak()

        if isinstance(stmt, ContinueStmt):
            raise _LoopContinue()

        if isinstance(stmt, ReturnStmt):
            raise _PipelineReturned(_eval_expr(stmt.expr, env))

        raise ExecutionError(f"Unsupported statement in runtime: {type(stmt).__name__}")

    return None


def _execute_parallel(
    program: Program,
    stmt: ParallelStmt,
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    max_workers: int,
    ctx: ExecutionContext | None = None,
) -> None:
    if ctx is not None:
        ctx.record_parallel_start(len(stmt.branches))

    snapshot = dict(env)
    worker_count = stmt.max_concurrency or max_workers
    with ThreadPoolExecutor(max_workers=worker_count) as executor:
        future_pairs = []
        for branch in stmt.branches:
            future = executor.submit(_execute_run_stmt, program, branch, snapshot, task_registry, ctx=ctx)
            future_pairs.append((branch.target, branch.timeout, future))
        for target, timeout, future in future_pairs:
            try:
                env[target] = future.result(timeout=timeout)
            except FuturesTimeoutError:
                raise ExecutionError(f"Parallel branch '{target}' timed out after {timeout}s.")

    if ctx is not None:
        ctx.record_parallel_end(len(stmt.branches))


def _execute_run_stmt(
    program: Program,
    stmt: RunStmt,
    env: dict[str, Any],
    task_registry: dict[str, TaskHandler],
    ctx: ExecutionContext | None = None,
) -> Any:
    # Check if this is a pipeline call
    if stmt.task_name in program.pipelines and stmt.task_name not in program.tasks:
        bound_args = {name: _eval_expr(expr, env) for name, expr in stmt.args.items()}
        if ctx is not None:
            ctx.record_task_start(f"pipeline:{stmt.task_name}", bound_args)
        try:
            result = execute_pipeline(
                program=program,
                pipeline_name=stmt.task_name,
                inputs=bound_args,
                task_registry=task_registry,
                ctx=ctx,
            )
        except Exception as exc:
            if ctx is not None:
                ctx.record_task_error(f"pipeline:{stmt.task_name}", exc)
            raise
        if ctx is not None:
            ctx.record_task_end(f"pipeline:{stmt.task_name}", result)
        return result

    task = program.tasks.get(stmt.task_name)
    if task is None:
        raise ExecutionError(f"Unknown task '{stmt.task_name}'.")

    handler = task_registry.get(stmt.task_name)
    if handler is None:
        raise ExecutionError(f"No runtime handler registered for task '{stmt.task_name}'.")

    bound_args = {name: _eval_expr(expr, env) for name, expr in stmt.args.items()}
    max_attempts = stmt.retries + 1
    last_error: Exception | None = None

    # Validate enum values in args at runtime
    _validate_enum_args(program, task, bound_args)

    if ctx is not None:
        ctx.record_task_start(stmt.task_name, bound_args)

    for attempt in range(max_attempts):
        if attempt > 0:
            delay = min(2 ** attempt * 0.1, 5.0)
            delay *= 0.5 + random.random()  # noqa: S311 - jitter for retry backoff
            time.sleep(delay)
        try:
            result = _invoke_handler(handler, copy.deepcopy(bound_args), stmt.agent_name, stmt.timeout)
        except Exception as exc:  # noqa: BLE001 - workflow policy decides error handling
            last_error = exc
            if ctx is not None:
                ctx.record_retry(stmt.task_name, attempt + 1, exc)
            if attempt < max_attempts - 1:
                continue
            if stmt.on_fail == "use":
                if stmt.fallback_expr is None:
                    raise ExecutionError(
                        f"Task '{stmt.task_name}' has on_fail use without fallback expression."
                    ) from exc
                fallback = _eval_expr(stmt.fallback_expr, env)
                if not _is_value_assignable(fallback, task.return_type, program):
                    raise ExecutionError(
                        f"Task '{stmt.task_name}' fallback produced invalid value "
                        f"{fallback!r} for type {task.return_type}."
                    ) from exc
                if ctx is not None:
                    ctx.record_task_end(stmt.task_name, fallback)
                return fallback
            task_label = _format_task_label(stmt.task_name, stmt.agent_name)
            if ctx is not None:
                ctx.record_task_error(stmt.task_name, exc)
            raise ExecutionError(
                f"{task_label} failed after {max_attempts} attempts. "
                f"Last error: {_format_exception_detail(exc)}"
            ) from exc

        if not _is_value_assignable(result, task.return_type, program):
            raise ExecutionError(
                f"Task '{stmt.task_name}' returned invalid value {result!r} "
                f"for type {task.return_type}."
            )

        # Validate enum values in result at runtime
        _validate_enum_result(program, task, result)

        if ctx is not None:
            ctx.record_task_end(stmt.task_name, result)
        return result

    raise ExecutionError(f"{_format_task_label(stmt.task_name, stmt.agent_name)} failed.") from last_error


def _validate_enum_args(program: Program, task, args: dict[str, Any]) -> None:
    """Validate that enum-typed args have valid variant values."""
    for param in task.params:
        if isinstance(param.type_expr, EnumType):
            enum_def = program.enum_types.get(param.type_expr.name)
            if enum_def is not None and param.name in args:
                value = args[param.name]
                if isinstance(value, str) and value not in enum_def.variants:
                    raise ExecutionError(
                        f"Task '{task.name}' arg '{param.name}' has value '{value}' "
                        f"which is not a valid variant of enum '{enum_def.name}'. "
                        f"Valid variants: {list(enum_def.variants)}"
                    )


def _validate_enum_result(program: Program, task, result: Any) -> None:
    """Validate enum values in task results."""
    # Walk the return type looking for EnumType fields
    _validate_enum_value(program, task.return_type, result)


def _validate_enum_value(program: Program, type_expr, value: Any) -> None:
    """Recursively validate enum values in a structured result."""
    if isinstance(type_expr, EnumType):
        enum_def = program.enum_types.get(type_expr.name)
        if enum_def is not None and isinstance(value, str) and value not in enum_def.variants:
            raise ExecutionError(
                f"Value '{value}' is not a valid variant of enum '{enum_def.name}'. "
                f"Valid variants: {list(enum_def.variants)}"
            )
    elif isinstance(type_expr, ObjType) and isinstance(value, dict):
        for field_name, field_type in type_expr.fields.items():
            if field_name in value:
                _validate_enum_value(program, field_type, value[field_name])


def _invoke_handler(
    handler: TaskHandler,
    args: dict[str, Any],
    agent_name: str | None,
    timeout: float | None,
) -> Any:
    if timeout is None:
        return handler(args, agent_name)
    with ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(handler, args, agent_name)
        try:
            return future.result(timeout=timeout)
        except FuturesTimeoutError:
            raise ExecutionError(
                f"Task handler timed out after {timeout}s."
            )


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
            raise ExecutionError(f"Unknown variable '{expr.parts[0]}'.")
        value = env[expr.parts[0]]
        for field in expr.parts[1:]:
            if not isinstance(value, dict) or field not in value:
                raise ExecutionError(f"Cannot resolve field access '{'.'.join(expr.parts)}'.")
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
        raise ExecutionError(f"Unsupported operator '{expr.op}'.")

    if isinstance(expr, ObjExpr):
        return {name: _eval_expr(value, env) for name, value in expr.fields.items()}

    if isinstance(expr, ListExpr):
        return [_eval_expr(item, env) for item in expr.items]

    return expr.value


def _validate_pipeline_inputs(pipeline: PipelineDef, inputs: dict[str, Any], program: Program | None = None) -> None:
    expected = {param.name: param.type_expr for param in pipeline.params}
    provided = set(inputs)
    missing = set(expected) - provided
    extra = provided - set(expected)

    if missing:
        raise ExecutionError(
            f"Pipeline '{pipeline.name}' missing inputs: {sorted(missing)}."
        )
    if extra:
        raise ExecutionError(
            f"Pipeline '{pipeline.name}' received unknown inputs: {sorted(extra)}."
        )

    for name, expected_type in expected.items():
        value = inputs[name]
        if not _is_value_assignable(value, expected_type, program):
            raise ExecutionError(
                f"Pipeline '{pipeline.name}' input '{name}' has invalid value {value!r} "
                f"for type {expected_type}."
            )


def _is_value_assignable(value: Any, expected: TypeExpr, program: Program | None = None) -> bool:
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
        return all(_is_value_assignable(item, expected.item_type, program) for item in value)

    if isinstance(expected, OptionType):
        if value is None:
            return True
        return _is_value_assignable(value, expected.item_type, program)

    if isinstance(expected, ObjType):
        if not isinstance(value, dict):
            return False
        if not set(expected.fields) <= set(value):
            return False
        return all(
            _is_value_assignable(value[field], field_type, program)
            for field, field_type in expected.fields.items()
        )

    if isinstance(expected, EnumType):
        # EnumType values are strings; runtime validation of variants happens separately
        return isinstance(value, str)

    return False
