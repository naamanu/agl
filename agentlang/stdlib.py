from __future__ import annotations

import json
import os
from dataclasses import dataclass
from threading import Lock
from typing import Any, Callable

from .adapters import (
    OpenAIAdapterError,
    OpenAIResponsesClient,
    ToolAdapterError,
    duckduckgo_search,
    fetch_url_text,
    format_search_hits,
)
from .ast import AgentDef, ListType, ObjType, OptionType, PrimitiveType, Program, TaskDef, TypeExpr
from .runtime import execute_tool

TaskHandler = Callable[[dict[str, Any], str | None], Any]
ToolHandler = Callable[[dict[str, Any]], Any]

_flaky_attempts: dict[str, int] = {}
_flaky_attempts_lock = Lock()


@dataclass(frozen=True)
class RuntimeAdapterConfig:
    mode: str
    default_model: str
    web_max_results: int
    http_timeout_s: float


def default_task_registry(
    program: Program | None = None,
    *,
    adapter_mode: str | None = None,
) -> dict[str, TaskHandler]:
    config = _resolve_config(adapter_mode)
    if program is None:
        raise RuntimeError("default_task_registry requires a parsed program.")

    agent_map = program.agents
    tool_registry = default_tool_registry(adapter_mode=adapter_mode)
    client = _build_openai_client(config)

    def resolve_agent(agent_name: str | None) -> AgentDef | None:
        if agent_name is None:
            return None
        return agent_map.get(agent_name)

    def research(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        topic = str(args["topic"])
        if config.mode == "live":
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model)
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
                    tool_executor=lambda name, call_args: execute_tool(
                        program,
                        name,
                        call_args,
                        tool_registry,
                    ),
                )
            else:
                text = _complete_live(client, model=model, system=system, prompt=prompt)
            return {"notes": text}

        who = agent or "default-agent"
        return {"notes": f"[{who}] key points for '{topic}'"}

    def draft(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        notes = str(args["notes"])
        if config.mode == "live":
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model)
            system = "You write clean drafts for engineering users."
            prompt = (
                "Write a short, structured draft from these notes.\n"
                "Use a title and concise sections.\n\n"
                f"Notes:\n{notes}"
            )
            text = _complete_live(client, model=model, system=system, prompt=prompt)
            return {"article": text}

        who = agent or "default-agent"
        return {"article": f"[{who}] Draft article:\n{notes}"}

    def compare(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        note_a = str(args["note_a"])
        note_b = str(args["note_b"])
        if config.mode == "live":
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model)
            system = "You compare options and return a concrete recommendation."
            prompt = (
                "Compare option A and B. Provide a clear decision and rationale.\n\n"
                f"Option A:\n{note_a}\n\nOption B:\n{note_b}"
            )
            text = _complete_live(client, model=model, system=system, prompt=prompt)
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
        if config.mode == "live":
            agent_def = resolve_agent(agent)
            model = _resolve_model(agent_def, config.default_model)
            prompt = (
                "Write a short support response to the user based on routing data.\n"
                f"Intent: {intent}\nQueue: {queue}"
            )
            text = _complete_live(client, model=model, system=None, prompt=prompt)
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
            raise RuntimeError(
                f"Transient failure for key '{key}' "
                f"({attempts_so_far + 1}/{failures_before_success})"
            )
        who = agent or "default-agent"
        return {"data": f"[{who}] fetched payload for {key}"}

    def llm_complete(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        prompt = str(args["prompt"])
        if config.mode != "live":
            who = agent or "default-agent"
            return {"text": f"[{who}] {prompt}"}
        agent_def = resolve_agent(agent)
        model = _resolve_model(agent_def, config.default_model)
        text = _complete_live(client, model=model, system=None, prompt=prompt)
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
        if config.mode != "live":
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


def _resolve_config(adapter_mode: str | None) -> RuntimeAdapterConfig:
    mode = (adapter_mode or os.getenv("AGENTLANG_ADAPTER", "mock")).strip().lower()
    if mode not in {"mock", "live"}:
        raise RuntimeError("Adapter mode must be one of: mock, live.")

    default_model = os.getenv("AGENTLANG_DEFAULT_MODEL", "gpt-4.1-mini")
    web_max_results = int(os.getenv("AGENTLANG_WEB_RESULTS", "5"))
    http_timeout_s = float(os.getenv("AGENTLANG_HTTP_TIMEOUT_S", "20"))
    return RuntimeAdapterConfig(
        mode=mode,
        default_model=default_model,
        web_max_results=web_max_results,
        http_timeout_s=http_timeout_s,
    )


def _build_openai_client(config: RuntimeAdapterConfig) -> OpenAIResponsesClient | None:
    if config.mode != "live":
        return None

    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        raise RuntimeError("OPENAI_API_KEY is required when adapter mode is 'live'.")

    base_url = os.getenv("OPENAI_BASE_URL", "https://api.openai.com/v1")
    return OpenAIResponsesClient(
        api_key=api_key,
        base_url=base_url,
        timeout_s=config.http_timeout_s,
    )


def _resolve_model(agent_def: AgentDef | None, default_model: str) -> str:
    if agent_def is None:
        return default_model
    return agent_def.model


def _run_web_search(query: str, *, max_results: int, timeout_s: float) -> list[dict[str, str]]:
    try:
        return duckduckgo_search(query, max_results=max_results, timeout_s=timeout_s)
    except ToolAdapterError as exc:
        raise RuntimeError(f"Tool 'web_search' failed: {exc}") from exc


def _run_fetch_url(url: str, *, timeout_s: float) -> str:
    try:
        return fetch_url_text(url, timeout_s=timeout_s)
    except ToolAdapterError as exc:
        raise RuntimeError(f"Tool 'fetch_url' failed: {exc}") from exc


def _complete_live(
    client: OpenAIResponsesClient | None,
    *,
    model: str,
    prompt: str,
    system: str | None,
    max_output_tokens: int = 700,
) -> str:
    if client is None:
        raise RuntimeError("OpenAI client was not initialized.")
    try:
        return client.complete(
            model=model,
            prompt=prompt,
            system=system,
            max_output_tokens=max_output_tokens,
        )
    except OpenAIAdapterError as exc:
        raise RuntimeError(f"LLM call failed: {exc}") from exc


def _complete_live_with_tools(
    client: OpenAIResponsesClient | None,
    *,
    model: str,
    prompt: str,
    system: str | None,
    tools: list[dict[str, Any]],
    tool_executor: Callable[[str, dict[str, Any]], Any],
    max_output_tokens: int = 700,
) -> str:
    if client is None:
        raise RuntimeError("OpenAI client was not initialized.")
    try:
        return client.complete_with_tools(
            model=model,
            prompt=prompt,
            system=system,
            tools=tools,
            call_tool=tool_executor,
            max_output_tokens=max_output_tokens,
        )
    except OpenAIAdapterError as exc:
        raise RuntimeError(f"LLM call failed: {exc}") from exc


def _make_agent_task_handler(
    program: Program,
    task: TaskDef,
    *,
    config: RuntimeAdapterConfig,
    resolve_agent: Callable[[str | None], AgentDef | None],
    client: OpenAIResponsesClient | None,
    tool_registry: dict[str, ToolHandler],
) -> TaskHandler:
    def handler(args: dict[str, Any], agent: str | None) -> Any:
        if agent is None:
            raise RuntimeError(
                f"Agent task '{task.name}' requires a bound agent at runtime."
            )

        if config.mode != "live":
            return _mock_value_for_type(
                task.return_type,
                label=f"{agent}:{task.name}",
                seed_args=args,
            )

        agent_def = resolve_agent(agent)
        model = _resolve_model(agent_def, config.default_model)
        tools = _agent_tool_definitions(program, agent_def)
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
                tool_executor=lambda name, call_args: execute_tool(
                    program,
                    name,
                    call_args,
                    tool_registry,
                ),
                max_output_tokens=1800,
            )
        else:
            raw = _complete_live(
                client,
                model=model,
                system=system,
                prompt=prompt,
                max_output_tokens=1800,
            )

        try:
            return json.loads(raw)
        except json.JSONDecodeError as exc:
            raise RuntimeError(
                f"Agent task '{task.name}' returned non-JSON output: {raw!r}"
            ) from exc

    return handler


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
        raise RuntimeError(f"Unsupported primitive type '{type_expr.name}'.")

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

    raise RuntimeError(f"Unsupported type for JSON schema conversion: {type_expr}.")


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
        raise RuntimeError(f"Unsupported primitive type '{type_expr.name}'.")

    if isinstance(type_expr, ListType):
        return []

    if isinstance(type_expr, OptionType):
        return None

    if isinstance(type_expr, ObjType):
        return {
            field: _mock_value_for_type(field_type, label=f"{label}.{field}", seed_args=seed_args)
            for field, field_type in type_expr.fields.items()
        }

    raise RuntimeError(f"Unsupported mock generation type: {type_expr}.")
