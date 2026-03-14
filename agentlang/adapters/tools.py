from __future__ import annotations

import json
import re
from html import unescape
from urllib import error, parse, request


class ToolAdapterError(RuntimeError):
    pass


def duckduckgo_search(
    query: str,
    *,
    max_results: int = 5,
    timeout_s: float = 15.0,
) -> list[dict[str, str]]:
    """Search DuckDuckGo via its HTML results page.

    The Instant Answer JSON API returns empty results for most queries.
    Scraping the lite HTML page gives real organic search results.
    """
    params = parse.urlencode({"q": query})
    url = f"https://html.duckduckgo.com/html/?{params}"

    req = request.Request(
        url=url,
        method="GET",
        headers={
            "User-Agent": "Mozilla/5.0 (compatible; AgentLang/0.1)",
        },
    )
    try:
        with request.urlopen(req, timeout=timeout_s) as resp:
            html = resp.read().decode("utf-8", errors="replace")
    except error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise ToolAdapterError(f"DuckDuckGo HTTP {exc.code}: {detail}") from exc
    except error.URLError as exc:
        raise ToolAdapterError(f"DuckDuckGo network error: {exc.reason}") from exc
    except TimeoutError as exc:
        raise ToolAdapterError("DuckDuckGo request timed out.") from exc

    return _parse_ddg_html(html, max_results)


def _parse_ddg_html(html: str, max_results: int) -> list[dict[str, str]]:
    """Extract search results from DuckDuckGo lite HTML."""
    results: list[dict[str, str]] = []

    # Match <a ... class="result__a" ... href="..." ...>title</a>
    # Attributes can appear in any order.
    link_pattern = re.compile(
        r'<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>(.*?)</a>',
        re.DOTALL,
    )
    # Also try href before class
    link_pattern_alt = re.compile(
        r'<a[^>]*href="([^"]*)"[^>]*class="result__a"[^>]*>(.*?)</a>',
        re.DOTALL,
    )
    snippet_pattern = re.compile(
        r'<a[^>]*class="result__snippet"[^>]*>(.*?)</a>',
        re.DOTALL,
    )

    links = link_pattern.findall(html) or link_pattern_alt.findall(html)
    snippets = snippet_pattern.findall(html)

    for i, (raw_url, raw_title) in enumerate(links):
        if len(results) >= max_results:
            break
        title = _strip_tags(unescape(raw_title)).strip()
        url = _extract_url(raw_url)
        snippet = ""
        if i < len(snippets):
            snippet = _strip_tags(unescape(snippets[i])).strip()
        if not title and not snippet:
            continue
        results.append({"title": title or "Untitled", "url": url, "snippet": snippet})

    return results


def _extract_url(raw: str) -> str:
    """DuckDuckGo wraps links in a redirect; extract the actual URL."""
    if "uddg=" in raw:
        match = re.search(r"uddg=([^&]+)", raw)
        if match:
            return parse.unquote(match.group(1))
    return raw


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


def fetch_url_text(
    url: str,
    *,
    timeout_s: float = 15.0,
    max_bytes: int = 50000,
) -> str:
    req = request.Request(
        url=url,
        method="GET",
        headers={"User-Agent": "AgentLang/0.1 (+https://nanamanu.com/agl)"},
    )
    try:
        with request.urlopen(req, timeout=timeout_s) as resp:
            raw = resp.read(max_bytes)
    except error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise ToolAdapterError(f"fetch_url HTTP {exc.code}: {detail}") from exc
    except error.URLError as exc:
        raise ToolAdapterError(f"fetch_url network error: {exc.reason}") from exc
    except TimeoutError as exc:
        raise ToolAdapterError("fetch_url request timed out.") from exc

    text = raw.decode("utf-8", errors="replace")
    return _strip_tags(text)


def _strip_tags(text: str) -> str:
    text = re.sub(r"(?is)<script.*?>.*?</script>", " ", text)
    text = re.sub(r"(?is)<style.*?>.*?</style>", " ", text)
    text = re.sub(r"(?s)<[^>]+>", " ", text)
    text = re.sub(r"\s+", " ", text)
    return text.strip()
