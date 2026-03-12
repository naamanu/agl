from __future__ import annotations

import argparse
import json
import sys

from agentlang import (
    check_program,
    default_task_registry,
    execute_pipeline,
    format_pipeline,
    lower_program,
    parse_program,
)


def main() -> None:
    if len(sys.argv) >= 2 and sys.argv[1] == "repl":
        _repl(sys.argv[2:])
        return

    parser = argparse.ArgumentParser(
        description="Run AgentLang pipelines or workflows from .agent source files."
    )
    parser.add_argument("source", help="Path to .agent file")
    parser.add_argument("pipeline", help="Pipeline or workflow name to execute")
    parser.add_argument(
        "--input",
        default="{}",
        help='JSON object for pipeline inputs, e.g. \'{"topic":"agent memory"}\'',
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=8,
        help="Maximum worker threads for runnable tasks",
    )
    parser.add_argument(
        "--adapter",
        choices=["mock", "live"],
        default=None,
        help=(
            "Task adapter mode. "
            "mock = deterministic local handlers, "
            "live = OpenAI + real tool adapters."
        ),
    )
    parser.add_argument(
        "--lower",
        action="store_true",
        help="Print the lowered pipeline IR for the selected pipeline/workflow and exit.",
    )
    parser.add_argument(
        "--trace-live",
        action="store_true",
        help="Emit live model/tool tracing to stderr when running with --adapter live.",
    )
    args = parser.parse_args()

    try:
        payload = json.loads(args.input)
        if not isinstance(payload, dict):
            raise ValueError("Input JSON must be an object.")
    except ValueError as exc:
        print(f"Invalid --input: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc

    try:
        source_text = _read_text(args.source)
        raw_program = parse_program(source_text, lower=False)
        program = lower_program(raw_program)

        if args.lower:
            lowered = program.pipelines.get(args.pipeline)
            if lowered is None:
                raise ValueError(f"Unknown pipeline or workflow '{args.pipeline}'.")
            print(format_pipeline(lowered))
            return

        check_program(program)

        result = execute_pipeline(
            program=program,
            pipeline_name=args.pipeline,
            inputs=payload,
            task_registry=default_task_registry(
                program,
                adapter_mode=args.adapter,
                trace_live=args.trace_live,
            ),
            max_workers=args.workers,
        )
    except Exception as exc:  # noqa: BLE001 - CLI should show concise failures.
        print(f"Execution error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc

    print(json.dumps({"result": result}, indent=2))


def _repl(argv: list[str]) -> None:
    parser = argparse.ArgumentParser(prog="main.py repl")
    parser.add_argument(
        "--adapter",
        choices=["mock", "live"],
        default=None,
        help="Task adapter mode (default: mock).",
    )
    parser.add_argument(
        "--trace-live",
        action="store_true",
        help="Emit live model/tool tracing to stderr when running with --adapter live.",
    )
    args = parser.parse_args(argv)
    adapter = args.adapter
    trace_live = args.trace_live

    try:
        import readline  # noqa: F401 — enables arrow-key history on supported platforms
    except ImportError:
        pass

    print(f"AgentLang REPL (adapter={adapter or 'mock'}). Type 'exit' to quit.")
    while True:
        try:
            line = input("> ").strip()
        except (EOFError, KeyboardInterrupt):
            print()
            break
        if not line:
            continue
        if line == "exit":
            break

        parts = line.split(None, 2)
        if len(parts) < 2:
            print("Usage: <source_file> <pipeline_name> [json_input]", file=sys.stderr)
            continue

        source_path, pipeline_name = parts[0], parts[1]
        input_json = parts[2] if len(parts) > 2 else "{}"

        try:
            payload = json.loads(input_json)
            if not isinstance(payload, dict):
                raise ValueError("Input JSON must be an object.")
        except ValueError as exc:
            print(f"Invalid input: {exc}", file=sys.stderr)
            continue

        try:
            source_text = _read_text(source_path)
            program = parse_program(source_text)
            check_program(program)
            result = execute_pipeline(
                program=program,
                pipeline_name=pipeline_name,
                inputs=payload,
                task_registry=default_task_registry(
                    program,
                    adapter_mode=adapter,
                    trace_live=trace_live,
                ),
            )
            print(json.dumps({"result": result}, indent=2))
        except Exception as exc:  # noqa: BLE001 - REPL should stay alive after errors.
            print(f"Error: {exc}", file=sys.stderr)


def _read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


if __name__ == "__main__":
    main()
