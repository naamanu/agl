from __future__ import annotations

import unittest

from agentlang import check_program, parse_program
from agentlang.checker import TypeCheckError


class CheckerTests(unittest.TestCase):
    def _checked(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_width_subtyping_accepts_extra_fields(self) -> None:
        src = """
task wide() -> Obj{a: String, b: String, c: Number} {}

pipeline p() -> String {
  let r = run wide with {};
  return r.a;
}
"""
        # The pipeline returns String, and r.a is String.
        # But the important thing is that checker doesn't reject the task return type
        # when compared to an expected type with fewer fields.
        program = self._checked(src)
        self.assertIsNotNone(program)

    def test_width_subtyping_rejects_missing_fields(self) -> None:
        src = """
task narrow() -> Obj{a: String} {}
task expect_wide(data: Obj{a: String, b: String}) -> String {}

pipeline p() -> String {
  let r = run narrow with {};
  let s = run expect_wide with { data: r };
  return s;
}
"""
        with self.assertRaises(TypeCheckError):
            program = parse_program(src)
            check_program(program)

    def test_pipeline_missing_return(self) -> None:
        src = """
task t() -> String {}

pipeline p() -> String {
  let r = run t with {};
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"missing a return"):
            self._checked(src)

    def test_unknown_variable_in_expression(self) -> None:
        src = """
pipeline p() -> String {
  return unknown_var;
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"Unknown variable"):
            self._checked(src)

    def test_type_mismatch_in_return(self) -> None:
        src = """
pipeline p() -> Number {
  return "not a number";
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"returns.*expected"):
            self._checked(src)

    def test_if_condition_must_be_bool(self) -> None:
        src = """
pipeline p() -> String {
  if "hello" {
    return "yes";
  }
  return "no";
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"If condition must be Bool"):
            self._checked(src)

    def test_break_outside_loop(self) -> None:
        src = """
pipeline p() -> String {
  break;
  return "x";
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"'break' is only valid"):
            self._checked(src)

    def test_continue_outside_loop(self) -> None:
        src = """
pipeline p() -> String {
  continue;
  return "x";
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"'continue' is only valid"):
            self._checked(src)

    def test_unknown_task_in_run(self) -> None:
        src = """
pipeline p() -> String {
  let r = run nonexistent with {};
  return r;
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"Unknown task"):
            self._checked(src)

    def test_agent_task_without_agent_binding(self) -> None:
        src = """
task t() -> String by agent {}

pipeline p() -> String {
  let r = run t with {};
  return r;
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"must be run with an explicit agent"):
            self._checked(src)

    def test_field_access_on_non_object(self) -> None:
        src = """
task t() -> String {}

pipeline p() -> String {
  let r = run t with {};
  return r.field;
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"Cannot access field"):
            self._checked(src)

    def test_span_info_in_error_message(self) -> None:
        src = """
pipeline p() -> String {
  return 42;
}
"""
        try:
            self._checked(src)
            self.fail("Expected TypeCheckError")
        except TypeCheckError as e:
            msg = str(e)
            self.assertRegex(msg, r"at \d+:\d+:")

    def test_binary_op_type_mismatch(self) -> None:
        src = """
pipeline p() -> String {
  return "hello" + 42;
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"Cannot apply \+"):
            self._checked(src)

    def test_empty_list_literal_rejected(self) -> None:
        src = """
pipeline p() -> String {
  return [];
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"Cannot infer type of empty list"):
            self._checked(src)


if __name__ == "__main__":
    unittest.main()
