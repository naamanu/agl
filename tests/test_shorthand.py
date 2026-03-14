from __future__ import annotations

import unittest

from agentlang import check_program, execute_pipeline, parse_program
from agentlang.parser import ParseError


class ShorthandSyntaxTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_shorthand_positional_args(self) -> None:
        source = """
task greet(name: String, greeting: String) -> String {}

agent ops {
  tools: []
}

pipeline main(name: String) -> String {
  let r = greet(name, "hello") by ops;
  return r;
}
"""
        program = self._program(source)

        def greet(args, _agent):
            return f"{args['greeting']} {args['name']}"

        result = execute_pipeline(program, "main", {"name": "Ada"}, {"greet": greet})
        self.assertEqual(result, "hello Ada")

    def test_shorthand_wrong_arg_count_raises(self) -> None:
        source = """
task greet(name: String, greeting: String) -> String {}

pipeline main() -> String {
  let r = greet("Ada");
  return r;
}
"""
        with self.assertRaisesRegex(ParseError, r"expected 2 args, got 1"):
            parse_program(source)

    def test_shorthand_with_retries_and_timeout(self) -> None:
        source = """
task fetch(url: String) -> String {}

pipeline main(url: String) -> String {
  let r = fetch(url) retries 2 timeout 30;
  return r;
}
"""
        program = self._program(source)

        def fetch(args, _agent):
            return f"fetched:{args['url']}"

        result = execute_pipeline(program, "main", {"url": "http://example.com"}, {"fetch": fetch})
        self.assertEqual(result, "fetched:http://example.com")

    def test_agent_without_model_parses(self) -> None:
        source = """
agent ops {
  tools: []
}

pipeline main() -> String {
  return "ok";
}
"""
        program = self._program(source)
        self.assertIsNone(program.agents["ops"].model)

    def test_agent_with_model_still_works(self) -> None:
        source = """
agent ops {
  model: "gpt-4.1-mini"
  , tools: []
}

pipeline main() -> String {
  return "ok";
}
"""
        program = self._program(source)
        self.assertEqual(program.agents["ops"].model, "gpt-4.1-mini")


class MaxConcurrencyTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_max_concurrency_parses(self) -> None:
        source = """
task fetch(id: Number) -> String {}

pipeline main() -> String {
  parallel max_concurrency 2 {
    let a = run fetch with { id: 1 };
    let b = run fetch with { id: 2 };
    let c = run fetch with { id: 3 };
  } join;
  return a + b + c;
}
"""
        program = self._program(source)
        # Verify the max_concurrency is set
        from agentlang.ast import ParallelStmt
        parallel = program.pipelines["main"].statements[0]
        self.assertIsInstance(parallel, ParallelStmt)
        self.assertEqual(parallel.max_concurrency, 2)

    def test_max_concurrency_executes(self) -> None:
        source = """
task fetch(id: Number) -> String {}

pipeline main() -> String {
  parallel max_concurrency 2 {
    let a = run fetch with { id: 1 };
    let b = run fetch with { id: 2 };
    let c = run fetch with { id: 3 };
  } join;
  return a + b + c;
}
"""
        program = self._program(source)

        def fetch(args, _agent):
            return f"r{args['id']}"

        result = execute_pipeline(program, "main", {}, {"fetch": fetch})
        self.assertEqual(result, "r1r2r3")

    def test_max_concurrency_must_be_positive(self) -> None:
        source = """
task fetch(id: Number) -> String {}

pipeline main() -> String {
  parallel max_concurrency 0 {
    let a = run fetch with { id: 1 };
  } join;
  return a;
}
"""
        with self.assertRaisesRegex(ParseError, r"max_concurrency must be at least 1"):
            parse_program(source)


if __name__ == "__main__":
    unittest.main()
