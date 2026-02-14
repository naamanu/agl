from __future__ import annotations

import argparse
import json
import sys

from agentlang import check_program, default_task_registry, execute_pipeline, parse_program


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run AgentLang pipelines from .agent source files."
    )
    parser.add_argument("source", help="Path to .agent file")
    parser.add_argument("pipeline", help="Pipeline name to execute")
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
        program = parse_program(source_text)
        check_program(program)

        result = execute_pipeline(
            program=program,
            pipeline_name=args.pipeline,
            inputs=payload,
            task_registry=default_task_registry(program, adapter_mode=args.adapter),
            max_workers=args.workers,
        )
    except Exception as exc:  # noqa: BLE001 - CLI should show concise failures.
        print(f"Execution error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc

    print(json.dumps({"result": result}, indent=2))


def _read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


if __name__ == "__main__":
    main()
