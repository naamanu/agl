from __future__ import annotations

import json
import threading
import time
from dataclasses import dataclass, field
from typing import Any


@dataclass
class ExecutionContext:
    """Structured observability for AgentLang pipeline execution.

    Thread-safe: all mutations are protected by a lock so that parallel
    branches sharing a single context produce correct traces.
    """

    events: list[dict[str, Any]] = field(default_factory=list)
    _start_times: dict[str, float] = field(default_factory=dict, repr=False)
    _lock: threading.Lock = field(default_factory=threading.Lock, repr=False)
    _id_counter: int = field(default=0, repr=False)

    def _next_id(self) -> int:
        """Return a monotonically increasing ID (must be called under lock)."""
        self._id_counter += 1
        return self._id_counter

    def record_task_start(self, task: str, args: dict[str, Any]) -> str:
        """Record a task start event and return a unique correlation key."""
        with self._lock:
            key = f"task:{task}:{self._next_id()}"
            self._start_times[key] = time.monotonic()
            self.events.append({
                "type": "task_start",
                "task": task,
                "args": _safe_serialize(args),
                "timestamp": time.time(),
                "id": key,
                "_key": key,
            })
            return key

    def record_task_end(self, task: str, result: Any, *, key: str | None = None, duration: float | None = None) -> None:
        with self._lock:
            elapsed = duration
            if elapsed is None and key and key in self._start_times:
                elapsed = time.monotonic() - self._start_times.pop(key)
            self.events.append({
                "type": "task_end",
                "task": task,
                "result": _safe_serialize(result),
                "duration_s": round(elapsed, 4) if elapsed is not None else None,
                "timestamp": time.time(),
                "id": key,
            })

    def record_task_error(self, task: str, error: Exception, *, key: str | None = None) -> None:
        with self._lock:
            # Clean up start time if we have the key
            if key and key in self._start_times:
                self._start_times.pop(key)
            self.events.append({
                "type": "task_error",
                "task": task,
                "error": f"{type(error).__name__}: {error}",
                "timestamp": time.time(),
                "id": key,
            })

    def record_parallel_start(self, branch_count: int) -> None:
        with self._lock:
            self.events.append({
                "type": "parallel_start",
                "branch_count": branch_count,
                "timestamp": time.time(),
            })

    def record_parallel_end(self, branch_count: int) -> None:
        with self._lock:
            self.events.append({
                "type": "parallel_end",
                "branch_count": branch_count,
                "timestamp": time.time(),
            })

    def record_retry(self, task: str, attempt: int, error: Exception, *, key: str | None = None) -> None:
        with self._lock:
            self.events.append({
                "type": "retry",
                "task": task,
                "attempt": attempt,
                "error": f"{type(error).__name__}: {error}",
                "timestamp": time.time(),
                "id": key,
            })

    def record_pipeline_call(self, pipeline_name: str, args: dict[str, Any]) -> None:
        with self._lock:
            self.events.append({
                "type": "pipeline_call",
                "pipeline": pipeline_name,
                "args": _safe_serialize(args),
                "timestamp": time.time(),
            })

    def to_json(self) -> str:
        with self._lock:
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
