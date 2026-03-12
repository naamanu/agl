from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Callable
from urllib import error, request


class OpenAIAdapterError(RuntimeError):
    pass


@dataclass(frozen=True)
class OpenAIResponsesClient:
    api_key: str
    base_url: str = "https://api.openai.com/v1"
    timeout_s: float = 45.0

    def complete(
        self,
        *,
        model: str,
        prompt: str,
        system: str | None = None,
        max_output_tokens: int | None = None,
    ) -> str:
        data = self.create_response(
            model=model,
            input_items=self._build_input(prompt=prompt, system=system),
            max_output_tokens=max_output_tokens,
        )
        text = _extract_text(data)
        if not text:
            raise OpenAIAdapterError("OpenAI response contained no text output.")
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
    ) -> str:
        response = self.create_response(
            model=model,
            input_items=self._build_input(prompt=prompt, system=system),
            tools=tools,
            parallel_tool_calls=False,
            max_output_tokens=max_output_tokens,
        )

        for _ in range(max_round_trips):
            function_calls = _extract_function_calls(response)
            if not function_calls:
                text = _extract_text(response)
                if not text:
                    raise OpenAIAdapterError("OpenAI response contained no text output.")
                return text

            output_items: list[dict[str, Any]] = []
            for tool_call in function_calls:
                name = tool_call["name"]
                call_id = tool_call["call_id"]
                arguments_text = tool_call["arguments"]
                try:
                    arguments = json.loads(arguments_text)
                except json.JSONDecodeError as exc:
                    raise OpenAIAdapterError(
                        f"OpenAI tool call for '{name}' returned invalid JSON arguments."
                    ) from exc
                if not isinstance(arguments, dict):
                    raise OpenAIAdapterError(
                        f"OpenAI tool call for '{name}' returned non-object arguments."
                    )
                result = call_tool(name, arguments)
                output_items.append(
                    {
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": json.dumps(result),
                    }
                )

            response = self.create_response(
                model=model,
                input_items=output_items,
                tools=tools,
                previous_response_id=response.get("id"),
                max_output_tokens=max_output_tokens,
            )

        raise OpenAIAdapterError("OpenAI tool-calling loop exceeded max_round_trips.")

    def create_response(
        self,
        *,
        model: str,
        input_items: list[dict[str, Any]],
        tools: list[dict[str, Any]] | None = None,
        previous_response_id: str | None = None,
        parallel_tool_calls: bool | None = None,
        max_output_tokens: int | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "model": model,
            "input": input_items,
        }
        if tools:
            payload["tools"] = tools
        if previous_response_id is not None:
            payload["previous_response_id"] = previous_response_id
        if parallel_tool_calls is not None:
            payload["parallel_tool_calls"] = parallel_tool_calls
        if max_output_tokens is not None:
            payload["max_output_tokens"] = max_output_tokens
        return self._post(payload)

    @staticmethod
    def _build_input(*, prompt: str, system: str | None) -> list[dict[str, Any]]:
        messages: list[dict[str, Any]] = []
        if system:
            messages.append(
                {
                    "role": "system",
                    "content": [{"type": "input_text", "text": system}],
                }
            )
        messages.append(
            {
                "role": "user",
                "content": [{"type": "input_text", "text": prompt}],
            }
        )
        return messages

    def _post(self, payload: dict[str, Any]) -> dict[str, Any]:
        body = json.dumps(payload).encode("utf-8")
        req = request.Request(
            url=f"{self.base_url.rstrip('/')}/responses",
            data=body,
            method="POST",
            headers={
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            },
        )

        try:
            with request.urlopen(req, timeout=self.timeout_s) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")
            raise OpenAIAdapterError(f"OpenAI HTTP {exc.code}: {detail}") from exc
        except error.URLError as exc:
            raise OpenAIAdapterError(f"OpenAI network error: {exc.reason}") from exc
        except TimeoutError as exc:
            raise OpenAIAdapterError("OpenAI request timed out.") from exc
        except json.JSONDecodeError as exc:
            raise OpenAIAdapterError("OpenAI response was not valid JSON.") from exc


def _extract_text(payload: dict[str, Any]) -> str:
    direct = payload.get("output_text")
    if isinstance(direct, str) and direct.strip():
        return direct.strip()

    output = payload.get("output")
    if not isinstance(output, list):
        return ""

    fragments: list[str] = []
    for item in output:
        if not isinstance(item, dict):
            continue
        content = item.get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if not isinstance(block, dict):
                continue
            text = block.get("text")
            if isinstance(text, str) and text.strip():
                fragments.append(text.strip())

    return "\n".join(fragments).strip()


def _extract_function_calls(payload: dict[str, Any]) -> list[dict[str, str]]:
    output = payload.get("output")
    if not isinstance(output, list):
        return []

    function_calls: list[dict[str, str]] = []
    for item in output:
        if not isinstance(item, dict):
            continue
        if item.get("type") != "function_call":
            continue
        name = item.get("name")
        call_id = item.get("call_id")
        arguments = item.get("arguments")
        if all(isinstance(value, str) for value in [name, call_id, arguments]):
            function_calls.append(
                {
                    "name": name,
                    "call_id": call_id,
                    "arguments": arguments,
                }
            )
    return function_calls
