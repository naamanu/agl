from __future__ import annotations

import json
from urllib import error, parse, request


class ToolAdapterError(RuntimeError):
    pass


def duckduckgo_search(
    query: str,
    *,
    max_results: int = 5,
    timeout_s: float = 15.0,
) -> list[dict[str, str]]:
    params = parse.urlencode(
        {
            "q": query,
            "format": "json",
            "no_redirect": "1",
            "no_html": "1",
        }
    )
    url = f"https://api.duckduckgo.com/?{params}"

    req = request.Request(url=url, method="GET")
    try:
        with request.urlopen(req, timeout=timeout_s) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
    except error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise ToolAdapterError(f"DuckDuckGo HTTP {exc.code}: {detail}") from exc
    except error.URLError as exc:
        raise ToolAdapterError(f"DuckDuckGo network error: {exc.reason}") from exc
    except TimeoutError as exc:
        raise ToolAdapterError("DuckDuckGo request timed out.") from exc
    except json.JSONDecodeError as exc:
        raise ToolAdapterError("DuckDuckGo response was not valid JSON.") from exc

    results: list[dict[str, str]] = []
    abstract_text = payload.get("AbstractText")
    abstract_url = payload.get("AbstractURL")
    if isinstance(abstract_text, str) and abstract_text.strip():
        results.append(
            {
                "title": payload.get("Heading") or "Abstract",
                "url": abstract_url or "",
                "snippet": abstract_text.strip(),
            }
        )

    for topic in _flatten_related(payload.get("RelatedTopics", [])):
        if len(results) >= max_results:
            break
        text = topic.get("Text")
        first_url = topic.get("FirstURL")
        if isinstance(text, str) and text.strip():
            results.append(
                {
                    "title": _title_from_text(text),
                    "url": first_url if isinstance(first_url, str) else "",
                    "snippet": text.strip(),
                }
            )

    return results[:max_results]


def format_search_hits(hits: list[dict[str, str]]) -> str:
    if not hits:
        return "No search hits."
    lines: list[str] = []
    for idx, hit in enumerate(hits, start=1):
        title = hit.get("title", "Untitled")
        snippet = hit.get("snippet", "")
        url = hit.get("url", "")
        lines.append(f"{idx}. {title} | {url}\n   {snippet}")
    return "\n".join(lines)


def _flatten_related(items: object) -> list[dict[str, object]]:
    if not isinstance(items, list):
        return []
    out: list[dict[str, object]] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        nested = item.get("Topics")
        if isinstance(nested, list):
            out.extend(_flatten_related(nested))
            continue
        out.append(item)
    return out


def _title_from_text(text: str) -> str:
    if " - " in text:
        return text.split(" - ", 1)[0].strip()
    return text[:80].strip()

