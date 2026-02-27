from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any

try:
    import readline  # noqa: F401 - side-effect: enables arrow-key history in input()
except ImportError:
    pass

from .ast import ListType, ObjType, PrimitiveType, TypeExpr
from .checker import TypeCheckError, check_program
from .lexer import LexError
from .parser import ParseError, parse_program
from .runtime import AgentRuntimeError, execute_pipeline
from .stdlib import default_task_registry

BANNER = "AgentLang REPL  —  type :help for commands, Ctrl-D to exit"

_HELP = """\
Definitions (accumulate across inputs):
  agent  <name> {{ model: "...", tools: [...] }}
  task   <name>(<params>) -> <type> {{}}
  pipeline <name>(<params>) -> <type> {{ ... }}

Commands:
  :run  <pipeline> [json]   Run a pipeline (json defaults to {{}})
  :load <path>              Load a .agent file into the session
  :agents                   List defined agents
  :tasks                    List defined tasks
  :pipelines                List defined pipelines
  :adapter mock|live        Switch adapter mode (currently {adapter})
  :reset                    Clear all definitions
  :help                     Show this help
  :quit / :exit             Exit the REPL

Note: task handlers are provided by the stdlib adapter.  Custom task
names outside the built-in set will raise a runtime error on :run.\
"""


# ---------------------------------------------------------------------------
# Session
# ---------------------------------------------------------------------------


@dataclass
class ReplSession:
    """Holds accumulated definitions as (source_fragment, defined_keys) pairs."""

    adapter_mode: str = "mock"
    max_workers: int = 8
    _entries: list[tuple[str, frozenset[str]]] = field(default_factory=list)

    # ------------------------------------------------------------------
    # Fragment management
    # ------------------------------------------------------------------

    def add_fragment(self, text: str) -> tuple[list[str], list[str]]:
        """Parse *text*, merge it into the session, type-check the whole program.

        Returns (new_names, redefined_names) on success.
        Raises LexError, ParseError, or TypeCheckError on failure (session unchanged).
        """
        # Step 1 – parse the fragment alone to discover what names it defines.
        mini = parse_program(text)
        new_keys: frozenset[str] = frozenset(
            {f"agent:{n}" for n in mini.agents}
            | {f"task:{n}" for n in mini.tasks}
            | {f"pipeline:{n}" for n in mini.pipelines}
        )
        if not new_keys:
            raise ParseError("No definitions found in input.")

        # Step 2 – identify overlapping (redefined) names.
        redefined: list[str] = []
        kept: list[tuple[str, frozenset[str]]] = []
        for frag_text, frag_keys in self._entries:
            overlap = frag_keys & new_keys
            if overlap:
                redefined.extend(k.split(":", 1)[1] for k in sorted(overlap))
            else:
                kept.append((frag_text, frag_keys))

        # Step 3 – reconstruct and type-check the merged program.
        candidate_entries = kept + [(text, new_keys)]
        full_source = "\n\n".join(t for t, _ in candidate_entries)
        full_program = parse_program(full_source)  # should not raise (no dupes now)
        check_program(full_program)  # raises TypeCheckError if inconsistent

        # Step 4 – commit.
        # Compute "added" before mutating: names in the new fragment that were
        # not present anywhere in the session before this call.
        all_old_keys: frozenset[str] = frozenset().union(*(k for _, k in self._entries))
        added = sorted(k.split(":", 1)[1] for k in new_keys - all_old_keys)
        self._entries = candidate_entries
        return added, redefined

    def full_source(self) -> str:
        return "\n\n".join(t for t, _ in self._entries)

    def program(self):
        src = self.full_source()
        if not src.strip():
            return None
        return parse_program(src)

    def reset(self) -> None:
        self._entries.clear()


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def run_repl(adapter_mode: str = "mock", max_workers: int = 8) -> None:
    session = ReplSession(adapter_mode=adapter_mode, max_workers=max_workers)
    print(BANNER)
    print()

    while True:
        try:
            raw = _read_block()
        except EOFError:
            print("\nBye!")
            break

        line = raw.strip()
        if not line:
            continue

        if line.startswith(":"):
            if _handle_command(line, session):
                break  # :quit / :exit returned True
        else:
            _handle_definition(line, session)


# ---------------------------------------------------------------------------
# Input reading
# ---------------------------------------------------------------------------


def _read_block() -> str:
    """Read one complete block (brace-balanced) or a single command line."""
    try:
        first = input(">>> ")
    except KeyboardInterrupt:
        print()
        return ""

    stripped = first.strip()
    # Commands and comment-only lines are single-line.
    if stripped.startswith(":") or not stripped or _brace_depth(first) == 0:
        return first

    lines = [first]
    while True:
        try:
            cont = input("... ")
        except KeyboardInterrupt:
            print()
            return ""
        lines.append(cont)
        if _brace_depth("\n".join(lines)) <= 0:
            break
    return "\n".join(lines)


def _brace_depth(text: str) -> int:
    """Count net open braces outside string literals."""
    depth = 0
    in_string = False
    i = 0
    while i < len(text):
        ch = text[i]
        if in_string:
            if ch == "\\" and i + 1 < len(text):
                i += 2  # skip escaped character
                continue
            if ch == '"':
                in_string = False
        else:
            if ch == '"':
                in_string = True
            elif ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
        i += 1
    return depth


# ---------------------------------------------------------------------------
# Definition handling
# ---------------------------------------------------------------------------


