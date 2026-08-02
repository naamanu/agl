# Agent requirements and deployment bindings

AGL 0.4 separates what an agent program needs from which provider happens to satisfy it.

Source declares portable requirements:

```agentlang
agent researcher {
  tools: [],
  requires: [reasoning, tool_calling],
  min_context: 100000,
  max_latency_ms: 5000,
  quality: "high"
}
```

A deployment JSON file selects operational details:

```json
{
  "agents": {
    "researcher": {
      "provider": "openai",
      "model": "gpt-5.6-sol",
      "reasoning_effort": "medium",
      "capabilities": ["reasoning", "tool_calling"],
      "context_window": 200000,
      "expected_latency_ms": 3000,
      "quality": "frontier"
    }
  }
}
```

Run with:

```bash
cargo run -- examples/deployment_requirements.agent answer \
  --adapter openai \
  --deployment examples/deployment.openai.json \
  --input '{"topic":"typed agent runtimes"}'
```

The compiler/runtime validates every source agent has a binding, provider matches the selected adapter, capabilities are present, context is large enough, latency is within the ceiling, and quality meets the requested tier. Validation occurs before registry construction or a provider call.

Bindings also supply an optional endpoint and reasoning effort. A deployment currently uses at most one endpoint per provider. API keys remain environment variables and never belong in deployment files.

The source-level `model` field remains a compatibility escape hatch when no deployment file is supplied. Deployment bindings and environment model overrides let production systems upgrade models without editing language source.
