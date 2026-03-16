from __future__ import annotations

import argparse
import json
import sys

from agentlang import (
    ExecutionContext,
    PluginRegistry,
    check_program,
    default_task_registry,
    execute_pipeline,
    format_pipeline,
    load_plugin,
    lower_program,
    parse_program,
    run_tests,
)


def main() -> None:
    if len(sys.argv) >= 2 and sys.argv[1] == "repl":
        _repl(sys.argv[2:])
        return

    parser = argparse.ArgumentParser(
        description="Run AgentLang pipelines or workflows from .agent source files."
    )
    parser.add_argument("source", help="Path to .agent file")
    parser.add_argument("pipeline", nargs="?", default=None, help="Pipeline or workflow name to execute")
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
    parser.add_argument(
        "--output-trace",
        metavar="PATH",
        default=None,
        help="Write structured execution trace to the given JSON file.",
    )
    parser.add_argument(
        "--plugin",
        action="append",
        default=[],
        metavar="MODULE",
        help="Load a plugin module (repeatable). Module must have a register(registry) function.",
    )
    parser.add_argument(
        "--test",
        action="store_true",
        help="Run all test blocks in the source file instead of a pipeline.",
    )
    args = parser.parse_args()

    if not args.test and args.pipeline is None:
        parser.error("pipeline is required unless --test is specified")

    try:
        payload = json.loads(args.input)
        if not isinstance(payload, dict):
            raise ValueError("Input JSON must be an object.")
    except ValueError as exc:
        print(f"Invalid --input: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc

    ctx = None
    try:
        source_text = _read_text(args.source)
        raw_program = parse_program(source_text, lower=False)
        program = lower_program(raw_program)

        if args.lower:
            if args.pipeline is None:
                parser.error("pipeline is required with --lower")
            lowered = program.pipelines.get(args.pipeline)
            if lowered is None:
                raise ValueError(f"Unknown pipeline or workflow '{args.pipeline}'.")
            print(format_pipeline(lowered))
            return

        check_program(program)

        # Load plugins
        plugin_registry = PluginRegistry()
        for plugin_path in args.plugin:
            load_plugin(plugin_path, plugin_registry)

        # Build task registry, merging plugin handlers (plugin takes precedence)
        plugin_tool_handlers = plugin_registry.get_tool_handlers()
        task_reg = default_task_registry(
            program,
            adapter_mode=args.adapter,
            trace_live=args.trace_live,
            extra_tool_handlers=plugin_tool_handlers,
        )
        task_reg.update(plugin_registry.get_task_handlers())

        # Create execution context if tracing requested
        ctx = ExecutionContext() if args.output_trace else None

        if args.test:
            results = run_tests(
                program=program,
                task_registry=task_reg,
                max_workers=args.workers,
                ctx=ctx,
            )
            passed = sum(1 for r in results if r["passed"])
            failed = sum(1 for r in results if not r["passed"])
            for r in results:
                status = "PASS" if r["passed"] else "FAIL"
                line = f"  {status}: {r['name']}"
                if r["error"]:
                    line += f" — {r['error']}"
                print(line)
            print(f"\n{passed} passed, {failed} failed, {len(results)} total")

            if failed > 0:
                raise SystemExit(1)
            return

        result = execute_pipeline(
            program=program,
            pipeline_name=args.pipeline,
            inputs=payload,
            task_registry=task_reg,
            max_workers=args.workers,
            ctx=ctx,
        )

    except Exception as exc:  # noqa: BLE001 - CLI should show concise failures.
        print(f"Execution error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
    finally:
        if ctx is not None and args.output_trace:
            try:
                with open(args.output_trace, "w", encoding="utf-8") as f:
                    f.write(ctx.to_json())
            except Exception:
                print(f"Warning: failed to write trace to {args.output_trace}", file=sys.stderr)

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
