from __future__ import annotations

import json
import os
import sys
from dataclasses import dataclass
from threading import Lock
from typing import Any, Callable

from .adapters import (
    AnthropicAdapterError,
    AnthropicMessagesClient,
    OpenAIAdapterError,
    OpenAIResponsesClient,
    ToolAdapterError,
    duckduckgo_search,
    fetch_url_text,
    format_search_hits,
)
from .ast import AgentDef, ListType, ObjType, OptionType, PrimitiveType, Program, TaskDef, TypeExpr
from .runtime import ExecutionError, execute_tool

TaskHandler = Callable[[dict[str, Any], str | None], Any]
ToolHandler = Callable[[dict[str, Any]], Any]

LLMClient = OpenAIResponsesClient | AnthropicMessagesClient

_OPENAI_TO_ANTHROPIC_MODELS: dict[str, str] = {
    "gpt-4.1": "claude-sonnet-4-20250514",
    "gpt-4.1-mini": "claude-haiku-4-5-20251001",
    "gpt-4o": "claude-sonnet-4-20250514",
    "gpt-4o-mini": "claude-haiku-4-5-20251001",
}

_LIVE_MODES = frozenset({"live", "anthropic"})

_flaky_attempts: dict[str, int] = {}
_flaky_attempts_lock = Lock()


@dataclass(frozen=True)
class RuntimeAdapterConfig:
    mode: str
    default_model: str
    web_max_results: int
    http_timeout_s: float
    trace_live: bool


