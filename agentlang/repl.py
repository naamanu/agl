from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from prompt_toolkit import PromptSession
from prompt_toolkit.formatted_text import HTML
from prompt_toolkit.history import FileHistory
from rich import box
from rich.columns import Columns
from rich.console import Console
from rich.json import JSON as RichJSON
from rich.markup import escape
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

from .ast import ListType, ObjType, PrimitiveType, TypeExpr
from .checker import TypeCheckError, check_program
from .lexer import LexError
from .parser import ParseError, parse_program
from .runtime import AgentRuntimeError, execute_pipeline
from .stdlib import default_task_registry

console = Console()

_HISTORY_FILE = Path.home() / ".agentlang_history"

# ---------------------------------------------------------------------------
# Session
# ---------------------------------------------------------------------------


@dataclass
class ReplSession:
    """Holds accumulated definitions as (source_fragment, defined_keys) pairs."""

    adapter_mode: str = "mock"
    max_workers: int = 8
    _entries: list[tuple[str, frozenset[str]]] = field(default_factory=list)

    def add_fragment(self, text: str) -> tuple[list[str], list[str]]:
        """Parse *text*, merge into the session, type-check the whole program.

        Returns (added_names, redefined_names) on success.
        Raises LexError, ParseError, or TypeCheckError on failure (unchanged).
        """
        mini = parse_program(text)
        new_keys: frozenset[str] = frozenset(
            {f"agent:{n}" for n in mini.agents}
            | {f"task:{n}" for n in mini.tasks}
            | {f"pipeline:{n}" for n in mini.pipelines}
        )
        if not new_keys:
            raise ParseError("No definitions found in input.")

        redefined: list[str] = []
        kept: list[tuple[str, frozenset[str]]] = []
        for frag_text, frag_keys in self._entries:
            overlap = frag_keys & new_keys
            if overlap:
                redefined.extend(k.split(":", 1)[1] for k in sorted(overlap))
            else:
                kept.append((frag_text, frag_keys))

        candidate_entries = kept + [(text, new_keys)]
        full_source = "\n\n".join(t for t, _ in candidate_entries)
        full_program = parse_program(full_source)
        check_program(full_program)

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
    prompt_session: PromptSession = PromptSession(  # type: ignore[type-arg]
        history=FileHistory(str(_HISTORY_FILE))
    )

    _print_banner()

    while True:
        try:
            raw = _read_block(prompt_session)
        except EOFError:
            console.print("\n[dim]Bye![/dim]")
            break

        line = raw.strip()
        if not line:
            continue

        if line.startswith(":"):
            if _handle_command(line, session):
                break
        else:
            _handle_definition(line, session)


# ---------------------------------------------------------------------------
# Banner & help
# ---------------------------------------------------------------------------


def _print_banner() -> None:
    title = Text.assemble(
        ("AgentLang ", "bold cyan"),
        ("REPL", "bold white"),
    )
    subtitle = Text.assemble(
        ("type ", "dim"),
        (":help", "cyan"),
        (" for commands  •  Ctrl-D to exit", "dim"),
    )
    console.print(
        Panel(
            subtitle,
            title=title,
            border_style="cyan dim",
            expand=False,
            padding=(0, 2),
        )
    )
    console.print()


def _print_help(adapter_mode: str) -> None:
    table = Table(box=box.SIMPLE, show_header=False, padding=(0, 2))
    table.add_column(style="cyan bold", no_wrap=True)
    table.add_column(style="dim")

    commands = [
        (":run <pipeline> [json]", "Execute a pipeline  (json defaults to {})"),
        (":load <path>",           "Load a .agent file into the session"),
        (":agents",                "List defined agents"),
        (":tasks",                 "List defined tasks"),
        (":pipelines",             "List defined pipelines"),
        (f":adapter mock|live",    f"Switch adapter mode  (currently [cyan]{adapter_mode}[/cyan])"),
        (":reset",                 "Clear all definitions"),
        (":help",                  "Show this help"),
        (":quit / :exit",          "Exit the REPL"),
    ]
    for cmd, desc in commands:
        table.add_row(cmd, desc)

    defs = Text.assemble(
        ("agent  ", "cyan bold"), ("<name> { model: \"...\", tools: [...] }\n", "dim"),
        ("task   ", "cyan bold"), ("<name>(<params>) -> <type> {}\n",           "dim"),
        ("pipeline ", "cyan bold"), ("<name>(<params>) -> <type> { ... }",      "dim"),
    )

    console.print(
        Panel(
            Columns([defs, table], equal=False, expand=False),
            title="[bold]Help[/bold]",
            border_style="dim",
            padding=(1, 2),
        )
    )
    console.print(
        "[dim]Note: task handlers come from the stdlib adapter. "
        "Custom task names outside the built-in set will raise a runtime error on :run.[/dim]"
    )


