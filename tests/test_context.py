from __future__ import annotations

import json
import unittest

from agentlang import ExecutionContext, check_program, execute_pipeline, parse_program


class ExecutionContextTests(unittest.TestCase):
    def test_records_task_start_and_end(self) -> None:
        ctx = ExecutionContext()
        ctx.record_task_start("my_task", {"key": "value"})
        ctx.record_task_end("my_task", "result_value", duration=0.5)

        self.assertEqual(len(ctx.events), 2)
        self.assertEqual(ctx.events[0]["type"], "task_start")
        self.assertEqual(ctx.events[0]["task"], "my_task")
        self.assertEqual(ctx.events[1]["type"], "task_end")
        self.assertEqual(ctx.events[1]["duration_s"], 0.5)

    def test_records_task_error(self) -> None:
        ctx = ExecutionContext()
        ctx.record_task_error("my_task", ValueError("boom"))
        self.assertEqual(ctx.events[0]["type"], "task_error")
        self.assertIn("boom", ctx.events[0]["error"])

    def test_records_parallel_events(self) -> None:
        ctx = ExecutionContext()
        ctx.record_parallel_start(3)
        ctx.record_parallel_end(3)
        self.assertEqual(ctx.events[0]["type"], "parallel_start")
        self.assertEqual(ctx.events[0]["branch_count"], 3)
        self.assertEqual(ctx.events[1]["type"], "parallel_end")

    def test_records_retry(self) -> None:
        ctx = ExecutionContext()
        ctx.record_retry("my_task", 2, RuntimeError("timeout"))
        self.assertEqual(ctx.events[0]["type"], "retry")
        self.assertEqual(ctx.events[0]["attempt"], 2)

    def test_to_json_produces_valid_json(self) -> None:
        ctx = ExecutionContext()
        ctx.record_task_start("t", {"x": 1})
        ctx.record_task_end("t", {"y": 2})
        output = ctx.to_json()
        parsed = json.loads(output)
        self.assertIn("trace", parsed)
        self.assertEqual(len(parsed["trace"]), 2)
        # Internal keys should be stripped
        for event in parsed["trace"]:
            for key in event:
                self.assertFalse(key.startswith("_"), f"Internal key {key!r} leaked")

    def test_context_threaded_through_pipeline(self) -> None:
        source = """
task greet(name: String) -> String {}

pipeline say_hi(name: String) -> String {
  let msg = run greet with { name: name };
  return msg;
}
"""
        program = parse_program(source)
        check_program(program)

        def greet(args, _agent):
            return f"hi {args['name']}"

        ctx = ExecutionContext()
        result = execute_pipeline(
            program, "say_hi", {"name": "Ada"}, {"greet": greet}, ctx=ctx
        )
        self.assertEqual(result, "hi Ada")
        # Should have pipeline_call, task_start, task_end events
        types = [e["type"] for e in ctx.events]
        self.assertIn("pipeline_call", types)
        self.assertIn("task_start", types)
        self.assertIn("task_end", types)


if __name__ == "__main__":
    unittest.main()