def _handle_definition(text: str, session: ReplSession) -> None:
    try:
        added, redefined = session.add_fragment(text)
    except LexError as exc:
        print(f"  Lex error: {exc}")
        return
    except ParseError as exc:
        print(f"  Parse error: {exc}")
        return
    except TypeCheckError as exc:
        print(f"  Type error: {exc}")
        return

    for name in redefined:
        print(f"  Redefined '{name}'")
    for name in added:
        print(f"  Defined '{name}'")


# ---------------------------------------------------------------------------
# Command handling  (:run, :load, :agents, etc.)
# ---------------------------------------------------------------------------


def _handle_command(line: str, session: ReplSession) -> bool:
    """Handle a :-command. Returns True if the REPL should exit."""
    parts = line.split(None, 2)
    cmd = parts[0].lower()

    if cmd in (":quit", ":exit"):
        print("Bye!")
        return True

    if cmd == ":help":
        print(_HELP.format(adapter=session.adapter_mode))
        return False

    if cmd == ":reset":
        session.reset()
        print("  Session cleared.")
        return False

    if cmd == ":adapter":
        if len(parts) < 2:
            print("  Usage: :adapter mock|live")
            return False
        mode = parts[1].lower()
        if mode not in {"mock", "live"}:
            print("  Adapter must be 'mock' or 'live'.")
            return False
        session.adapter_mode = mode
        print(f"  Adapter mode: {mode}")
        return False

    if cmd == ":load":
        if len(parts) < 2:
            print("  Usage: :load <path>")
            return False
        _cmd_load(parts[1], session)
        return False

    if cmd in (":agents", ":tasks", ":pipelines"):
        _cmd_list(session, cmd[1:])
        return False

    if cmd == ":run":
        if len(parts) < 2:
            print("  Usage: :run <pipeline> [json]")
            return False
        raw_json = parts[2] if len(parts) > 2 else "{}"
        _cmd_run(parts[1], raw_json, session)
        return False

    print(f"  Unknown command '{cmd}'. Type :help for commands.")
    return False


def _cmd_load(path: str, session: ReplSession) -> None:
    try:
        text = open(path, encoding="utf-8").read()
    except OSError as exc:
        print(f"  Cannot read '{path}': {exc}")
        return

    try:
        added, redefined = session.add_fragment(text)
    except LexError as exc:
        print(f"  Lex error in '{path}': {exc}")
        return
    except ParseError as exc:
        print(f"  Parse error in '{path}': {exc}")
        return
    except TypeCheckError as exc:
        print(f"  Type error in '{path}': {exc}")
        return

    na = len(added)
    summary = f"{na} definition{'s' if na != 1 else ''} added"
    print(f"  Loaded '{path}'  ({summary})")
    for name in added:
        print(f"    + {name}")
    if redefined:
        print(f"  Redefined: {', '.join(sorted(redefined))}")


def _cmd_list(session: ReplSession, kind: str) -> None:
    program = session.program()
    if program is None:
        print("  (empty session — nothing defined yet)")
        return

    items: dict[str, Any] = getattr(program, kind)
    if not items:
        print(f"  No {kind} defined.")
        return

    if kind == "agents":
        for name, agent in items.items():
            tools = ", ".join(agent.tools) if agent.tools else "none"
            print(f"  {name}  model={agent.model}  tools=[{tools}]")
    elif kind == "tasks":
        for name, task in items.items():
            sig = _fmt_params(task.params)
            print(f"  task {name}({sig}) -> {_fmt_type(task.return_type)}")
    elif kind == "pipelines":
        for name, pipe in items.items():
            sig = _fmt_params(pipe.params)
            print(f"  pipeline {name}({sig}) -> {_fmt_type(pipe.return_type)}")


def _cmd_run(pipeline_name: str, raw_json: str, session: ReplSession) -> None:
    try:
        inputs = json.loads(raw_json)
        if not isinstance(inputs, dict):
            raise ValueError("JSON input must be an object.")
    except ValueError as exc:
        print(f"  Invalid JSON: {exc}")
        return

    program = session.program()
    if program is None:
        print("  No definitions in session. Use :load or define something first.")
        return

    if pipeline_name not in program.pipelines:
        available = ", ".join(program.pipelines) or "(none)"
        print(f"  Unknown pipeline '{pipeline_name}'. Available: {available}")
        return

    try:
        check_program(program)
        registry = default_task_registry(program, adapter_mode=session.adapter_mode)
        result = execute_pipeline(
            program=program,
            pipeline_name=pipeline_name,
            inputs=inputs,
            task_registry=registry,
            max_workers=session.max_workers,
        )
    except TypeCheckError as exc:
        print(f"  Type error: {exc}")
        return
    except AgentRuntimeError as exc:
        print(f"  Runtime error: {exc}")
        return
    except Exception as exc:  # noqa: BLE001
        print(f"  Error: {exc}")
        return

    print(json.dumps(result, indent=2))


# ---------------------------------------------------------------------------
# Type formatting helpers
# ---------------------------------------------------------------------------


def _fmt_type(t: TypeExpr) -> str:
    if isinstance(t, PrimitiveType):
        return t.name
    if isinstance(t, ListType):
        return f"List[{_fmt_type(t.item_type)}]"
    if isinstance(t, ObjType):
        inner = ", ".join(f"{k}: {_fmt_type(v)}" for k, v in t.fields)
        return f"Obj{{{inner}}}"
    return repr(t)


def _fmt_params(params: Any) -> str:
    return ", ".join(f"{p.name}: {_fmt_type(p.type_expr)}" for p in params)
