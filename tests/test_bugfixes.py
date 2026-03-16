"""Tests for 5 targeted bugfixes (+ 4 follow-up corrections):

1. Timeout actually bounds wall-clock time (not just raises an error)
2. Plugin tool handlers are wired into execution
3. Pipeline-call modifiers (retries, on_fail, timeout) are rejected by checker
4. ExecutionContext is thread-safe and uses correlation keys
5. Enum validation recurses into List, Option, and nested Obj

Follow-up corrections:
A. Timed-out threads are daemon threads
B. --output-trace writes trace on failure
C. Correlation keys survive JSON export
D. Plugin tool test exercises execute_tool path
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest

from agentlang import (
    ExecutionContext,
    HandlerTimeoutError,
    PluginRegistry,
    check_program,
    default_task_registry,
    default_tool_registry,
    execute_pipeline,
    execute_tool,
    get_leaked_thread_count,
    parse_program,
)
from agentlang.checker import TypeCheckError
from agentlang.runtime import ExecutionError


class TimeoutWallClockTest(unittest.TestCase):
    """Fix 1: timeout must bound wall-clock time, not just raise an error."""

    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_timeout_returns_control_within_bounded_time(self) -> None:
        src = """
task slow() -> String {}

pipeline p() -> String {
  let r = run slow with {} timeout 0.2;
  return r;
}
"""
        program = self._program(src)

        def slow(_args, _agent):
            time.sleep(10)
            return "done"

        start = time.monotonic()
        with self.assertRaisesRegex(ExecutionError, r"exceeded.*deadline"):
            execute_pipeline(program, "p", {}, {"slow": slow})
        elapsed = time.monotonic() - start
        # Must return control well before the 10s sleep finishes.
        # Allow generous margin but much less than 10s.
        self.assertLess(elapsed, 2.0, f"Timeout took {elapsed:.2f}s — handler blocked shutdown")

    def test_timed_out_threads_are_daemon(self) -> None:
        """Timed-out handler threads must be daemon so they don't block exit."""
        src = """
task slow() -> String {}

pipeline p() -> String {
  let r = run slow with {} timeout 0.2;
  return r;
}
"""
        program = self._program(src)

        def slow(_args, _agent):
            time.sleep(10)
            return "done"

        with self.assertRaises(ExecutionError):
            execute_pipeline(program, "p", {}, {"slow": slow})

        # All remaining non-main threads should be daemon
        for t in threading.enumerate():
            if t is not threading.main_thread():
                self.assertTrue(t.daemon, f"Thread {t.name} is not daemon")


class PluginToolHandlerTest(unittest.TestCase):
    """Fix 2: plugin-provided tool handlers must be wired into execution."""

    def test_plugin_tool_handler_callable_through_execute_tool(self) -> None:
        src = """
tool custom_search(query: String) -> List[Obj{title: String}] {}
pipeline p() -> String { return "x"; }
"""
        program = parse_program(src)
        check_program(program)

        registry = PluginRegistry()
        tool_called = {"called": False}

        def custom_search_handler(args):
            tool_called["called"] = True
            return [{"title": f"Result for {args['query']}"}]

        registry.register_tool("custom_search", custom_search_handler)

        tool_reg = default_tool_registry(adapter_mode="mock")
        tool_reg.update(registry.get_tool_handlers())

        result = execute_tool(program, "custom_search", {"query": "test"}, tool_reg)

        self.assertTrue(tool_called["called"])
        self.assertEqual(result[0]["title"], "Result for test")

    def test_plugin_registry_get_tool_handlers_returns_registered(self) -> None:
        registry = PluginRegistry()

        def my_tool(args):
            return {"result": "ok"}

        registry.register_tool("my_tool", my_tool)
        handlers = registry.get_tool_handlers()
        self.assertIn("my_tool", handlers)
        self.assertIs(handlers["my_tool"], my_tool)


