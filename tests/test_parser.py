from __future__ import annotations

import unittest

from agentlang.parser import ParseError, parse_program
from agentlang.ast import (
    LiteralExpr,
    ObjType,
    PipelineDef,
    PrimitiveType,
    RefExpr,
    RunStmt,
    Span,
)


class ParserTests(unittest.TestCase):
    def test_minimal_pipeline(self) -> None:
        src = 'pipeline hello() -> String { return "hi"; }'
        program = parse_program(src, lower=False)
        self.assertIn("hello", program.pipelines)
        p = program.pipelines["hello"]
        self.assertEqual(p.name, "hello")
        self.assertEqual(p.return_type, PrimitiveType("String"))

    def test_duplicate_agent_raises(self) -> None:
        src = """
agent a { model: "m", tools: [] }
agent a { model: "m", tools: [] }
pipeline p() -> String { return "x"; }
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate agent"):
            parse_program(src, lower=False)

    def test_duplicate_task_raises(self) -> None:
        src = """
task t() -> String {}
task t() -> String {}
pipeline p() -> String { return "x"; }
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate task"):
            parse_program(src, lower=False)

    def test_duplicate_pipeline_raises(self) -> None:
        src = """
pipeline p() -> String { return "x"; }
pipeline p() -> String { return "y"; }
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate pipeline"):
            parse_program(src, lower=False)

    def test_duplicate_param_raises(self) -> None:
        src = 'pipeline p(a: String, a: String) -> String { return "x"; }'
        with self.assertRaisesRegex(ParseError, r"Duplicate parameter"):
            parse_program(src, lower=False)

    def test_duplicate_tool_in_agent_raises(self) -> None:
        src = """
tool t(q: String) -> String {}
agent a { model: "m", tools: [t, t] }
pipeline p() -> String { return "x"; }
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate tool"):
            parse_program(src, lower=False)

    def test_duplicate_obj_field_raises(self) -> None:
        src = 'pipeline p() -> String { return { a: "x", a: "y" }; }'
        with self.assertRaisesRegex(ParseError, r"Duplicate object literal field"):
            parse_program(src, lower=False)

    def test_duplicate_arg_in_run_raises(self) -> None:
        src = """
task t(x: String) -> String {}
pipeline p() -> String {
  let r = run t with { x: "a", x: "b" };
  return r;
}
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate argument"):
            parse_program(src, lower=False)

    def test_missing_semicolon_raises(self) -> None:
        src = 'pipeline p() -> String { return "x" }'
        with self.assertRaisesRegex(ParseError, r"Expected SEMI"):
            parse_program(src, lower=False)

    def test_unexpected_token_raises(self) -> None:
        src = "foobar"
        with self.assertRaisesRegex(ParseError, r"Unexpected token"):
            parse_program(src, lower=False)

    def test_span_captured_on_expressions(self) -> None:
        src = 'pipeline p() -> String { return "hello"; }'
        program = parse_program(src, lower=False)
        ret_stmt = program.pipelines["p"].statements[0]
        self.assertIsNotNone(ret_stmt.span)
        self.assertIsInstance(ret_stmt.span, Span)

    def test_timeout_parsing(self) -> None:
        src = """
task t() -> String {}
pipeline p() -> String {
  let r = run t with {} timeout 5.0;
  return r;
}
"""
        program = parse_program(src, lower=False)
        stmt = program.pipelines["p"].statements[0]
        self.assertIsInstance(stmt, RunStmt)
        self.assertEqual(stmt.timeout, 5.0)

    def test_timeout_must_be_positive(self) -> None:
        src = """
task t() -> String {}
pipeline p() -> String {
  let r = run t with {} timeout 0;
  return r;
}
"""
        with self.assertRaisesRegex(ParseError, r"Timeout must be positive"):
            parse_program(src, lower=False)

    def test_duplicate_timeout_raises(self) -> None:
        src = """
task t() -> String {}
pipeline p() -> String {
  let r = run t with {} timeout 1 timeout 2;
  return r;
}
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate 'timeout'"):
            parse_program(src, lower=False)

    def test_obj_type_fields_frozen(self) -> None:
        src = """
task t() -> Obj{a: String, b: Number} {}
pipeline p() -> String {
  let r = run t with {};
  return r.a;
}
"""
        program = parse_program(src, lower=False)
        task = program.tasks["t"]
        self.assertIsInstance(task.return_type, ObjType)
        with self.assertRaises(TypeError):
            task.return_type.fields["c"] = PrimitiveType("Bool")

    def test_program_dicts_are_frozen(self) -> None:
        src = 'pipeline p() -> String { return "x"; }'
        program = parse_program(src, lower=False)
        with self.assertRaises(TypeError):
            program.pipelines["new"] = program.pipelines["p"]

    def test_lists_are_tuples(self) -> None:
        src = """
task t(x: String) -> String {}
pipeline p() -> String {
  let r = run t with { x: "a" };
  return r;
}
"""
        program = parse_program(src, lower=False)
        self.assertIsInstance(program.pipelines["p"].statements, tuple)
        self.assertIsInstance(program.tasks["t"].params, tuple)

    def test_round_trip_parse(self) -> None:
        src = """
task fetch(topic: String) -> Obj{notes: String} {}

pipeline blog(topic: String) -> String {
  let r = run fetch with { topic: topic };
  return r.notes;
}
"""
        program1 = parse_program(src, lower=False)
        from agentlang.lowering import format_pipeline
        rendered = format_pipeline(program1.pipelines["blog"])
        # Just check it parses without error
        program2 = parse_program(rendered + "\ntask fetch(topic: String) -> Obj{notes: String} {}", lower=False)
        self.assertIn("blog", program2.pipelines)


if __name__ == "__main__":
    unittest.main()
