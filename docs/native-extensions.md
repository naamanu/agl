# Native extensions and plugin migration

Rust applications should extend AGL by depending on the `agl` crate and registering handlers at startup. This is the stable native extension boundary:

```rust
let mut registry = agl::Registry::default();
registry.register("uppercase", |args, _agent| {
    let text = args["text"].as_str().unwrap_or_default();
    Ok(serde_json::json!({"text": text.to_uppercase()}))
});
```

Run the complete example with:

```bash
cargo run --example native_embed
```

AGL deliberately does not load Rust dynamic libraries. Rust does not provide a stable ABI for arbitrary trait objects and closures, so a `libloading`-style interface would couple plugins to an exact compiler and crate build. Compile-time registration is safer, easier to test, and works across supported platforms.

Existing Python plugins remain supported through `--plugin`. The Rust CLI discovers their task and tool registrations, then invokes handlers in isolated Python subprocesses over JSON. This bridge is intended for migration and requires `python3` at runtime.

Migration steps:

1. Add `agl` and `serde_json` to the host Rust application.
2. Translate each Python handler into a Rust closure or function.
3. Register task handlers on `Registry`; register model-callable tools on `ToolRegistry`.
4. Run the existing `.agent` test blocks against the native registries.
5. Remove `--plugin` only after result and error behavior match.