# ---------------------------------------------------------------------------
# Input reading
# ---------------------------------------------------------------------------


def _read_block(prompt_session: PromptSession) -> str:  # type: ignore[type-arg]
    """Read one complete block (brace-balanced) or a single command line."""
    try:
        first: str = prompt_session.prompt(
            HTML("<ansibrightcyan><b>>>> </b></ansibrightcyan>")
        )
    except KeyboardInterrupt:
        console.print()
        return ""

    stripped = first.strip()
    if stripped.startswith(":") or not stripped or _brace_depth(first) == 0:
        return first

    lines = [first]
    while True:
        try:
            cont: str = prompt_session.prompt(
                HTML("<ansigray>... </ansigray>")
            )
        except KeyboardInterrupt:
            console.print()
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
                i += 2
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
        console.print(f"  [bold red]Lex error:[/] {exc}")
        return
    except ParseError as exc:
        console.print(f"  [bold red]Parse error:[/] {exc}")
        return
    except TypeCheckError as exc:
        console.print(f"  [bold red]Type error:[/] {exc}")
        return

    for name in redefined:
        console.print(f"  [dim]Redefined[/dim] [yellow]{name}[/yellow]")
    for name in added:
        console.print(f"  [dim]Defined[/dim] [bold green]{name}[/bold green]")


# ---------------------------------------------------------------------------
# Command handling
# ---------------------------------------------------------------------------


def _handle_command(line: str, session: ReplSession) -> bool:
    """Returns True if the REPL should exit."""
    parts = line.split(None, 2)
    cmd = parts[0].lower()

    if cmd in (":quit", ":exit"):
        console.print("[dim]Bye![/dim]")
        return True

    if cmd == ":help":
        _print_help(session.adapter_mode)
        return False

    if cmd == ":reset":
        session.reset()
        console.print("[dim]  Session cleared.[/dim]")
        return False

    if cmd == ":adapter":
        if len(parts) < 2:
            console.print("  [dim]Usage:[/dim] :adapter mock|live")
            return False
        mode = parts[1].lower()
        if mode not in {"mock", "live"}:
            console.print("  [bold red]Adapter must be[/] 'mock' or 'live'.")
            return False
        session.adapter_mode = mode
        console.print(f"  [dim]Adapter mode:[/dim] [cyan]{mode}[/cyan]")
        return False

    if cmd == ":load":
        if len(parts) < 2:
            console.print("  [dim]Usage:[/dim] :load <path>")
            return False
        _cmd_load(parts[1], session)
        return False

    if cmd in (":agents", ":tasks", ":pipelines"):
        _cmd_list(session, cmd[1:])
        return False

    if cmd == ":run":
        if len(parts) < 2:
            console.print("  [dim]Usage:[/dim] :run <pipeline> [json]")
            return False
        raw_json = parts[2] if len(parts) > 2 else "{}"
        _cmd_run(parts[1], raw_json, session)
        return False

    console.print(f"  [bold red]Unknown command[/] '{cmd}'. Type :help for commands.")
    return False


def _cmd_load(path: str, session: ReplSession) -> None:
    try:
        text = open(path, encoding="utf-8").read()
    except OSError as exc:
        console.print(f"  [bold red]Cannot read[/] '{path}': {exc}")
        return

    try:
        added, redefined = session.add_fragment(text)
    except LexError as exc:
        console.print(f"  [bold red]Lex error[/] in '{path}': {exc}")
        return
    except ParseError as exc:
        console.print(f"  [bold red]Parse error[/] in '{path}': {exc}")
        return
    except TypeCheckError as exc:
        console.print(f"  [bold red]Type error[/] in '{path}': {exc}")
        return

    na = len(added)
    summary = f"[dim]{na} definition{'s' if na != 1 else ''} added[/dim]"
    console.print(f"  [green]Loaded[/green] '[bold]{path}[/bold]'  ({summary})")
    for name in added:
        console.print(f"    [green]+[/green] [green]{name}[/green]")
    if redefined:
        names = ", ".join(f"[yellow]{escape(n)}[/yellow]" for n in sorted(redefined))
        console.print(f"  [yellow]Redefined:[/yellow] {names}")


