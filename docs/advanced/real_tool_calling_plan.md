# Real Tool Calling Plan

This plan upgrades AgentLang from a workflow DSL with tool metadata into a runtime that can execute real tool-calling agent tasks while preserving the language's typed orchestration model.

## Goals

1. Make tools first-class DSL/runtime objects.
2. Preserve explicit pipeline orchestration in the DSL.
3. Add an execution path where agent tasks can call tools and must still satisfy declared task types.
4. Keep deterministic task handlers and agent-driven task handlers as separate execution modes.

## Implementation Order

### 1. First-class tool declarations

- Add `tool` declarations to the AST and parser.
- A tool declaration will look like:

  ```agentlang
  tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}
  ```

- Tools are declarations only; execution remains in the runtime registry.
- Checker responsibilities:
  - enforce unique tool names
  - allow agents to reference only declared tools

### 2. Runtime tool registry

- Add a runtime tool registry parallel to the existing task registry.
- Each tool runtime entry must include:
  - typed argument contract from the DSL declaration
  - typed return contract from the DSL declaration
  - Python handler implementation
- Runtime must validate tool input values before calling handlers and validate tool outputs after handler execution.

### 3. OpenAI tool-calling loop

- Extend the OpenAI adapter from plain text completion to a loop that supports:
  - tool definitions in the request
  - model-emitted tool calls
  - dispatching tool calls into the runtime tool registry
  - feeding tool outputs back into the model until final output is produced
- Add execution limits:
  - max tool calls per task
  - clear error on unknown tool name or invalid tool args/output

### 4. Agent-task execution mode

- Add an explicit agent-task syntax:

  ```agentlang
  task investigate(topic: String) -> Obj{summary: String, sources: List[String]} by agent {}
  ```

- Semantics:
  - deterministic tasks continue to use Python task handlers
  - `by agent` tasks are solved by the model bound in the run statement's agent
  - the runtime exposes only the tools allowed by that agent declaration

### 5. Runtime contract enforcement

- For agent tasks and tools, validate:
  - task args
  - tool args
  - tool outputs
  - final agent task output
  - pipeline return values
- Structured outputs must conform to the declared DSL return type.

### 6. Examples after each slice

- After tool declarations: add a syntax-only example and run parse/type-check validation.
- After the tool registry: add a deterministic tool-backed example and run it in mock mode.
- After the OpenAI tool loop and agent tasks: add a real live-capable example and run it in mock mode and live mode.

### 7. Tests

- Add tests for:
  - parsing `tool`
  - agent references to undeclared tools
  - invalid tool args/output
  - unknown tool call requested by the model
  - max-tool-call budget
  - agent task output validation
  - end-to-end tool-calling happy path

## Non-Goals For This Pass

- Durable execution
- Arbitrary user-defined prompt blocks in the DSL
- General loops or recursive task execution
- Provider-specific syntax in the language

## Design Default

AgentLang remains an orchestration DSL first. Tool calling is a runtime capability attached to agent tasks, not a replacement for explicit pipelines.
