from __future__ import annotations

import unittest

from agentlang import check_program, execute_pipeline, parse_program
from agentlang.checker import TypeCheckError
from agentlang.runtime import ExecutionError


class TryCatchTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_try_catch_recovers_from_failing_task(self) -> None:
        source = """
task risky_fetch(url: String) -> String {}
task fallback_fetch(query: String) -> String {}

pipeline main(url: String) -> String {
  try {
    let data = run risky_fetch with { url: url };
    return data;
  } catch err {
    let data = run fallback_fetch with { query: err };
    return data;
  }
}
"""
        program = self._program(source)

        def risky_fetch(_args, _agent):
            raise ValueError("connection refused")

        def fallback_fetch(args, _agent):
            return f"fallback for: {args['query']}"

        result = execute_pipeline(
            program, "main", {"url": "http://example.com"},
            {"risky_fetch": risky_fetch, "fallback_fetch": fallback_fetch},
        )
        self.assertIn("fallback for:", result)

    def test_try_catch_passes_through_on_success(self) -> None:
        source = """
task fetch(url: String) -> String {}

pipeline main(url: String) -> String {
  try {
    let data = run fetch with { url: url };
    return data;
  } catch err {
    return "failed: " + err;
  }
}
"""
        program = self._program(source)

        def fetch(_args, _agent):
            return "success"

        result = execute_pipeline(
            program, "main", {"url": "http://example.com"}, {"fetch": fetch},
        )
        self.assertEqual(result, "success")

    def test_error_var_contains_error_message(self) -> None:
        source = """
task boom() -> String {}

pipeline main() -> String {
  try {
    let x = run boom with {};
    return x;
  } catch err {
    return "caught: " + err;
  }
}
"""
        program = self._program(source)

        def boom(_args, _agent):
            raise RuntimeError("kaboom")

        result = execute_pipeline(program, "main", {}, {"boom": boom})
        self.assertIn("kaboom", result)

    def test_try_catch_type_checks_both_branches(self) -> None:
        source = """
task fetch() -> String {}

pipeline bad() -> Number {
  try {
    let x = run fetch with {};
    return x;
  } catch err {
    return 42;
  }
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(TypeCheckError, r"returns"):
            check_program(program)

    def test_nested_try_catch(self) -> None:
        source = """
task a() -> String {}
task b() -> String {}
task c() -> String {}

pipeline main() -> String {
  try {
    try {
      let x = run a with {};
      return x;
    } catch err1 {
      let y = run b with {};
      return y;
    }
  } catch err2 {
    let z = run c with {};
    return z;
  }
}
"""
        program = self._program(source)

        def a(_args, _agent):
            raise ValueError("a fails")

        def b(_args, _agent):
            raise ValueError("b fails")

        def c(_args, _agent):
            return "c wins"

        result = execute_pipeline(program, "main", {}, {"a": a, "b": b, "c": c})
        self.assertEqual(result, "c wins")


if __name__ == "__main__":
    unittest.main()
