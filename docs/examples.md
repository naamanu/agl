# Examples Guide

This project includes five `.agent` examples under `examples/`.

## 1) `examples/blog.agent`

Goal:

- simple sequential pipeline: research -> draft

Run:

```bash
python main.py examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'
```

## 2) `examples/compare.agent`

Goal:

- run two research tasks in parallel
- compare results in a downstream step

Run:

```bash
python main.py examples/compare.agent compare_options --input '{"query":"vector database"}'
```

## 3) `examples/support.agent`

Goal:

- classify user request intent/urgency
- route request and generate response

Run:

```bash
python main.py examples/support.agent support_reply --input '{"message":"urgent refund request"}'
```

## 4) `examples/reliability.agent`

Goal:

- demonstrate retries and fallback policy
- demonstrate `if/else` routing based on fallback detection

Run (succeeds before fallback):

```bash
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":1}'
```

Run (forces fallback):

```bash
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":5}'
```

## 5) `examples/live_answer.agent`

Goal:

- minimal single-task flow for adapter validation

Run (mock):

```bash
python main.py examples/live_answer.agent answer --input '{"question":"What is an agentic workflow?"}'
```

Run (live):

```bash
export OPENAI_API_KEY="..."
python main.py examples/live_answer.agent answer --adapter live --input '{"question":"What is an agentic workflow?"}'
```

## Tips For Authoring New Examples

- keep task signatures explicit and small
- choose pipeline return type intentionally
- include one example that exercises each new language feature
- add both happy-path and failure-path inputs for reliability features

