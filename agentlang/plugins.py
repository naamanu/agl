from __future__ import annotations

import importlib
import sys
from dataclasses import dataclass, field
from typing import Any, Callable


@dataclass
class PluginRegistry:
    """Registry for plugin-provided task and tool handlers."""

    _task_handlers: dict[str, Callable] = field(default_factory=dict)
    _tool_handlers: dict[str, Callable] = field(default_factory=dict)

    def register_task(self, name: str, handler: Callable[[dict[str, Any], str | None], Any]) -> None:
        self._task_handlers[name] = handler

    def register_tool(self, name: str, handler: Callable[[dict[str, Any]], Any]) -> None:
        self._tool_handlers[name] = handler

    def get_task_handlers(self) -> dict[str, Callable]:
        return dict(self._task_handlers)

    def get_tool_handlers(self) -> dict[str, Callable]:
        return dict(self._tool_handlers)


def load_plugin(module_path: str, registry: PluginRegistry) -> None:
    """Load a plugin module and call its register(registry) function."""
    # Support both dotted module names and file paths
    if module_path.endswith(".py"):
        import importlib.util

        spec = importlib.util.spec_from_file_location("_agentlang_plugin", module_path)
        if spec is None or spec.loader is None:
            raise ImportError(f"Cannot load plugin from '{module_path}'.")
        mod = importlib.util.module_from_spec(spec)
        sys.modules["_agentlang_plugin"] = mod
        spec.loader.exec_module(mod)
    else:
        mod = importlib.import_module(module_path)

    if not hasattr(mod, "register"):
        raise AttributeError(f"Plugin module '{module_path}' has no register(registry) function.")

    mod.register(registry)
