from __future__ import annotations

import unittest

from agentlang import check_program, execute_pipeline, parse_program, run_tests
from agentlang.checker import TypeCheckError
from agentlang.runtime import ExecutionError


class AssertTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_assert_passes_on_true_condition(self) -> None:
        source = """
task fetch() -> String {}

pipeline main() -> String {
  let x = run fetch with {};
  assert x == "hello";
  return x;
}
"""
        program = self._program(source)

        def fetch(_args, _agent):
            return "hello"

        result = execute_pipeline(program, "main", {}, {"fetch": fetch})
        self.assertEqual(result, "hello")

    def test_assert_fails_on_false_condition(self) -> None:
        source = """
task fetch() -> String {}

pipeline main() -> String {
  let x = run fetch with {};
  assert x == "expected", "wrong value";
  return x;
}
"""
        program = self._program(source)

        def fetch(_args, _agent):
            return "actual"

        with self.assertRaisesRegex(ExecutionError, r"Assertion failed: wrong value"):
            execute_pipeline(program, "main", {}, {"fetch": fetch})

    def test_assert_without_message(self) -> None:
        source = """
task fetch() -> Number {}

pipeline main() -> Number {
  let x = run fetch with {};
  assert x == 42;
  return x;
}
"""
        program = self._program(source)

        def fetch(_args, _agent):
            return 0

        with self.assertRaisesRegex(ExecutionError, r"Assertion failed"):
            execute_pipeline(program, "main", {}, {"fetch": fetch})

    def test_assert_condition_must_be_bool(self) -> None:
        source = """
pipeline main() -> String {
  assert "not a bool";
  return "ok";
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(TypeCheckError, r"Assert condition must be Bool"):
            check_program(program)

    def test_test_block_passes(self) -> None:
        source = """
task greet(name: String) -> String {}

test "greet returns hello" {
  let r = run greet with { name: "Ada" };
  assert r == "hello Ada", "Expected hello Ada";
}
"""
        program = self._program(source)

        def greet(args, _agent):
            return f"hello {args['name']}"

        results = run_tests(program, {"greet": greet})
        self.assertEqual(len(results), 1)
        self.assertTrue(results[0]["passed"])
        self.assertIsNone(results[0]["error"])

    def test_test_block_fails(self) -> None:
        source = """
task greet(name: String) -> String {}

test "greet returns wrong" {
  let r = run greet with { name: "Ada" };
  assert r == "wrong", "Mismatch";
}
"""
        program = self._program(source)

        def greet(args, _agent):
            return f"hello {args['name']}"

        results = run_tests(program, {"greet": greet})
        self.assertEqual(len(results), 1)
        self.assertFalse(results[0]["passed"])
        self.assertIn("Mismatch", results[0]["error"])

    def test_multiple_test_blocks(self) -> None:
        source = """
task echo(msg: String) -> String {}

test "echo passes through" {
  let r = run echo with { msg: "hello" };
  assert r == "hello";
}

test "echo with world" {
  let r = run echo with { msg: "world" };
  assert r == "world";
}
"""
        program = self._program(source)

        def echo(args, _agent):
            return args["msg"]

        results = run_tests(program, {"echo": echo})
        self.assertEqual(len(results), 2)
        self.assertTrue(all(r["passed"] for r in results))

    def test_test_block_with_task_failure(self) -> None:
        source = """
task boom() -> String {}

test "boom catches error" {
  let r = run boom with {};
  assert r == "ok";
}
"""
        program = self._program(source)

        def boom(_args, _agent):
            raise ValueError("exploded")

        results = run_tests(program, {"boom": boom})
        self.assertEqual(len(results), 1)
        self.assertFalse(results[0]["passed"])
        self.assertIn("exploded", results[0]["error"])


if __name__ == "__main__":
    unittest.main()
