from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any

from .adapters import (
    OpenAIAdapterError,
    OpenAIResponsesClient,
    ToolAdapterError,
    duckduckgo_search,
    format_search_hits,
)
from .ast import AgentDef, Program
from .runtime import AgentRuntimeError, TaskHandler

_flaky_attempts: dict[str, int] = {}


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
    agent_map = program.agents if program is not None else {}
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
            search_context = ""
            if agent_def is not None and "web_search" in agent_def.tools:
                search_context = _run_search_context(
                    topic,
                    max_results=config.web_max_results,
                    timeout_s=config.http_timeout_s,
                )
            system = (
                "You produce factual, concise research notes for downstream agent workflows."
            )
            prompt = (
                f"Topic: {topic}\n\n"
                "If web context is present, prioritize it.\n"
                f"Web context:\n{search_context or 'No external context.'}\n\n"
                "Return 5-8 short bullet points in plain text."
            )
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
        attempts_so_far = _flaky_attempts.get(key, 0)
        if attempts_so_far < failures_before_success:
            _flaky_attempts[key] = attempts_so_far + 1
            raise AgentRuntimeError(
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

    return {
        "research": research,
        "draft": draft,
        "compare": compare,
        "extract_intent": extract_intent,
        "route": route,
        "respond": respond,
        "flaky_fetch": flaky_fetch,
        "llm_complete": llm_complete,
    }


def _resolve_config(adapter_mode: str | None) -> RuntimeAdapterConfig:
    mode = (adapter_mode or os.getenv("AGENTLANG_ADAPTER", "mock")).strip().lower()
    if mode not in {"mock", "live"}:
        raise AgentRuntimeError("Adapter mode must be one of: mock, live.")

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
        raise AgentRuntimeError("OPENAI_API_KEY is required when adapter mode is 'live'.")

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


def _run_search_context(topic: str, *, max_results: int, timeout_s: float) -> str:
    try:
        hits = duckduckgo_search(topic, max_results=max_results, timeout_s=timeout_s)
    except ToolAdapterError as exc:
        return f"Search unavailable: {exc}"
    return format_search_hits(hits)


def _complete_live(
    client: OpenAIResponsesClient | None,
    *,
    model: str,
    prompt: str,
    system: str | None,
) -> str:
    if client is None:
        raise AgentRuntimeError("OpenAI client was not initialized.")
    try:
        return client.complete(model=model, prompt=prompt, system=system, max_output_tokens=700)
    except OpenAIAdapterError as exc:
        raise AgentRuntimeError(f"LLM call failed: {exc}") from exc

