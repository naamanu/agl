# Getting Started

## Prerequisites

- Python `3.14+`
- Terminal access in this repository root

## Project Layout

```text
agentlang/
  adapters/
  ast.py
  checker.py
  lexer.py
  parser.py
  runtime.py
  stdlib.py
examples/
  blog.agent
  compare.agent
  live_answer.agent
  reliability.agent
  support.agent
main.py
```

## First Run (Mock Mode)

`mock` mode is the default and does not require external API keys.

```bash
python main.py examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'
```

Expected shape of output:

```json
{
  "result": "..."
}
```

## CLI Usage

```bash
python main.py <source.agent> <pipeline_name> [--input '<json>'] [--workers N] [--adapter mock|live]
```

Arguments:

- `source`: path to `.agent` file
- `pipeline`: pipeline name in that file
- `--input`: JSON object mapped to pipeline params
- `--workers`: max thread workers for parallel blocks (must be `>= 1`)
- `--adapter`: runtime adapter mode (`mock` or `live`)

Input validation is strict:
- required pipeline inputs must be present
- unknown extra input keys are rejected
- input values must match declared DSL types (`String`, `Number`, `Bool`, `List[...]`, `Obj{...}`)

## Quick Examples

```bash
python main.py examples/compare.agent compare_options --input '{"query":"vector database"}'
python main.py examples/support.agent support_reply --input '{"message":"urgent refund request"}'
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":5}'
```

## Live Mode (OpenAI + Tool Adapters)

```bash
export OPENAI_API_KEY="..."
python main.py examples/blog.agent blog_post \
  --adapter live \
  --input '{"topic":"agent memory patterns"}'
```

If `OPENAI_API_KEY` is missing in live mode, execution fails with:

```text
Execution error: OPENAI_API_KEY is required when adapter mode is 'live'.
```
