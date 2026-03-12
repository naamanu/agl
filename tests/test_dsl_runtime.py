from __future__ import annotations

from dataclasses import replace
import unittest

from agentlang import check_program, execute_pipeline, parse_program
from agentlang.ast import PrimitiveType
from agentlang.checker import TypeCheckError
from agentlang.runtime import RuntimeError as AgentLangRuntimeError


class AgentLangDslRuntimeTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_runtime_rejects_invalid_task_result_shape(self) -> None:
        source = """
task fetch() -> Obj{name: String} {}

pipeline greet() -> String {
  let person = run fetch with {};
  return person.name;
}
"""
        program = self._program(source)

        def fetch(_args, _agent):
            return {"wrong": "Ada"}

        with self.assertRaisesRegex(
            AgentLangRuntimeError,
            r"Task 'fetch' returned invalid value",
        ):
            execute_pipeline(program, "greet", {}, {"fetch": fetch})

    def test_runtime_rejects_invalid_pipeline_return_value(self) -> None:
        source = """
task fetch() -> String {}

pipeline greet() -> String {
  let msg = run fetch with {};
  return msg;
}
"""
        program = self._program(source)
        program.pipelines["greet"] = replace(
            program.pipelines["greet"],
            return_type=PrimitiveType("Number"),
        )

        def fetch(_args, _agent):
            return "hello"

        with self.assertRaisesRegex(
            AgentLangRuntimeError,
            r"Pipeline 'greet' returned invalid value",
        ):
            execute_pipeline(program, "greet", {}, {"fetch": fetch})

    def test_option_null_and_if_let_execute_both_paths(self) -> None:
        source = """
task maybe_person(flag: Bool) -> Option[Obj{name: String}] {}

pipeline greet(flag: Bool) -> String {
  let person = run maybe_person with { flag: flag };
  if let value = person {
    return "hi " + value.name;
  } else {
    return "nobody";
  }
}
"""
        program = self._program(source)

        def maybe_person(args, _agent):
            if args["flag"]:
                return {"name": "Ada"}
            return None

        registry = {"maybe_person": maybe_person}
        self.assertEqual(
            execute_pipeline(program, "greet", {"flag": True}, registry),
            "hi Ada",
        )
        self.assertEqual(
            execute_pipeline(program, "greet", {"flag": False}, registry),
            "nobody",
        )

    def test_if_let_requires_option_type(self) -> None:
        source = """
pipeline bad() -> String {
  if let value = "plain string" {
    return value;
  } else {
    return "fallback";
  }
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(TypeCheckError, r"If-let expression must have Option type"):
            check_program(program)

    def test_null_compares_with_option(self) -> None:
        source = """
task maybe_text() -> Option[String] {}

pipeline is_missing() -> Bool {
  let value = run maybe_text with {};
  return value == null;
}
"""
        program = self._program(source)

        def maybe_text(_args, _agent):
            return None

        result = execute_pipeline(program, "is_missing", {}, {"maybe_text": maybe_text})
        self.assertTrue(result)

    def test_object_field_can_use_option_type(self) -> None:
        source = """
task profile() -> Obj{nickname: Option[String]} {}

pipeline nickname_or_default() -> String {
  let p = run profile with {};
  if let nick = p.nickname {
    return nick;
  } else {
    return "anon";
  }
}
"""
        program = self._program(source)

        def profile(_args, _agent):
            return {"nickname": None}

        result = execute_pipeline(program, "nickname_or_default", {}, {"profile": profile})
        self.assertEqual(result, "anon")


if __name__ == "__main__":
    unittest.main()