def _cmd_list(session: ReplSession, kind: str) -> None:
    program = session.program()
    if program is None:
        console.print("[dim]  (empty session — nothing defined yet)[/dim]")
        return

    items: dict[str, Any] = getattr(program, kind)
    if not items:
        console.print(f"[dim]  No {kind} defined.[/dim]")
        return

    table = Table(box=box.SIMPLE, show_header=False, padding=(0, 1))

    if kind == "agents":
        table.add_column(style="bold cyan", no_wrap=True)
        table.add_column(no_wrap=True)
        table.add_column(no_wrap=True)
        for name, agent in items.items():
            tools_str = escape(", ".join(agent.tools)) if agent.tools else "[dim]none[/dim]"
            table.add_row(
                name,
                f"[dim]model=[/dim][cyan]{escape(agent.model)}[/cyan]",
                f"[dim]tools=[[/dim]{tools_str}[dim]][/dim]",
            )

    elif kind == "tasks":
        table.add_column(style="dim",      no_wrap=True)
        table.add_column(style="bold",     no_wrap=True)
        table.add_column(no_wrap=True)
        for name, task in items.items():
            sig = _fmt_params(task.params)
            ret = _fmt_type(task.return_type)
            table.add_row("task", name, f"({sig}) [dim]->[/dim] {ret}")

    elif kind == "pipelines":
        table.add_column(style="dim",      no_wrap=True)
        table.add_column(style="bold",     no_wrap=True)
        table.add_column(no_wrap=True)
        for name, pipe in items.items():
            sig = _fmt_params(pipe.params)
            ret = _fmt_type(pipe.return_type)
            table.add_row("pipeline", name, f"({sig}) [dim]->[/dim] {ret}")

    console.print(table)


def _cmd_run(pipeline_name: str, raw_json: str, session: ReplSession) -> None:
    try:
        inputs = json.loads(raw_json)
        if not isinstance(inputs, dict):
            raise ValueError("JSON input must be an object.")
    except ValueError as exc:
        console.print(f"  [bold red]Invalid JSON:[/] {exc}")
        return

    program = session.program()
    if program is None:
        console.print("[dim]  No definitions in session. Use :load or define something first.[/dim]")
        return

    if pipeline_name not in program.pipelines:
        available = ", ".join(f"[cyan]{escape(n)}[/cyan]" for n in program.pipelines) or "[dim]none[/dim]"
        console.print(
            f"  [bold red]Unknown pipeline[/] '[bold]{pipeline_name}[/bold]'."
            f" Available: {available}"
        )
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
        console.print(f"  [bold red]Type error:[/] {exc}")
        return
    except AgentRuntimeError as exc:
        console.print(f"  [bold red]Runtime error:[/] {exc}")
        return
    except Exception as exc:  # noqa: BLE001
        console.print(f"  [bold red]Error:[/] {exc}")
        return

    console.print(Panel(RichJSON(json.dumps(result)), border_style="dim green", expand=False))


# ---------------------------------------------------------------------------
# Type formatting helpers
# ---------------------------------------------------------------------------


def _fmt_type(t: TypeExpr) -> str:
    if isinstance(t, PrimitiveType):
        return f"[yellow]{t.name}[/yellow]"
    if isinstance(t, ListType):
        return f"[dim]List[[/dim]{_fmt_type(t.item_type)}[dim]][/dim]"
    if isinstance(t, ObjType):
        inner = "[dim], [/dim]".join(
            f"[dim]{k}:[/dim] {_fmt_type(v)}" for k, v in t.fields
        )
        return f"[dim]Obj{{[/dim]{inner}[dim]}}[/dim]"
    return repr(t)


def _fmt_params(params: Any) -> str:
    return "[dim], [/dim]".join(
        f"[dim]{p.name}:[/dim] {_fmt_type(p.type_expr)}" for p in params
    )
