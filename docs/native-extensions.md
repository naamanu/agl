# Native extensions

Rust applications extend AGL by depending on the `agl` crate and registering
handlers at startup:

```rust
let mut registry = agl::Registry::default();
registry.register("uppercase", |args, _agent| {
    let text = args["text"].as_str().unwrap_or_default();
    Ok(serde_json::json!({"text": text.to_uppercase()}))
});
```

Run the complete embedding example with:

```bash
cargo run --example native_embed
```

Register tasks on `Registry` and model-callable tools on `ToolRegistry`. Pass
custom tools to `registry_for_with_tools` before running pipelines with a native
provider adapter. Contextual task handlers receive an `Invocation` and return
`TaskOutput` or `HandlerFailure`.

The versioned traits in `agl::extension` cover task handlers, tools, model
adapters, hosts, policies, and graders. Validate extension descriptors against
`EXTENSION_API_VERSION` before registration.

Custom handlers are compiled into the host application. AGL does not load Rust
dynamic libraries or external handler modules. Native extensions share the
host process and permissions.

For removed host APIs and migration steps, see the
[Rust-only migration notes](migrations/0.3-to-0.6.md#rust-only-implementation).