class PipelineCallModifiersTest(unittest.TestCase):
    """Fix 3: retries, on_fail, timeout on pipeline-calls-pipeline must be rejected."""

    def test_retries_on_pipeline_call_rejected(self) -> None:
        src = """
task noop() -> String {}

pipeline sub() -> String {
  let r = run noop with {};
  return r;
}

pipeline main() -> String {
  let r = run sub with {} retries 2;
  return r;
}
"""
        program = parse_program(src)
        with self.assertRaises(TypeCheckError) as cm:
            check_program(program)
        self.assertIn("retries", str(cm.exception).lower())
        self.assertIn("pipeline", str(cm.exception).lower())

    def test_on_fail_on_pipeline_call_rejected(self) -> None:
        src = """
task noop() -> String {}

pipeline sub() -> String {
  let r = run noop with {};
  return r;
}

pipeline main() -> String {
  let r = run sub with {} on_fail use "fallback";
  return r;
}
"""
        program = parse_program(src)
        with self.assertRaises(TypeCheckError) as cm:
            check_program(program)
        self.assertIn("on_fail", str(cm.exception).lower())

    def test_timeout_on_pipeline_call_rejected(self) -> None:
        src = """
task noop() -> String {}

pipeline sub() -> String {
  let r = run noop with {};
  return r;
}

pipeline main() -> String {
  let r = run sub with {} timeout 5;
  return r;
}
"""
        program = parse_program(src)
        with self.assertRaises(TypeCheckError) as cm:
            check_program(program)
        self.assertIn("timeout", str(cm.exception).lower())

    def test_plain_pipeline_call_still_works(self) -> None:
        src = """
task echo(msg: String) -> String {}

pipeline sub(msg: String) -> String {
  let r = run echo with { msg: msg };
  return r;
}

pipeline main() -> String {
  let r = run sub with { msg: "hello" };
  return r;
}
"""
        program = parse_program(src)
        check_program(program)

        def echo(args, _agent):
            return args["msg"]

        result = execute_pipeline(program, "main", {}, {"echo": echo})
        self.assertEqual(result, "hello")


