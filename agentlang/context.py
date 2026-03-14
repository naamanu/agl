from __future__ import annotations

import json
import time
from dataclasses import dataclass, field
from typing import Any


@dataclass
class ExecutionContext:
    """Structured observability for AgentLang pipeline execution."""

    events: list[dict[str, Any]] = field(default_factory=list)
    _start_times: dict[str, float] = field(default_factory=dict, repr=False)

    def record_task_start(self, task: str, args: dict[str, Any]) -> None:
        key = f"task:{task}:{len(self.events)}"
        self._start_times[key] = time.monotonic()
        self.events.append({
            "type": "task_start",
            "task": task,
            "args": _safe_serialize(args),
            "timestamp": time.time(),
            "_key": key,
        })

    def record_task_end(self, task: str, result: Any, duration: float | None = None) -> None:
        key = None
        for evt in reversed(self.events):
            if evt.get("type") == "task_start" and evt.get("task") == task:
                key = evt.get("_key")
                break
        elapsed = duration
        if elapsed is None and key and key in self._start_times:
            elapsed = time.monotonic() - self._start_times.pop(key)
        self.events.append({
            "type": "task_end",
            "task": task,
            "result": _safe_serialize(result),
            "duration_s": round(elapsed, 4) if elapsed is not None else None,
            "timestamp": time.time(),
        })

    def record_task_error(self, task: str, error: Exception) -> None:
        self.events.append({
            "type": "task_error",
            "task": task,
            "error": f"{type(error).__name__}: {error}",
            "timestamp": time.time(),
        })

    def record_parallel_start(self, branch_count: int) -> None:
        self.events.append({
            "type": "parallel_start",
            "branch_count": branch_count,
            "timestamp": time.time(),
        })

    def record_parallel_end(self, branch_count: int) -> None:
        self.events.append({
            "type": "parallel_end",
            "branch_count": branch_count,
            "timestamp": time.time(),
        })

    def record_retry(self, task: str, attempt: int, error: Exception) -> None:
        self.events.append({
            "type": "retry",
            "task": task,
            "attempt": attempt,
            "error": f"{type(error).__name__}: {error}",
            "timestamp": time.time(),
        })

    def record_pipeline_call(self, pipeline_name: str, args: dict[str, Any]) -> None:
        self.events.append({
            "type": "pipeline_call",
            "pipeline": pipeline_name,
            "args": _safe_serialize(args),
            "timestamp": time.time(),
        })

    def to_json(self) -> str:
        # Strip internal keys from events
        clean_events = []
        for evt in self.events:
            clean = {k: v for k, v in evt.items() if not k.startswith("_")}
            clean_events.append(clean)
        return json.dumps({"trace": clean_events}, indent=2)


def _safe_serialize(value: Any) -> Any:
    """Best-effort serialization for trace output."""
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, dict):
        return {k: _safe_serialize(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [_safe_serialize(item) for item in value]
    return str(value)
