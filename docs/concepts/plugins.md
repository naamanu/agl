# Native handlers

AGL applications extend the runtime through Rust registries. Task declarations
specify types in the DSL; a host supplies their implementations before execution.

## Register a task

```rust
let mut registry = agl::Registry::default();
registry.register("uppercase", |args, _agent| {
    let text = args["text"].as_str().unwrap_or_default();
    Ok(serde_json::json!({"text": text.to_uppercase()}))
});
```

Pass the registry to `agl::execute_pipeline` or `agl::run_tests`. Use
`Registry::register_contextual` when a handler needs invocation identity,
cancellation, usage reporting, or structured failures.

The executable example includes parsing, checking, registration, and execution:

```bash
cargo run --example native_embed
```

## Register a tool

Model-callable tools use `agl::adapters::tools::ToolRegistry`. Register their
handlers before passing the tool registry to
`agl::stdlib::registry_for_with_tools`. Declare tool signatures in the DSL so
arguments and results can be validated.

## Execution boundary

Custom handlers are compiled into a Rust host application. The stock CLI uses
built-in handlers and native provider adapters. It does not load external
handler modules. Native handlers share the host process and its permissions.

See [native extensions](../native-extensions.md) for extension contracts and
[testing](testing.md) for in-language test blocks.