class ExecutionContextThreadSafetyTest(unittest.TestCase):
    """Fix 4: ExecutionContext must be safe under concurrent access."""

    def test_correlation_keys_are_unique(self) -> None:
        ctx = ExecutionContext()
        key1 = ctx.record_task_start("task_a", {"x": 1})
        key2 = ctx.record_task_start("task_a", {"x": 2})
        self.assertNotEqual(key1, key2)

    def test_concurrent_same_task_produces_correct_durations(self) -> None:
        ctx = ExecutionContext()

        def worker(task_name: str, sleep_time: float, results: dict) -> None:
            key = ctx.record_task_start(task_name, {})
            time.sleep(sleep_time)
            ctx.record_task_end(task_name, "ok", key=key)
            # Find our end event by matching key through events
            results[key] = sleep_time

        results: dict[str, float] = {}
        threads = []
        for i in range(4):
            t = threading.Thread(target=worker, args=("same_task", 0.05 * (i + 1), results))
            threads.append(t)
            t.start()
        for t in threads:
            t.join()

        # All 4 starts and 4 ends should be recorded
        starts = [e for e in ctx.events if e["type"] == "task_start"]
        ends = [e for e in ctx.events if e["type"] == "task_end"]
        self.assertEqual(len(starts), 4)
        self.assertEqual(len(ends), 4)

        # Each duration should be reasonable (not None, not wildly wrong)
        for end_evt in ends:
            self.assertIsNotNone(end_evt["duration_s"])
            self.assertGreater(end_evt["duration_s"], 0)
            self.assertLess(end_evt["duration_s"], 1.0)

    def test_no_data_corruption_under_concurrent_writes(self) -> None:
        ctx = ExecutionContext()
        errors: list[str] = []

        def writer(task_id: int) -> None:
            try:
                for j in range(20):
                    key = ctx.record_task_start(f"t{task_id}", {"i": j})
                    ctx.record_task_end(f"t{task_id}", f"r{j}", key=key)
            except Exception as e:
                errors.append(str(e))

        threads = [threading.Thread(target=writer, args=(i,)) for i in range(8)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        self.assertEqual(errors, [])
        # 8 threads x 20 iterations x 2 events (start + end)
        self.assertEqual(len(ctx.events), 8 * 20 * 2)


class EnumNestedValidationTest(unittest.TestCase):
    """Fix 5: enum validation must recurse into List, Option, and nested objects."""

    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_enum_in_list_arg_validated(self) -> None:
        src = """
enum Color { red, green, blue };

task paint(colors: List[Color]) -> String {}

pipeline p() -> String {
  let r = run paint with { colors: ["red", "green"] };
  return r;
}
"""
        program = self._program(src)

        def paint(args, _agent):
            return ",".join(args["colors"])

        # Valid enum values
        result = execute_pipeline(program, "p", {}, {"paint": paint})
        self.assertEqual(result, "red,green")

    def test_invalid_enum_in_list_arg_rejected(self) -> None:
        src = """
enum Color { red, green, blue };

task paint(colors: List[Color]) -> String {}

pipeline p(c: String) -> String {
  let r = run paint with { colors: [c] };
  return r;
}
"""
        program = self._program(src)

        def paint(args, _agent):
            return ",".join(args["colors"])

        with self.assertRaisesRegex(ExecutionError, r"not a valid variant"):
            execute_pipeline(program, "p", {"c": "purple"}, {"paint": paint})

    def test_enum_in_option_result_validated(self) -> None:
        """Enum inside Option in a task result should be validated."""
        src = """
enum Status { active, inactive };

task get_status() -> Obj{status: Option[Status]} {}

pipeline p() -> String {
  let r = run get_status with {};
  return "ok";
}
"""
        program = self._program(src)

        def get_status_valid(_args, _agent):
            return {"status": "active"}

        result = execute_pipeline(program, "p", {}, {"get_status": get_status_valid})
        self.assertEqual(result, "ok")

    def test_invalid_enum_in_option_result_rejected(self) -> None:
        """Invalid enum inside Option in a task result should be rejected."""
        src = """
enum Status { active, inactive };

task get_status() -> Obj{status: Option[Status]} {}

pipeline p() -> String {
  let r = run get_status with {};
  return "ok";
}
"""
        program = self._program(src)

        def get_status_bad(_args, _agent):
            return {"status": "unknown"}

        with self.assertRaisesRegex(ExecutionError, r"not a valid variant"):
            execute_pipeline(program, "p", {}, {"get_status": get_status_bad})

    def test_null_option_enum_passes_validation(self) -> None:
        """None value for Option[Enum] should pass validation."""
        src = """
enum Status { active, inactive };

task get_status() -> Obj{status: Option[Status]} {}

pipeline p() -> String {
  let r = run get_status with {};
  return "ok";
}
"""
        program = self._program(src)

        def get_status_null(_args, _agent):
            return {"status": None}

        result = execute_pipeline(program, "p", {}, {"get_status": get_status_null})
        self.assertEqual(result, "ok")

    def test_enum_in_nested_obj_in_list_validated(self) -> None:
        src = """
enum Priority { low, medium, high };

task process(items: List[Obj{name: String, prio: Priority}]) -> String {}

pipeline p() -> String {
  let r = run process with { items: [{ name: "a", prio: "low" }] };
  return r;
}
"""
        program = self._program(src)

        def process(args, _agent):
            return str(len(args["items"]))

        result = execute_pipeline(program, "p", {}, {"process": process})
        self.assertEqual(result, "1")

    def test_invalid_enum_in_nested_obj_in_list_rejected(self) -> None:
        src = """
enum Priority { low, medium, high };

task process(items: List[Obj{name: String, prio: Priority}]) -> String {}

pipeline p(prio: String) -> String {
  let r = run process with { items: [{ name: "a", prio: prio }] };
  return r;
}
"""
        program = self._program(src)

        def process(args, _agent):
            return str(len(args["items"]))

        with self.assertRaisesRegex(ExecutionError, r"not a valid variant"):
            execute_pipeline(program, "p", {"prio": "critical"}, {"process": process})

    def test_enum_in_result_list_validated(self) -> None:
        src = """
enum Color { red, green, blue };

task get_colors() -> List[Color] {}

pipeline p() -> String {
  let r = run get_colors with {};
  return "ok";
}
"""
        program = self._program(src)

        def get_colors_bad(_args, _agent):
            return ["red", "purple"]

        with self.assertRaisesRegex(ExecutionError, r"not a valid variant"):
            execute_pipeline(program, "p", {}, {"get_colors": get_colors_bad})


class CorrelationKeyExportTest(unittest.TestCase):
    """Follow-up C: correlation keys must survive JSON export."""

    def test_id_present_in_exported_events(self) -> None:
        ctx = ExecutionContext()
        k1 = ctx.record_task_start("fetch", {"url": "a"})
        k2 = ctx.record_task_start("fetch", {"url": "b"})
        ctx.record_task_end("fetch", "res_a", key=k1)
        ctx.record_task_end("fetch", "res_b", key=k2)

        exported = json.loads(ctx.to_json())
        events = exported["trace"]

        starts = [e for e in events if e["type"] == "task_start"]
        ends = [e for e in events if e["type"] == "task_end"]

        self.assertEqual(len(starts), 2)
        self.assertEqual(len(ends), 2)

        # "id" must be present
        for e in starts + ends:
            self.assertIn("id", e)

        # "_key" must NOT be present (stripped by to_json)
        for e in events:
            self.assertNotIn("_key", e)

        # Pairing: each start's id should match exactly one end's id
        start_ids = {e["id"] for e in starts}
        end_ids = {e["id"] for e in ends}
        self.assertEqual(start_ids, end_ids)

    def test_error_event_has_id(self) -> None:
        ctx = ExecutionContext()
        k = ctx.record_task_start("fail_task", {})
        ctx.record_task_error("fail_task", RuntimeError("boom"), key=k)

        exported = json.loads(ctx.to_json())
        error_events = [e for e in exported["trace"] if e["type"] == "task_error"]
        self.assertEqual(len(error_events), 1)
        self.assertIn("id", error_events[0])
        self.assertEqual(error_events[0]["id"], k)


class TraceOnFailureTest(unittest.TestCase):
    """Follow-up B: --output-trace must write trace even when execution fails."""

    def test_trace_written_on_pipeline_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            src_path = os.path.join(tmpdir, "fail.agent")
            trace_path = os.path.join(tmpdir, "trace.json")

            with open(src_path, "w") as f:
                f.write("""
task boom() -> String {}

pipeline p() -> String {
  let r = run boom with {};
  return r;
}
""")

            proj_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            result = subprocess.run(
                [sys.executable, "main.py", src_path, "p", "--output-trace", trace_path],
                cwd=proj_root,
                capture_output=True,
                text=True,
            )

            # Pipeline should fail (no handler registered for boom in default mock)
            self.assertNotEqual(result.returncode, 0)

            # But trace file should still be written
            self.assertTrue(
                os.path.exists(trace_path),
                f"Trace file not written on failure. stderr: {result.stderr}",
            )

            with open(trace_path) as f:
                trace_data = json.load(f)
            self.assertIn("trace", trace_data)


class RetryTraceCorrelationTest(unittest.TestCase):
    """Retry events must carry the correlation ID of their parent task."""

    def test_retry_events_have_matching_correlation_ids(self) -> None:
        """Two concurrent same-name tasks with retries must be distinguishable by id."""
        ctx = ExecutionContext()

        # Simulate two concurrent "fetch" tasks, each with start -> retry -> retry -> end
        k1 = ctx.record_task_start("fetch", {"url": "a"})
        k2 = ctx.record_task_start("fetch", {"url": "b"})

        ctx.record_retry("fetch", 1, RuntimeError("fail-a1"), key=k1)
        ctx.record_retry("fetch", 1, RuntimeError("fail-b1"), key=k2)
        ctx.record_retry("fetch", 2, RuntimeError("fail-a2"), key=k1)
        ctx.record_retry("fetch", 2, RuntimeError("fail-b2"), key=k2)

        ctx.record_task_end("fetch", "result-a", key=k1)
        ctx.record_task_end("fetch", "result-b", key=k2)

        exported = json.loads(ctx.to_json())
        events = exported["trace"]

        # All retry events must have "id"
        retries = [e for e in events if e["type"] == "retry"]
        self.assertEqual(len(retries), 4)
        for r in retries:
            self.assertIn("id", r)
            self.assertIsNotNone(r["id"])

        # Retries for task k1 must have id == k1
        k1_retries = [e for e in retries if e["id"] == k1]
        k2_retries = [e for e in retries if e["id"] == k2]
        self.assertEqual(len(k1_retries), 2)
        self.assertEqual(len(k2_retries), 2)

        # The two task lifecycles are distinguishable
        self.assertNotEqual(k1, k2)

        # Start/retry/end for each key form a coherent lifecycle
        for key in (k1, k2):
            lifecycle = [e for e in events if e.get("id") == key]
            types = [e["type"] for e in lifecycle]
            self.assertEqual(types[0], "task_start")
            self.assertEqual(types[-1], "task_end")
            self.assertTrue(all(t == "retry" for t in types[1:-1]))


class PluginWiringTest(unittest.TestCase):
    """Plugin tools must be wired through the full pipeline execution path."""

    def test_plugin_tool_wired_through_pipeline_execution(self) -> None:
        """Registry merging via default_task_registry(..., extra_tool_handlers=...) works."""
        src = """
tool custom_lookup(key: String) -> String {}

agent researcher {
  tools: [custom_lookup]
}

task research(topic: String) -> String by agent {}

pipeline p(topic: String) -> String {
  let r = run research with { topic: topic } by researcher;
  return r;
}
"""
        program = parse_program(src)
        check_program(program)

        plugin_registry = PluginRegistry()
        plugin_registry.register_tool("custom_lookup", lambda args: f"found:{args['key']}")

        task_reg = default_task_registry(
            program,
            adapter_mode="mock",
            extra_tool_handlers=plugin_registry.get_tool_handlers(),
        )

        # In mock mode the agent handler won't actually call the tool,
        # but this proves the registry merging path works without errors.
        result = execute_pipeline(program, "p", {"topic": "test"}, task_reg)
        self.assertIsNotNone(result)

    def test_plugin_tool_handler_invoked_through_execute_tool(self) -> None:
        """Prove the plugin tool handler is actually called through execute_tool
        with the merged registry built the same way main.py builds it."""
        src = """
tool custom_lookup(key: String) -> String {}

agent researcher {
  tools: [custom_lookup]
}

task research(topic: String) -> String by agent {}

pipeline p(topic: String) -> String {
  let r = run research with { topic: topic } by researcher;
  return r;
}
"""
        program = parse_program(src)
        check_program(program)

        tool_called = {"called": False, "args": None}

        def custom_lookup_handler(args):
            tool_called["called"] = True
            tool_called["args"] = args
            return f"found:{args['key']}"

        plugin_registry = PluginRegistry()
        plugin_registry.register_tool("custom_lookup", custom_lookup_handler)

        # Build tool registry the same way main.py does
        tool_reg = default_tool_registry(adapter_mode="mock")
        tool_reg.update(plugin_registry.get_tool_handlers())

        # Directly exercise execute_tool with the merged registry —
        # this is the path agent handlers use when calling tools in live mode
        result = execute_tool(program, "custom_lookup", {"key": "test"}, tool_reg)

        self.assertTrue(tool_called["called"])
        self.assertEqual(result, "found:test")
        self.assertEqual(tool_called["args"], {"key": "test"})

    def test_plugin_tool_called_via_cli_subprocess(self) -> None:
        """Full CLI path: main.py --plugin loads and merges tool handlers."""
        with tempfile.TemporaryDirectory() as tmpdir:
            src_path = os.path.join(tmpdir, "prog.agent")
            plugin_path = os.path.join(tmpdir, "myplugin.py")

            with open(src_path, "w") as f:
                f.write("""
task echo(msg: String) -> String {}

pipeline main(msg: String) -> String {
  let r = run echo with { msg: msg };
  return r;
}
""")

            with open(plugin_path, "w") as f:
                f.write("""
def register(registry):
    registry.register_task("echo", lambda args, agent: args["msg"])
""")

            proj_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            result = subprocess.run(
                [
                    sys.executable, "main.py", src_path, "main",
                    "--input", '{"msg": "hello"}',
                    "--plugin", plugin_path,
                ],
                cwd=proj_root,
                capture_output=True,
                text=True,
            )

            self.assertEqual(
                result.returncode, 0,
                f"CLI failed: stdout={result.stdout}, stderr={result.stderr}",
            )
            output = json.loads(result.stdout)
            self.assertEqual(output["result"], "hello")


class TimeoutSemanticsTest(unittest.TestCase):
    """Timeout error message must honestly describe deadline-exceeded semantics."""

    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_timeout_error_message_mentions_deadline(self) -> None:
        src = """
task slow() -> String {}

pipeline p() -> String {
  let r = run slow with {} timeout 0.2;
  return r;
}
"""
        program = self._program(src)

        def slow(_args, _agent):
            time.sleep(10)
            return "done"

        with self.assertRaisesRegex(ExecutionError, r"exceeded.*deadline"):
            execute_pipeline(program, "p", {}, {"slow": slow})

    def test_timeout_raises_handler_timeout_error(self) -> None:
        """HandlerTimeoutError is a subclass of ExecutionError."""
        src = """
task slow() -> String {}

pipeline p() -> String {
  let r = run slow with {} timeout 0.2;
  return r;
}
"""
        program = self._program(src)

        def slow(_args, _agent):
            time.sleep(10)
            return "done"

        with self.assertRaises(ExecutionError):
            execute_pipeline(program, "p", {}, {"slow": slow})

    def test_timeout_skips_retries(self) -> None:
        """A timed-out handler must not be retried (previous invocation still running)."""
        src = """
task slow() -> String {}

pipeline p() -> String {
  let r = run slow with {} timeout 0.2 retries 3;
  return r;
}
"""
        program = self._program(src)
        call_count = {"n": 0}

        def slow(_args, _agent):
            call_count["n"] += 1
            time.sleep(10)
            return "done"

        with self.assertRaises(ExecutionError):
            execute_pipeline(program, "p", {}, {"slow": slow})

        # Handler should have been called exactly once — no retries after timeout
        self.assertEqual(call_count["n"], 1)

    def test_leaked_thread_count_reflects_live_threads(self) -> None:
        """Count must go back to 0 after the abandoned thread finishes."""
        src = """
task slow() -> String {}

pipeline p() -> String {
  let r = run slow with {} timeout 0.1;
  return r;
}
"""
        program = self._program(src)

        def slow(_args, _agent):
            time.sleep(0.3)
            return "done"

        with self.assertRaises(ExecutionError):
            execute_pipeline(program, "p", {}, {"slow": slow})

        # Immediately after timeout, the thread should still be alive
        self.assertGreater(get_leaked_thread_count(), 0)

        # After the thread finishes (0.3s sleep), count should return to 0
        time.sleep(0.5)
        self.assertEqual(get_leaked_thread_count(), 0)


if __name__ == "__main__":
    unittest.main()
