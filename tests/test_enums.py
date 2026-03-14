from __future__ import annotations

import unittest

from agentlang import check_program, execute_pipeline, parse_program
from agentlang.checker import TypeCheckError
from agentlang.parser import ParseError
from agentlang.runtime import ExecutionError


class EnumTests(unittest.TestCase):
    def _program(self, source: str):
        program = parse_program(source)
        check_program(program)
        return program

    def test_enum_declaration_and_usage_in_task_param(self) -> None:
        source = """
enum FilingStatus { single, married_joint, head_of_household };

task classify(status: FilingStatus) -> String {}

pipeline main(status: String) -> String {
  let r = run classify with { status: status };
  return r;
}
"""
        program = self._program(source)
        self.assertIn("FilingStatus", program.enum_types)
        self.assertEqual(program.enum_types["FilingStatus"].variants, ("single", "married_joint", "head_of_household"))

    def test_enum_runtime_validates_valid_variant(self) -> None:
        source = """
enum FilingStatus { single, married_joint, head_of_household };

task classify(status: FilingStatus) -> String {}

pipeline main(status: String) -> String {
  let r = run classify with { status: status };
  return r;
}
"""
        program = self._program(source)

        def classify(args, _agent):
            return f"classified:{args['status']}"

        result = execute_pipeline(program, "main", {"status": "single"}, {"classify": classify})
        self.assertEqual(result, "classified:single")

    def test_enum_runtime_rejects_invalid_variant(self) -> None:
        source = """
enum FilingStatus { single, married_joint, head_of_household };

task classify(status: FilingStatus) -> String {}

pipeline main(status: String) -> String {
  let r = run classify with { status: status };
  return r;
}
"""
        program = self._program(source)

        def classify(args, _agent):
            return "ok"

        with self.assertRaisesRegex(ExecutionError, r"not a valid variant"):
            execute_pipeline(program, "main", {"status": "invalid_status"}, {"classify": classify})

    def test_type_alias_resolves_to_underlying_type(self) -> None:
        source = """
type TaxProfile = Obj{bracket: String, entity_type: String};

task classify() -> TaxProfile {}

pipeline main() -> String {
  let r = run classify with {};
  return r.bracket;
}
"""
        program = self._program(source)

        def classify(_args, _agent):
            return {"bracket": "22%", "entity_type": "individual"}

        result = execute_pipeline(program, "main", {}, {"classify": classify})
        self.assertEqual(result, "22%")

    def test_type_alias_reused_in_multiple_tasks(self) -> None:
        source = """
type Profile = Obj{name: String, age: Number};

task fetch() -> Profile {}
task update(p: Profile) -> String {}

pipeline main() -> String {
  let p = run fetch with {};
  let msg = run update with { p: p };
  return msg;
}
"""
        program = self._program(source)

        def fetch(_args, _agent):
            return {"name": "Ada", "age": 36}

        def update(args, _agent):
            return f"updated:{args['p']['name']}"

        result = execute_pipeline(program, "main", {}, {"fetch": fetch, "update": update})
        self.assertEqual(result, "updated:Ada")

    def test_duplicate_enum_raises(self) -> None:
        source = """
enum Status { active, inactive };
enum Status { pending };

pipeline main() -> String {
  return "ok";
}
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate enum"):
            parse_program(source)

    def test_duplicate_type_alias_raises(self) -> None:
        source = """
type Foo = String;
type Foo = Number;

pipeline main() -> String {
  return "ok";
}
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate type alias"):
            parse_program(source)

    def test_duplicate_enum_variant_raises(self) -> None:
        source = """
enum Status { active, active };

pipeline main() -> String {
  return "ok";
}
"""
        with self.assertRaisesRegex(ParseError, r"Duplicate enum variant"):
            parse_program(source)

    def test_enum_string_assignable_to_string_param(self) -> None:
        """Enum values should be assignable to String-typed parameters."""
        source = """
enum Color { red, green, blue };

task greet(name: String) -> String {}

pipeline main(c: String) -> String {
  let r = run greet with { name: c };
  return r;
}
"""
        program = self._program(source)

        def greet(args, _agent):
            return f"hi {args['name']}"

        result = execute_pipeline(program, "main", {"c": "red"}, {"greet": greet})
        self.assertEqual(result, "hi red")


if __name__ == "__main__":
    unittest.main()