def default_task_registry(
    program: Program | None = None,
    *,
    adapter_mode: str | None = None,
    trace_live: bool | None = None,
    extra_tool_handlers: dict[str, ToolHandler] | None = None,
) -> dict[str, TaskHandler]:
    config = _resolve_config(adapter_mode, trace_live=trace_live)
    if program is None:
        raise ExecutionError("default_task_registry requires a parsed program.")

    agent_map = program.agents
    tool_registry = default_tool_registry(adapter_mode=adapter_mode)
    # Merge plugin-provided tool handlers (plugin takes precedence over builtins)
    if extra_tool_handlers:
        tool_registry.update(extra_tool_handlers)
    client = _build_llm_client(config)

    def resolve_agent(agent_name: str | None) -> AgentDef | None:
        if agent_name is None:
            return None
        return agent_map.get(agent_name)

    def research(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        topic = str(args["topic"])
        if config.mode in _LIVE_MODES:
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model, config)
            trace = _start_live_trace(
                config,
                task_name="research",
                agent=agent,
                model=model,
                args=args,
            )
            system = (
                "You produce factual, concise research notes for downstream agent workflows."
            )
            prompt = (
                f"Topic: {topic}\n\n"
                "Use available tools when they would improve factual grounding.\n"
                "Return 5-8 short bullet points in plain text."
            )
            tools = _agent_tool_definitions(program, agent_def)
            if tools:
                text = _complete_live_with_tools(
                    client,
                    model=model,
                    system=system,
                    prompt=prompt,
                    tools=tools,
                    tool_executor=_trace_tool_executor(
                        config,
                        task_name="research",
                        agent=agent,
                        executor=lambda name, call_args: execute_tool(
                            program,
                            name,
                            call_args,
                            tool_registry,
                        ),
                    ),
                    trace=trace,
                )
            else:
                text = _complete_live(
                    client,
                    model=model,
                    system=system,
                    prompt=prompt,
                    trace=trace,
                )
            trace(f"result={_preview_value(text)}")
            return {"notes": text}

        who = agent or "default-agent"
        return {"notes": f"[{who}] key points for '{topic}'"}

    def draft(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        notes = str(args["notes"])
        if config.mode in _LIVE_MODES:
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model, config)
            trace = _start_live_trace(
                config,
                task_name="draft",
                agent=agent,
                model=model,
                args=args,
            )
            system = "You write clean drafts for engineering users."
            prompt = (
                "Write a short, structured draft from these notes.\n"
                "Use a title and concise sections.\n\n"
                f"Notes:\n{notes}"
            )
            text = _complete_live(client, model=model, system=system, prompt=prompt, trace=trace)
            trace(f"result={_preview_value(text)}")
            return {"article": text}

        who = agent or "default-agent"
        return {"article": f"[{who}] Draft article:\n{notes}"}

    def compare(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        note_a = str(args["note_a"])
        note_b = str(args["note_b"])
        if config.mode in _LIVE_MODES:
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model, config)
            trace = _start_live_trace(
                config,
                task_name="compare",
                agent=agent,
                model=model,
                args=args,
            )
            system = "You compare options and return a concrete recommendation."
            prompt = (
                "Compare option A and B. Provide a clear decision and rationale.\n\n"
                f"Option A:\n{note_a}\n\nOption B:\n{note_b}"
            )
            text = _complete_live(client, model=model, system=system, prompt=prompt, trace=trace)
            trace(f"result={_preview_value(text)}")
            return {"decision": text}

        who = agent or "default-agent"
        return {"decision": f"[{who}] Option A vs B\nA: {note_a}\nB: {note_b}"}

    def extract_intent(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        message = str(args["message"]).lower()
        if "refund" in message:
            intent = "billing"
        elif "bug" in message or "error" in message:
            intent = "technical"
        else:
            intent = "general"
        urgency = "high" if any(w in message for w in ["urgent", "asap", "down"]) else "normal"
        return {"intent": intent, "urgency": urgency}

    def route(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        intent = str(args["intent"])
        urgency = str(args["urgency"])
        queue = f"{intent}-priority" if urgency == "high" else f"{intent}-standard"
        return {"queue": queue}

    def respond(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        intent = str(args["intent"])
        queue = str(args["queue"])
        if config.mode in _LIVE_MODES:
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model, config)
            trace = _start_live_trace(
                config,
                task_name="respond",
                agent=agent,
                model=model,
                args=args,
            )
            prompt = (
                "Write a short support response to the user based on routing data.\n"
                f"Intent: {intent}\nQueue: {queue}"
            )
            text = _complete_live(client, model=model, system=None, prompt=prompt, trace=trace)
            trace(f"result={_preview_value(text)}")
            return {"reply": text}

        who = agent or "default-agent"
        return {"reply": f"[{who}] Routed as {intent} to {queue}."}

    def flaky_fetch(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        key = str(args["key"])
        failures_before_success = int(args["failures_before_success"])

        with _flaky_attempts_lock:
            attempts_so_far = _flaky_attempts.get(key, 0)
            if attempts_so_far < failures_before_success:
                _flaky_attempts[key] = attempts_so_far + 1
                should_fail = True
            else:
                should_fail = False

        if should_fail:
            raise ExecutionError(
                f"Transient failure for key '{key}' "
                f"({attempts_so_far + 1}/{failures_before_success})"
            )
        who = agent or "default-agent"
        return {"data": f"[{who}] fetched payload for {key}"}

    def llm_complete(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        prompt = str(args["prompt"])
        if config.mode not in _LIVE_MODES:
            who = agent or "default-agent"
            return {"text": f"[{who}] {prompt}"}
        agent_def = resolve_agent(agent)
        model = _resolve_model(agent_def, config.default_model, config)
        trace = _start_live_trace(
            config,
            task_name="llm_complete",
            agent=agent,
            model=model,
            args=args,
        )
        text = _complete_live(client, model=model, system=None, prompt=prompt, trace=trace)
        trace(f"result={_preview_value(text)}")
        return {"text": text}

    def countdown(args: dict[str, Any], agent: str | None) -> dict[str, Any]:
        current = float(args["current"])
        next_value = current - 1
        if next_value < 0:
            next_value = 0
        if next_value.is_integer():
            next_number: int | float = int(next_value)
        else:
            next_number = next_value
        return {"next": next_number, "done": next_number <= 0}

    def merge_drafts(args: dict[str, Any], agent: str | None) -> dict[str, Any]:
        draft_a = args["draft_a"]
        draft_b = args["draft_b"]
        wc_a = args["word_count_a"]
        wc_b = args["word_count_b"]
        article = draft_a + ("\n\n" + draft_b if draft_b else "")
        sections = [s for s in [draft_a, draft_b] if s]
        return {"article": article, "sections": sections, "total_words": wc_a + wc_b}

    def fallback_enrich(args: dict[str, Any], agent: str | None) -> dict[str, Any]:
        query = args["query"]
        return {"extra": f"[fallback enrichment for '{query}']"}

    registry = {
        "research": research,
        "draft": draft,
        "compare": compare,
        "extract_intent": extract_intent,
        "route": route,
        "respond": respond,
        "flaky_fetch": flaky_fetch,
        "llm_complete": llm_complete,
        "countdown": countdown,
        "merge_drafts": merge_drafts,
        "fallback_enrich": fallback_enrich,
    }

    for task in program.tasks.values():
        if task.execution_mode == "agent":
            registry[task.name] = _make_agent_task_handler(
                program,
                task,
                config=config,
                resolve_agent=resolve_agent,
                client=client,
                tool_registry=tool_registry,
            )

    return registry


def default_tool_registry(
    *,
    adapter_mode: str | None = None,
) -> dict[str, ToolHandler]:
    config = _resolve_config(adapter_mode)

    def web_search(args: dict[str, Any]) -> list[dict[str, str]]:
        query = str(args["query"])
        return _run_web_search(
            query,
            max_results=config.web_max_results,
            timeout_s=config.http_timeout_s,
        )

    def fetch_url(args: dict[str, Any]) -> dict[str, str]:
        url = str(args["url"])
        if config.mode not in _LIVE_MODES:
            return {"content": f"[fetch_url] {url}"}
        return {
            "content": _run_fetch_url(
                url,
                timeout_s=config.http_timeout_s,
            )
        }

    return {
        "web_search": web_search,
        "fetch_url": fetch_url,
    }


def _resolve_config(
    adapter_mode: str | None,
    *,
    trace_live: bool | None = None,
) -> RuntimeAdapterConfig:
    mode = (adapter_mode or os.getenv("AGENTLANG_ADAPTER", "mock")).strip().lower()
    if mode not in {"mock", "live", "anthropic"}:
        raise ExecutionError("Adapter mode must be one of: mock, live, anthropic.")

    default_model = os.getenv("AGENTLANG_DEFAULT_MODEL", "gpt-4.1-mini")
    web_max_results = int(os.getenv("AGENTLANG_WEB_RESULTS", "5"))
    http_timeout_s = float(os.getenv("AGENTLANG_HTTP_TIMEOUT_S", "20"))
    if trace_live is None:
        trace_live = _is_truthy_env(os.getenv("AGENTLANG_TRACE_LIVE", "0"))
    return RuntimeAdapterConfig(
        mode=mode,
        default_model=default_model,
        web_max_results=web_max_results,
        http_timeout_s=http_timeout_s,
        trace_live=trace_live,
    )


def _build_llm_client(config: RuntimeAdapterConfig) -> LLMClient | None:
    if config.mode == "live":
        api_key = os.getenv("OPENAI_API_KEY")
        if not api_key:
            raise ExecutionError("OPENAI_API_KEY is required when adapter mode is 'live'.")
        base_url = os.getenv("OPENAI_BASE_URL", "https://api.openai.com/v1")
        return OpenAIResponsesClient(
            api_key=api_key,
            base_url=base_url,
            timeout_s=config.http_timeout_s,
        )

    if config.mode == "anthropic":
        api_key = os.getenv("ANTHROPIC_API_KEY")
        if not api_key:
            raise ExecutionError("ANTHROPIC_API_KEY is required when adapter mode is 'anthropic'.")
        base_url = os.getenv("ANTHROPIC_BASE_URL", "https://api.anthropic.com")
        return AnthropicMessagesClient(
            api_key=api_key,
            base_url=base_url,
            timeout_s=config.http_timeout_s,
        )

    return None


def _resolve_model(agent_def: AgentDef | None, default_model: str, config: RuntimeAdapterConfig | None = None) -> str:
    model = default_model if (agent_def is None or agent_def.model is None) else agent_def.model
    if config is not None and config.mode == "anthropic":
        if model.startswith("claude-"):
            return model
        return _OPENAI_TO_ANTHROPIC_MODELS.get(model, model)
    return model


def _run_web_search(query: str, *, max_results: int, timeout_s: float) -> list[dict[str, str]]:
    try:
        return duckduckgo_search(query, max_results=max_results, timeout_s=timeout_s)
    except ToolAdapterError as exc:
        raise ExecutionError(f"Tool 'web_search' failed: {exc}") from exc


def _run_fetch_url(url: str, *, timeout_s: float) -> str:
    try:
        return fetch_url_text(url, timeout_s=timeout_s)
    except ToolAdapterError as exc:
        raise ExecutionError(f"Tool 'fetch_url' failed: {exc}") from exc


def _complete_live(
    client: LLMClient | None,
    *,
    model: str,
    prompt: str,
    system: str | None,
    max_output_tokens: int = 700,
    trace: Callable[[str], None] | None = None,
) -> str:
    if client is None:
        raise ExecutionError("LLM client was not initialized.")
    try:
        return client.complete(
            model=model,
            prompt=prompt,
            system=system,
            max_output_tokens=max_output_tokens,
            trace=trace,
        )
    except (OpenAIAdapterError, AnthropicAdapterError) as exc:
        raise ExecutionError(f"LLM call failed: {exc}") from exc


def _complete_live_with_tools(
    client: LLMClient | None,
    *,
    model: str,
    prompt: str,
    system: str | None,
    tools: list[dict[str, Any]],
    tool_executor: Callable[[str, dict[str, Any]], Any],
    max_output_tokens: int = 700,
    trace: Callable[[str], None] | None = None,
) -> str:
    if client is None:
        raise ExecutionError("LLM client was not initialized.")
    try:
        return client.complete_with_tools(
            model=model,
            prompt=prompt,
            system=system,
            tools=tools,
            call_tool=tool_executor,
            max_output_tokens=max_output_tokens,
            trace=trace,
        )
    except (OpenAIAdapterError, AnthropicAdapterError) as exc:
        raise ExecutionError(f"LLM call failed: {exc}") from exc


def _make_agent_task_handler(
    program: Program,
    task: TaskDef,
    *,
    config: RuntimeAdapterConfig,
    resolve_agent: Callable[[str | None], AgentDef | None],
    client: LLMClient | None,
    tool_registry: dict[str, ToolHandler],
) -> TaskHandler:
    def handler(args: dict[str, Any], agent: str | None) -> Any:
        if agent is None:
            raise ExecutionError(
                f"Agent task '{task.name}' requires a bound agent at runtime."
            )

        if config.mode not in _LIVE_MODES:
            return _mock_value_for_type(
                task.return_type,
                label=f"{agent}:{task.name}",
                seed_args=args,
            )

        agent_def = resolve_agent(agent)
        model = _resolve_model(agent_def, config.default_model, config)
        tools = _agent_tool_definitions(program, agent_def)
        trace = _start_live_trace(
            config,
            task_name=task.name,
            agent=agent,
            model=model,
            args=args,
        )
        system = (
            "You are executing a typed AgentLang task. "
            "Use tools when helpful. Return only valid compact JSON for the declared return schema."
        )
        prompt = (
            f"Task name: {task.name}\n"
            f"Task inputs:\n{json.dumps(args, indent=2, sort_keys=True)}\n\n"
            f"Return schema:\n{json.dumps(_type_to_json_schema(task.return_type), indent=2)}\n\n"
            "Return only minified JSON on a single line. "
            "Do not include markdown fences, commentary, or pretty-printing whitespace."
        )

        if tools:
            raw = _complete_live_with_tools(
                client,
                model=model,
                prompt=prompt,
                system=system,
                tools=tools,
                tool_executor=_trace_tool_executor(
                    config,
                    task_name=task.name,
                    agent=agent,
                    executor=lambda name, call_args: execute_tool(
                        program,
                        name,
                        call_args,
                        tool_registry,
                    ),
                ),
                max_output_tokens=1800,
                trace=trace,
            )
        else:
            raw = _complete_live(
                client,
                model=model,
                system=system,
                prompt=prompt,
                max_output_tokens=1800,
                trace=trace,
            )
        trace(f"raw={_preview_value(raw)}")

        raw = raw.strip()
        if raw.startswith("```"):
            raw = raw.split("\n", 1)[1] if "\n" in raw else raw[3:]
        if raw.endswith("```"):
            raw = raw[:-3].strip()

        try:
            result = json.loads(raw)
        except json.JSONDecodeError:
            start = raw.find("{")
            end = raw.rfind("}")
            if start != -1 and end != -1 and end > start:
                try:
                    result = json.loads(raw[start:end + 1])
                except json.JSONDecodeError as exc:
                    trace(f"error=non-json-output raw={_preview_value(raw)}")
                    raise ExecutionError(
                        f"Agent task '{task.name}' returned non-JSON output: {raw!r}"
                    ) from exc
            else:
                trace(f"error=non-json-output raw={_preview_value(raw)}")
                raise ExecutionError(
                    f"Agent task '{task.name}' returned non-JSON output: {raw!r}"
                )
        trace(f"result={_preview_value(result)}")
        return result

    return handler


def _start_live_trace(
    config: RuntimeAdapterConfig,
    *,
    task_name: str,
    agent: str | None,
    model: str,
    args: dict[str, Any],
) -> Callable[[str], None]:
    prefix = (
        f"task={task_name} agent={agent or 'default-agent'} "
        f"model={model}"
    )
    _trace(config, f"{prefix} start args={_preview_value(args)}")
    return lambda message: _trace(config, f"{prefix} {message}")


def _trace_tool_executor(
    config: RuntimeAdapterConfig,
    *,
    task_name: str,
    agent: str | None,
    executor: Callable[[str, dict[str, Any]], Any],
) -> Callable[[str, dict[str, Any]], Any]:
    agent_name = agent or "default-agent"

    def traced(name: str, call_args: dict[str, Any]) -> Any:
        _trace(
            config,
            f"task={task_name} agent={agent_name} tool={name} call args={_preview_value(call_args)}",
        )
        try:
            result = executor(name, call_args)
        except Exception as exc:  # noqa: BLE001 - tracing should not alter task semantics
            _trace(
                config,
                f"task={task_name} agent={agent_name} tool={name} error={type(exc).__name__}: {exc}",
            )
            raise
        _trace(
            config,
            f"task={task_name} agent={agent_name} tool={name} result={_preview_value(result)}",
        )
        return result

    return traced


def _trace(config: RuntimeAdapterConfig, message: str) -> None:
    if config.mode not in _LIVE_MODES or not config.trace_live:
        return
    print(f"[trace] {message}", file=sys.stderr)


def _preview_value(value: Any, *, limit: int = 200) -> str:
    try:
        rendered = json.dumps(value, ensure_ascii=True, sort_keys=True)
    except TypeError:
        rendered = repr(value)
    compact = " ".join(rendered.split())
    if len(compact) <= limit:
        return compact
    return compact[: limit - 3] + "..."


def _is_truthy_env(value: str) -> bool:
    return value.strip().lower() in {"1", "true", "yes", "on"}


def _agent_tool_definitions(program: Program, agent_def: AgentDef | None) -> list[dict[str, Any]]:
    if agent_def is None:
        return []

    tool_defs: list[dict[str, Any]] = []
    for tool_name in agent_def.tools:
        tool = program.tools.get(tool_name)
        if tool is None:
            continue
        tool_defs.append(
            {
                "type": "function",
                "name": tool.name,
                "description": f"Execute the '{tool.name}' tool declared in AgentLang.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        param.name: _type_to_json_schema(param.type_expr)
                        for param in tool.params
                    },
                    "required": [param.name for param in tool.params],
                    "additionalProperties": False,
                },
            }
        )
    return tool_defs


def _type_to_json_schema(type_expr: TypeExpr) -> dict[str, Any]:
    if isinstance(type_expr, PrimitiveType):
        if type_expr.name == "String":
            return {"type": "string"}
        if type_expr.name == "Number":
            return {"type": "number"}
        if type_expr.name == "Bool":
            return {"type": "boolean"}
        raise ExecutionError(f"Unsupported primitive type '{type_expr.name}'.")

    if isinstance(type_expr, ListType):
        return {
            "type": "array",
            "items": _type_to_json_schema(type_expr.item_type),
        }

    if isinstance(type_expr, ObjType):
        return {
            "type": "object",
            "properties": {
                field: _type_to_json_schema(field_type)
                for field, field_type in type_expr.fields.items()
            },
            "required": list(type_expr.fields),
            "additionalProperties": False,
        }

    if isinstance(type_expr, OptionType):
        schema = _type_to_json_schema(type_expr.item_type)
        schema_type = schema.get("type")
        if isinstance(schema_type, list):
            return {**schema, "type": [*schema_type, "null"]}
        if isinstance(schema_type, str):
            return {**schema, "type": [schema_type, "null"]}
        return {"anyOf": [schema, {"type": "null"}]}

    raise ExecutionError(f"Unsupported type for JSON schema conversion: {type_expr}.")


def _is_review_type(type_expr: TypeExpr) -> bool:
    if not isinstance(type_expr, ObjType):
        return False
    approved = type_expr.fields.get("approved")
    feedback = type_expr.fields.get("feedback")
    return (
        isinstance(approved, PrimitiveType)
        and approved.name == "Bool"
        and isinstance(feedback, PrimitiveType)
        and feedback.name == "String"
    )


def _mock_value_for_type(
    type_expr: TypeExpr,
    *,
    label: str,
    seed_args: dict[str, Any],
) -> Any:
    if isinstance(type_expr, PrimitiveType):
        if type_expr.name == "String":
            if "topic" in seed_args:
                return f"[{label}] {seed_args['topic']}"
            if "query" in seed_args:
                return f"[{label}] {seed_args['query']}"
            return f"[{label}]"
        if type_expr.name == "Number":
            return 0
        if type_expr.name == "Bool":
            return False
        raise ExecutionError(f"Unsupported primitive type '{type_expr.name}'.")

    if isinstance(type_expr, ListType):
        return []

    if isinstance(type_expr, OptionType):
        return None

    if isinstance(type_expr, ObjType):
        if _is_review_type(type_expr):
            result: dict[str, Any] = {"approved": True, "feedback": "mock approved"}
            for field, field_type in type_expr.fields.items():
                if field not in result:
                    result[field] = _mock_value_for_type(
                        field_type, label=f"{label}.{field}", seed_args=seed_args
                    )
            return result
        return {
            field: _mock_value_for_type(field_type, label=f"{label}.{field}", seed_args=seed_args)
            for field, field_type in type_expr.fields.items()
        }

    raise ExecutionError(f"Unsupported mock generation type: {type_expr}.")
