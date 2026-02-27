from __future__ import annotations

import argparse
import json
import sys

from agentlang import check_program, default_task_registry, execute_pipeline, parse_program
from agentlang.repl import run_repl


def main() -> None:
    top = argparse.ArgumentParser(
        description="AgentLang CLI — run pipelines or start the interactive REPL.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python main.py run examples/blog.agent blog_post --input '{\"topic\":\"AI\"}'\n"
            "  python main.py repl --adapter mock"
        ),
    )
    sub = top.add_subparsers(dest="command", required=True)

    # ------------------------------------------------------------------ run
    run_p = sub.add_parser(
        "run",
        help="Execute a pipeline from a .agent source file.",
    )
    run_p.add_argument("source", help="Path to .agent file")
    run_p.add_argument("pipeline", help="Pipeline name to execute")
    run_p.add_argument(
        "--input",
        default="{}",
        help='JSON object of pipeline inputs, e.g. \'{"topic":"agent memory"}\'',
    )
    run_p.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Maximum worker threads for parallel tasks (default: 8)",
    )
    run_p.add_argument(
        "--adapter",
        choices=["mock", "live"],
        default=None,
        help="Task adapter: mock (deterministic stubs) or live (OpenAI + tools)",
    )

    # ------------------------------------------------------------------ repl
    repl_p = sub.add_parser(
        "repl",
        help="Start the interactive AgentLang REPL.",
    )
    repl_p.add_argument(
        "--adapter",
        choices=["mock", "live"],
        default=None,
        help="Task adapter mode (default: mock, or AGENTLANG_ADAPTER env var)",
    )
    repl_p.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Maximum worker threads for parallel tasks (default: 8)",
    )

    args = top.parse_args()

    if args.command == "run":
        _cmd_run(args)
    elif args.command == "repl":
        _cmd_repl(args)


def _cmd_run(args: argparse.Namespace) -> None:
    try:
        payload = json.loads(args.input)
        if not isinstance(payload, dict):
            raise ValueError("Input JSON must be an object.")
    except ValueError as exc:
        print(f"Invalid --input: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc

    try:
        source_text = _read_text(args.source)
        program = parse_program(source_text)
        check_program(program)

        result = execute_pipeline(
            program=program,
            pipeline_name=args.pipeline,
            inputs=payload,
            task_registry=default_task_registry(program, adapter_mode=args.adapter),
            max_workers=args.workers,
        )
    except Exception as exc:  # noqa: BLE001 - CLI shows concise failures.
        print(f"Execution error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc

    print(json.dumps({"result": result}, indent=2))


def _cmd_repl(args: argparse.Namespace) -> None:
    run_repl(
        adapter_mode=args.adapter or "mock",
        max_workers=args.workers,
    )


def _read_text(path: str) -> str:
    with open(path, encoding="utf-8") as f:
        return f.read()


if __name__ == "__main__":
    main()
