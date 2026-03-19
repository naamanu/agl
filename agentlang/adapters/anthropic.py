from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Callable
from urllib import error, request


class AnthropicAdapterError(RuntimeError):
    pass


@dataclass(frozen=True)
class AnthropicMessagesClient:
    api_key: str
    base_url: str = "https://api.anthropic.com"
    timeout_s: float = 45.0

    def complete(
        self,
        *,
        model: str,
        prompt: str,
        system: str | None = None,
        max_output_tokens: int | None = None,
        trace: Callable[[str], None] | None = None,
    ) -> str:
        if trace is not None:
            trace(f"anthropic request mode=complete model={model}")
        data = self.create_message(
            model=model,
            system=system,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=max_output_tokens or 1024,
        )
        text = _extract_text(data)
        if not text:
            raise AnthropicAdapterError("Anthropic response contained no text output.")
        if trace is not None:
            trace(f"anthropic response mode=complete text={_preview_text(text)}")
        return text

    def complete_with_tools(
        self,
        *,
        model: str,
        prompt: str,
        system: str | None,
        tools: list[dict[str, Any]],
        call_tool: Callable[[str, dict[str, Any]], Any],
        max_output_tokens: int | None = None,
        max_round_trips: int = 8,
        trace: Callable[[str], None] | None = None,
    ) -> str:
        if trace is not None:
            trace(
                f"anthropic request mode=tool-call model={model} "
                f"tools={','.join(tool['name'] for tool in tools)}"
            )
        max_tokens = max_output_tokens or 1024
        anthropic_tools = [_convert_tool_def(t) for t in tools]
        messages: list[dict[str, Any]] = [{"role": "user", "content": prompt}]

        response = self.create_message(
            model=model,
            system=system,
            messages=messages,
            tools=anthropic_tools,
            max_tokens=max_tokens,
        )

        for _ in range(max_round_trips):
            tool_uses = _extract_tool_uses(response)
            if not tool_uses:
                text = _extract_text(response)
                if not text:
                    raise AnthropicAdapterError("Anthropic response contained no text output.")
                if trace is not None:
                    trace(f"anthropic response mode=tool-call text={_preview_text(text)}")
                return text

            if trace is not None:
                trace(
                    "anthropic tool-calls "
                    + ",".join(tu["name"] for tu in tool_uses)
                )

            # Append the assistant's full response as a message
            messages.append({"role": "assistant", "content": response["content"]})

            # Build tool_result blocks for a single user message
            tool_result_blocks: list[dict[str, Any]] = []
            for tu in tool_uses:
                name = tu["name"]
                tool_id = tu["id"]
                arguments = tu["input"]
                if not isinstance(arguments, dict):
                    raise AnthropicAdapterError(
                        f"Anthropic tool call for '{name}' returned non-object input."
                    )
                result = call_tool(name, arguments)
                tool_result_blocks.append(
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": json.dumps(result),
                    }
                )

            messages.append({"role": "user", "content": tool_result_blocks})

            response = self.create_message(
                model=model,
                system=system,
                messages=messages,
                tools=anthropic_tools,
                max_tokens=max_tokens,
            )

        raise AnthropicAdapterError("Anthropic tool-calling loop exceeded max_round_trips.")

    def create_message(
        self,
        *,
        model: str,
        messages: list[dict[str, Any]],
        system: str | None = None,
        tools: list[dict[str, Any]] | None = None,
        max_tokens: int = 1024,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "model": model,
            "messages": messages,
            "max_tokens": max_tokens,
        }
        if system:
            payload["system"] = system
        if tools:
            payload["tools"] = tools
        return self._post(payload)

    def _post(self, payload: dict[str, Any]) -> dict[str, Any]:
        body = json.dumps(payload).encode("utf-8")
        req = request.Request(
            url=f"{self.base_url.rstrip('/')}/v1/messages",
            data=body,
            method="POST",
            headers={
                "x-api-key": self.api_key,
                "anthropic-version": "2023-06-01",
                "Content-Type": "application/json",
            },
        )

        try:
            with request.urlopen(req, timeout=self.timeout_s) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")
            raise AnthropicAdapterError(f"Anthropic HTTP {exc.code}: {detail}") from exc
        except error.URLError as exc:
            raise AnthropicAdapterError(f"Anthropic network error: {exc.reason}") from exc
        except TimeoutError as exc:
            raise AnthropicAdapterError("Anthropic request timed out.") from exc
        except json.JSONDecodeError as exc:
            raise AnthropicAdapterError("Anthropic response was not valid JSON.") from exc


def _convert_tool_def(tool: dict[str, Any]) -> dict[str, Any]:
    """Convert an OpenAI-style function tool definition to Anthropic format."""
    return {
        "name": tool["name"],
        "description": tool.get("description", f"Execute the '{tool['name']}' tool."),
        "input_schema": tool.get("parameters", {"type": "object", "properties": {}}),
    }


def _extract_text(payload: dict[str, Any]) -> str:
    content = payload.get("content")
    if not isinstance(content, list):
        return ""

    fragments: list[str] = []
    for block in content:
        if not isinstance(block, dict):
            continue
        if block.get("type") == "text":
            text = block.get("text")
            if isinstance(text, str) and text.strip():
                fragments.append(text.strip())

    return "\n".join(fragments).strip()


def _extract_tool_uses(payload: dict[str, Any]) -> list[dict[str, Any]]:
    content = payload.get("content")
    if not isinstance(content, list):
        return []

    tool_uses: list[dict[str, Any]] = []
    for block in content:
        if not isinstance(block, dict):
            continue
        if block.get("type") != "tool_use":
            continue
        name = block.get("name")
        tool_id = block.get("id")
        input_data = block.get("input")
        if isinstance(name, str) and isinstance(tool_id, str):
            tool_uses.append(
                {
                    "name": name,
                    "id": tool_id,
                    "input": input_data if isinstance(input_data, dict) else {},
                }
            )
    return tool_uses


def _preview_text(text: str, limit: int = 180) -> str:
    compact = " ".join(text.split())
    if len(compact) <= limit:
        return compact
    return compact[: limit - 3] + "..."
