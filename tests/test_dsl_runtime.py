from __future__ import annotations

from dataclasses import replace
import unittest

from agentlang import (
    check_program,
    default_task_registry,
    execute_pipeline,
    execute_tool,
    format_pipeline,
    lower_program,
    parse_program,
)
from agentlang.lowering import LoweringError
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

    def test_runtime_surfaces_underlying_task_failure_reason(self) -> None:
        source = """
agent ops {
  model: "gpt-4.1-mini"
  , tools: []
}

task fetch() -> String {}

pipeline greet() -> String {
  let msg = run fetch with {} by ops;
  return msg;
}
"""
        program = self._program(source)

        def fetch(_args, _agent):
            raise ValueError("socket timeout")

        with self.assertRaisesRegex(
            AgentLangRuntimeError,
            r"Task 'fetch' by agent 'ops' failed after 1 attempts\. Last error: ValueError: socket timeout",
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

    def test_agent_tools_must_be_declared(self) -> None:
        source = """
agent researcher {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

pipeline noop() -> String {
  return "ok";
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(
            TypeCheckError,
            r"Agent 'researcher' references unknown tool 'web_search'",
        ):
            check_program(program)

    def test_declared_tool_allows_agent_reference(self) -> None:
        source = """
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent researcher {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

pipeline noop() -> String {
  return "ok";
}
"""
        program = self._program(source)
        self.assertIn("web_search", program.tools)

    def test_execute_tool_validates_args_and_output(self) -> None:
        source = """
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

pipeline noop() -> String {
  return "ok";
}
"""
        program = self._program(source)

        def web_search(_args):
            return [{"title": "A", "url": "https://example.com", "snippet": "alpha"}]

        result = execute_tool(program, "web_search", {"query": "agent"}, {"web_search": web_search})
        self.assertEqual(result[0]["title"], "A")

        with self.assertRaisesRegex(
            AgentLangRuntimeError,
            r"Tool 'web_search' missing args",
        ):
            execute_tool(program, "web_search", {}, {"web_search": web_search})

        def bad_web_search(_args):
            return [{"title": "A", "url": 42, "snippet": "alpha"}]

        with self.assertRaisesRegex(
            AgentLangRuntimeError,
            r"Tool 'web_search' returned invalid value",
        ):
            execute_tool(
                program,
                "web_search",
                {"query": "agent"},
                {"web_search": bad_web_search},
            )

    def test_agent_task_requires_run_binding(self) -> None:
        source = """
task investigate(topic: String) -> Obj{summary: String} by agent {}

pipeline bad(topic: String) -> String {
  let r = run investigate with { topic: topic };
  return r.summary;
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(
            TypeCheckError,
            r"Agent task 'investigate' must be run with an explicit agent binding",
        ):
            check_program(program)

    def test_agent_task_executes_in_mock_mode(self) -> None:
        source = """
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent researcher {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

task investigate(topic: String) -> Obj{summary: String, sources: List[String]} by agent {}

pipeline brief(topic: String) -> String {
  let r = run investigate with { topic: topic } by researcher;
  return r.summary;
}
"""
        program = self._program(source)
        result = execute_pipeline(
            program,
            "brief",
            {"topic": "incident response"},
            default_task_registry(program, adapter_mode="mock"),
        )
        self.assertEqual(result, "[researcher:investigate.summary] incident response")

    def test_while_loop_executes_until_condition_is_false(self) -> None:
        source = """
agent ops {
  model: "gpt-4.1-mini"
  , tools: []
}

task countdown(current: Number) -> Obj{next: Number, done: Bool} {}

pipeline loop_to_zero(start: Number) -> Number {
  let state = run countdown with { current: start } by ops;
  while state.done == false {
    let state = run countdown with { current: state.next } by ops;
  }
  return state.next;
}
"""
        program = self._program(source)
        result = execute_pipeline(
            program,
            "loop_to_zero",
            {"start": 3},
            default_task_registry(program, adapter_mode="mock"),
        )
        self.assertEqual(result, 0)

    def test_while_condition_must_be_bool(self) -> None:
        source = """
pipeline bad() -> String {
  while "nope" {
    return "x";
  }
  return "done";
}
"""
        program = parse_program(source)
        with self.assertRaisesRegex(TypeCheckError, r"While condition must be Bool"):
            check_program(program)

    def test_break_and_continue_work_inside_while(self) -> None:
        source = """
agent ops {
  model: "gpt-4.1-mini"
  , tools: []
}

task countdown(current: Number) -> Obj{next: Number, done: Bool} {}

pipeline stop_early(start: Number) -> Number {
  let state = run countdown with { current: start } by ops;
  while state.done == false {
    if state.next == 2 {
      break;
    }
    let state = run countdown with { current: state.next } by ops;
  }
  return state.next;
}

pipeline skip_once(start: Number) -> Number {
  let state = run countdown with { current: start } by ops;
  while state.done == false {
    if state.next == 2 {
      let state = run countdown with { current: state.next } by ops;
      continue;
    }
    let state = run countdown with { current: state.next } by ops;
  }
  return state.next;
}
"""
        program = self._program(source)
        registry = default_task_registry(program, adapter_mode="mock")
        self.assertEqual(
            execute_pipeline(program, "stop_early", {"start": 4}, registry),
            2,
        )
        self.assertEqual(
            execute_pipeline(program, "skip_once", {"start": 4}, registry),
            0,
        )

    def test_break_and_continue_are_rejected_outside_loops(self) -> None:
        break_source = """
pipeline bad() -> String {
  break;
  return "x";
}
"""
        continue_source = """
pipeline bad() -> String {
  continue;
  return "x";
}
"""
        with self.assertRaisesRegex(TypeCheckError, r"'break' is only valid inside while loops"):
            check_program(parse_program(break_source))
        with self.assertRaisesRegex(TypeCheckError, r"'continue' is only valid inside while loops"):
            check_program(parse_program(continue_source))

    def test_multiagent_review_loop_terminates_and_publishes(self) -> None:
        source = """
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}
tool fetch_url(url: String) -> Obj{content: String} {}

agent planner {
  model: "gpt-4.1"
  , tools: [web_search, fetch_url]
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

agent editor {
  model: "gpt-4.1-mini"
  , tools: []
}

agent publisher {
  model: "gpt-4.1-mini"
  , tools: []
}

task plan_blog(topic: String) -> Obj{outline: String, sources: List[String]} by agent {}
task review_outline(topic: String, outline: String, sources: List[String]) -> Obj{approved: Bool, feedback: String} by agent {}
task revise_outline(topic: String, outline: String, sources: List[String], feedback: String) -> Obj{outline: String, sources: List[String]} by agent {}
task write_blog(topic: String, outline: String) -> Obj{article: String} by agent {}
task edit_blog(topic: String, article: String) -> Obj{title: String, article: String} by agent {}
task publish_blog(topic: String, title: String, article: String) -> Obj{post: String} by agent {}

workflow publish_topic_blog(topic: String) -> String {
  stage plan = planner does plan_blog(topic);
  review outline = reviewer checks plan revise with planner using revise_outline max_rounds 3;
  stage draft = planner does write_blog(topic, outline.outline);
  stage edited = editor does edit_blog(topic, draft.article);
  stage published = publisher does publish_blog(topic, edited.title, edited.article);
  return published.post;
}
"""
        program = self._program(source)
        result = execute_pipeline(
            program,
            "publish_topic_blog",
            {"topic": "incident response"},
            default_task_registry(program, adapter_mode="mock"),
        )
        self.assertEqual(result, "[publisher:publish_blog.post] incident response")

    def test_workflow_lowers_review_loop_to_pipeline_ir(self) -> None:
        source = """
agent planner {
  model: "gpt-4.1"
  , tools: []
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: []
}

task plan_blog(topic: String) -> Obj{outline: String, sources: List[String]} by agent {}
task review_outline(topic: String, outline: String, sources: List[String]) -> Obj{approved: Bool, feedback: String} by agent {}
task revise_outline(topic: String, outline: String, sources: List[String], feedback: String) -> Obj{outline: String, sources: List[String]} by agent {}

workflow publish_topic_blog(topic: String) -> Obj{outline: String, sources: List[String]} {
  stage plan = planner does plan_blog(topic);
  review outline = reviewer checks plan revise with planner using revise_outline max_rounds 2;
  return outline;
}
"""
        raw_program = parse_program(source, lower=False)
        lowered = lower_program(raw_program)
        rendered = format_pipeline(lowered.pipelines["publish_topic_blog"])
        self.assertIn("let __outline_review = run review_outline", rendered)
        self.assertIn("while __outline_review.approved == false", rendered)
        self.assertIn("let __outline_remaining = run countdown", rendered)

    def test_workflow_rejects_consumed_artifact_reference(self) -> None:
        source = """
agent planner {
  model: "gpt-4.1"
  , tools: []
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: []
}

task plan_blog(topic: String) -> Obj{outline: String, sources: List[String]} by agent {}
task review_outline(topic: String, outline: String, sources: List[String]) -> Obj{approved: Bool, feedback: String} by agent {}
task revise_outline(topic: String, outline: String, sources: List[String], feedback: String) -> Obj{outline: String, sources: List[String]} by agent {}
task write_blog(topic: String, outline: String) -> Obj{article: String} by agent {}

workflow bad(topic: String) -> String {
  stage plan = planner does plan_blog(topic);
  review outline = reviewer checks plan revise with planner using revise_outline max_rounds 1;
  stage draft = planner does write_blog(topic, plan.outline);
  return draft.article;
}
"""
        with self.assertRaisesRegex(LoweringError, r"consumed artifact 'plan'"):
            parse_program(source)


if __name__ == "__main__":
    unittest.main()
