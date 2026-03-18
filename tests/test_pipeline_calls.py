from __future__ import annotations

import unittest

from agentlang import check_program, execute_pipeline, parse_program
from agentlang.checker import TypeCheckError
from agentlang.runtime import ExecutionError


class PipelineCallsTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_pipeline_calls_pipeline(self) -> None:
        source = """
task greet(name: String) -> String {}

pipeline sub(name: String) -> String {
  let msg = run greet with { name: name };
  return msg;
}

pipeline main(name: String) -> String {
  let result = run sub with { name: name };
  return "outer: " + result;
}
"""
        program = self._program(source)

        def greet(args, _agent):
            return f"hi {args['name']}"

        result = execute_pipeline(program, "main", {"name": "Ada"}, {"greet": greet})
        self.assertEqual(result, "outer: hi Ada")

    def test_pipeline_calls_pipeline_with_complex_types(self) -> None:
        source = """
task fetch(id: Number) -> Obj{name: String, score: Number} {}

pipeline inner(id: Number) -> Obj{name: String, score: Number} {
  let r = run fetch with { id: id };
  return r;
}

pipeline outer(id: Number) -> String {
  let profile = run inner with { id: id };
  return profile.name;
}
"""
        program = self._program(source)

        def fetch(args, _agent):
            return {"name": "Alice", "score": 95}

        result = execute_pipeline(program, "outer", {"id": 1}, {"fetch": fetch})
        self.assertEqual(result, "Alice")

    def test_pipeline_call_type_checks_args(self) -> None:
        source = """
pipeline sub(x: Number) -> String {
  return "ok";
}

pipeline main() -> String {
  let r = run sub with { x: "not_a_number" };
  return r;
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(TypeCheckError, r"has type"):
            check_program(program)

    def test_pipeline_call_type_checks_return(self) -> None:
        """Pipeline return type should be used as the run stmt result type."""
        source = """
task fetch() -> String {}

pipeline sub() -> String {
  let r = run fetch with {};
  return r;
}

pipeline main() -> Number {
  let r = run sub with {};
  return r;
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(TypeCheckError, r"returns"):
            check_program(program)

    def test_cannot_use_agent_with_pipeline_call(self) -> None:
        source = """
agent ops {
  tools: []
}

pipeline sub() -> String {
  return "ok";
}

pipeline main() -> String {
  let r = run sub with {} by ops;
  return r;
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(TypeCheckError, r"Cannot use 'by agent'"):
            check_program(program)

    def test_deeply_nested_pipeline_calls(self) -> None:
        source = """
task echo(msg: String) -> String {}

pipeline level2(msg: String) -> String {
  let r = run echo with { msg: msg };
  return r;
}

pipeline level1(msg: String) -> String {
  let r = run level2 with { msg: msg };
  return "l1:" + r;
}

pipeline main(msg: String) -> String {
  let r = run level1 with { msg: msg };
  return "main:" + r;
}
"""
        program = self._program(source)

        def echo(args, _agent):
            return args["msg"]

        result = execute_pipeline(program, "main", {"msg": "hello"}, {"echo": echo})
        self.assertEqual(result, "main:l1:hello")


if __name__ == "__main__":
    unittest.main()
