"""External service adapters for AgentLang."""

from .openai import OpenAIAdapterError, OpenAIResponsesClient
from .tools import ToolAdapterError, duckduckgo_search, fetch_url_text, format_search_hits

__all__ = [
    "OpenAIAdapterError",
    "OpenAIResponsesClient",
    "ToolAdapterError",
    "duckduckgo_search",
    "fetch_url_text",
    "format_search_hits",
]
