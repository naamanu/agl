"""
Parallelism microbenchmark for AgentLang v0.

Compares sequential vs parallel execution of two equal-latency task calls.
Each task sleeps for SLEEP_S seconds to simulate I/O-bound work.

Usage (from repository root):
    python3.14 paper/benchmark.py
"""

from __future__ import annotations

import statistics
import sys
import time
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from agentlang import check_program, execute_pipeline, parse_program

SLEEP_S = 0.1
RUNS = 10

_SOURCE = """
agent bench {
  model: "mock"
  , tools: []
}

task slow_task(label: String) -> Obj{result: String} {}

pipeline sequential(label_a: String, label_b: String) -> String {
  let a = run slow_task with { label: label_a } by bench;
  let b = run slow_task with { label: label_b } by bench;
  return a.result + " " + b.result;
}

pipeline parallel_join(label_a: String, label_b: String) -> String {
  parallel {
    let a = run slow_task with { label: label_a } by bench;
    let b = run slow_task with { label: label_b } by bench;
  } join;
  return a.result + " " + b.result;
}
"""


def _slow_task(args: dict, _agent: str | None) -> dict:
    time.sleep(SLEEP_S)
    return {"result": f"done:{args['label']}"}


_REGISTRY: dict = {"slow_task": _slow_task}


def _bench(pipeline_name: str, program, inputs: dict, n: int) -> list[float]:
    times = []
    for _ in range(n):
        t0 = time.perf_counter()
        execute_pipeline(program, pipeline_name, inputs, _REGISTRY)
        times.append(time.perf_counter() - t0)
    return times


def main() -> None:
    program = parse_program(_SOURCE)
    check_program(program)
    inputs = {"label_a": "A", "label_b": "B"}

    print(f"Benchmark: {RUNS} runs, sleep={SLEEP_S}s per task, 2 tasks per pipeline")
    print()

    seq = _bench("sequential", program, inputs, RUNS)
    par = _bench("parallel_join", program, inputs, RUNS)

    seq_mean = statistics.mean(seq)
    par_mean = statistics.mean(par)
    speedup = seq_mean / par_mean

    print(f"Sequential  mean: {seq_mean:.4f}s  (stdev {statistics.stdev(seq):.4f}s)")
    print(f"Parallel    mean: {par_mean:.4f}s  (stdev {statistics.stdev(par):.4f}s)")
    print(f"Speedup:          {speedup:.2f}x")


if __name__ == "__main__":
    main()
