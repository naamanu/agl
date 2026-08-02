# Modules and packages

An AGL 0.6 module exports only declarations marked `public`:

```agentlang
language "0.6";
import text from "text.agent";

public pipeline report(input: String) -> String {
  let output = text::normalize(input);
  return output;
}
```

Paths resolve relative to the importer. Qualified names are deterministic, cycles show their full path, private imports fail, and `.agl-cache` stores validated public interfaces.

Packages use `agl.json` and a deterministic `agl.lock`. Run `agl package lock agl.json`. Local dependencies are hashed from their contents. Git dependencies must pin a revision and integrity. Generate an API snapshot with `--api api.json`; compare releases with `agl api-compare previous.json current.json`.
