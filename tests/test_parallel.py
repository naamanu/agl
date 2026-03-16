from __future__ import annotations

import time
import unittest
from unittest.mock import patch

from agentlang import (
    check_program,
    default_task_registry,
    execute_pipeline,
    parse_program,
)
from agentlang.runtime import ExecutionError


class ParallelTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_parallel_branches_execute_concurrently(self) -> None:
        src = """
task slow() -> String {}

pipeline p() -> String {
  parallel {
    let a = run slow with {};
    let b = run slow with {};
  } join;
  return a + b;
}
"""
        program = self._program(src)

        def slow(_args, _agent):
            time.sleep(0.1)
            return "done"

        start = time.monotonic()
        result = execute_pipeline(program, "p", {}, {"slow": slow})
        elapsed = time.monotonic() - start
        self.assertEqual(result, "donedone")
        # If sequential, it would take >= 0.2s. Concurrent should be ~0.1s.
        self.assertLess(elapsed, 0.18)

    def test_parallel_branches_have_isolated_environments(self) -> None:
        src = """
task read_var(name: String) -> String {}

pipeline p(input: String) -> String {
  parallel {
    let a = run read_var with { name: input };
    let b = run read_var with { name: input };
  } join;
  return a + b;
}
"""
        program = self._program(src)
        call_count = {"a": 0, "b": 0}

        def read_var(args, _agent):
            return f"[{args['name']}]"

        result = execute_pipeline(program, "p", {"input": "x"}, {"read_var": read_var})
        self.assertEqual(result, "[x][x]")

    def test_timeout_causes_execution_error(self) -> None:
        src = """
task slow() -> String {}

pipeline p() -> String {
  let r = run slow with {} timeout 0.1;
  return r;
}
"""
        program = self._program(src)

        def slow(_args, _agent):
            time.sleep(2)
            return "done"

        with self.assertRaisesRegex(ExecutionError, r"exceeded.*deadline"):
            execute_pipeline(program, "p", {}, {"slow": slow})

    def test_retry_backoff_uses_sleep(self) -> None:
        src = """
task flaky() -> String {}

pipeline p() -> String {
  let r = run flaky with {} retries 2;
  return r;
}
"""
        program = self._program(src)
        attempts = []

        def flaky(_args, _agent):
            attempts.append(1)
            if len(attempts) < 3:
                raise ValueError("fail")
            return "ok"

        with patch("agentlang.runtime.time.sleep") as mock_sleep:
            result = execute_pipeline(program, "p", {}, {"flaky": flaky})
        self.assertEqual(result, "ok")
        self.assertEqual(len(attempts), 3)
        # sleep should have been called for attempt 1 and 2 (not attempt 0)
        self.assertEqual(mock_sleep.call_count, 2)

    def test_width_subtyping_at_runtime(self) -> None:
        src = """
task wide() -> Obj{a: String, b: String} {}

pipeline p() -> String {
  let r = run wide with {};
  return r.a;
}
"""
        program = self._program(src)

        def wide(_args, _agent):
            return {"a": "hello", "b": "world", "extra": "ignored"}

        result = execute_pipeline(program, "p", {}, {"wide": wide})
        self.assertEqual(result, "hello")


if __name__ == "__main__":
    unittest.main()
