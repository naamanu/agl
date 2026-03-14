from __future__ import annotations

import os
import tempfile
import unittest

from agentlang import PluginRegistry, check_program, execute_pipeline, load_plugin, parse_program


class PluginTests(unittest.TestCase):
    def test_register_task_handler(self) -> None:
        registry = PluginRegistry()

        def my_handler(args, agent):
            return "from_plugin"

        registry.register_task("my_task", my_handler)
        handlers = registry.get_task_handlers()
        self.assertIn("my_task", handlers)
        self.assertEqual(handlers["my_task"]({"x": 1}, None), "from_plugin")

    def test_register_tool_handler(self) -> None:
        registry = PluginRegistry()

        def my_tool(args):
            return {"result": True}

        registry.register_tool("my_tool", my_tool)
        handlers = registry.get_tool_handlers()
        self.assertIn("my_tool", handlers)

    def test_plugin_handler_overrides_builtin(self) -> None:
        source = """
task greet(name: String) -> String {}

pipeline main(name: String) -> String {
  let msg = run greet with { name: name };
  return msg;
}
"""
        program = parse_program(source)
        check_program(program)

        def builtin_greet(args, _agent):
            return f"builtin: {args['name']}"

        def plugin_greet(args, _agent):
            return f"plugin: {args['name']}"

        # Simulate plugin override
        registry = {"greet": builtin_greet}
        registry.update({"greet": plugin_greet})

        result = execute_pipeline(program, "main", {"name": "Ada"}, registry)
        self.assertEqual(result, "plugin: Ada")

    def test_load_plugin_from_file(self) -> None:
        plugin_code = """
def register(registry):
    def custom_task(args, agent):
        return "custom:" + args.get("x", "")
    registry.register_task("custom_task", custom_task)
"""
        with tempfile.NamedTemporaryFile(
            suffix=".py", mode="w", delete=False, encoding="utf-8"
        ) as f:
            f.write(plugin_code)
            plugin_path = f.name

        try:
            registry = PluginRegistry()
            load_plugin(plugin_path, registry)
            handlers = registry.get_task_handlers()
            self.assertIn("custom_task", handlers)
            self.assertEqual(handlers["custom_task"]({"x": "hello"}, None), "custom:hello")
        finally:
            os.unlink(plugin_path)

    def test_load_plugin_without_register_raises(self) -> None:
        plugin_code = "# no register function\n"
        with tempfile.NamedTemporaryFile(
            suffix=".py", mode="w", delete=False, encoding="utf-8"
        ) as f:
            f.write(plugin_code)
            plugin_path = f.name

        try:
            registry = PluginRegistry()
            with self.assertRaises(AttributeError):
                load_plugin(plugin_path, registry)
        finally:
            os.unlink(plugin_path)


if __name__ == "__main__":
    unittest.main()
