"""External service adapters for AgentLang."""

from .anthropic import AnthropicAdapterError, AnthropicMessagesClient
from .openai import OpenAIAdapterError, OpenAIResponsesClient
from .tools import ToolAdapterError, duckduckgo_search, fetch_url_text, format_search_hits

__all__ = [
    "AnthropicAdapterError",
    "AnthropicMessagesClient",
    "OpenAIAdapterError",
    "OpenAIResponsesClient",
    "ToolAdapterError",
    "duckduckgo_search",
    "fetch_url_text",
    "format_search_hits",
]
