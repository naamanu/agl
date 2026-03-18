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


    def test_nested_if_rebind_drops_variable_from_merge(self) -> None:
        """Issue 1: Nested conditionals that rebind a variable to an incompatible
        type must drop it from the post-merge environment."""
        src = """
task number_task(x: String) -> Number {}
task bool_task() -> Bool {}

pipeline p(x: String) -> String {
  let c1 = run bool_task with {};
  let c2 = run bool_task with {};
  if c1 {
    if c2 {
      let x = run number_task with { x: x };
    }
  }
  return x;
}
"""
        with self.assertRaises(TypeCheckError):
            self._checked(src)

    def test_string_variable_not_assignable_to_enum_param(self) -> None:
        """Issue 2: A String-typed variable should not be accepted where an
        enum parameter is expected."""
        src = """
enum Color { red, green, blue };

task paint(c: Color) -> String {}
task fetch_name() -> String {}

pipeline p() -> String {
  let name = run fetch_name with {};
  let r = run paint with { c: name };
  return r;
}
"""
        with self.assertRaises(TypeCheckError):
            self._checked(src)

    def test_enum_literal_still_works_for_enum_param(self) -> None:
        """Issue 2 follow-up: String literals matching an enum variant should
        still be inferred as the enum type and accepted."""
        src = """
enum Color { red, green, blue };

task paint(c: Color) -> String {}

pipeline p() -> String {
  let r = run paint with { c: "red" };
  return r;
}
"""
        program = self._checked(src)
        self.assertIsNotNone(program)

    def test_overlapping_enum_variants_rejected(self) -> None:
        """Issue 3: Two enums sharing a variant name must be rejected."""
        src = """
enum Color { active, inactive };
enum Status { active, pending };

pipeline p() -> String {
  return "ok";
}
"""
        with self.assertRaises(TypeCheckError):
            self._checked(src)


if __name__ == "__main__":
    unittest.main()
