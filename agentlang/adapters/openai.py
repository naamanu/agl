from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any
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
        payload: dict[str, Any] = {
            "model": model,
            "input": self._build_input(prompt=prompt, system=system),
        }
        if max_output_tokens is not None:
            payload["max_output_tokens"] = max_output_tokens

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
                data = json.loads(resp.read().decode("utf-8"))
        except error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")
            raise OpenAIAdapterError(f"OpenAI HTTP {exc.code}: {detail}") from exc
        except error.URLError as exc:
            raise OpenAIAdapterError(f"OpenAI network error: {exc.reason}") from exc
        except TimeoutError as exc:
            raise OpenAIAdapterError("OpenAI request timed out.") from exc
        except json.JSONDecodeError as exc:
            raise OpenAIAdapterError("OpenAI response was not valid JSON.") from exc

        text = _extract_text(data)
        if not text:
            raise OpenAIAdapterError("OpenAI response contained no text output.")
        return text

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

