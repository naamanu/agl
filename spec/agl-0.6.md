# AGL 0.6 language specification

Status: draft normative

Language version: `0.6`

AGL 0.6 extends [AGL 0.5](agl-0.5.md) with modules, reproducible packages, editor protocols, and stable host contracts.

## 1. Modules and visibility

`import util from "util.agent";` resolves relative to the importing file after canonicalizing its path. Imported declarations use `util::name`. Imports are deterministic and cycles are rejected with the complete cycle path.

Every 0.6 top-level declaration is explicitly `public` or `private`; omitted visibility is private. Only public names may be referenced by an importer. Private declarations are still compiled under the module namespace for internal calls. A module interface contains serialized public signatures, inferred effects, language version, and a content fingerprint. Interfaces are cached in `.agl-cache` and can be validated without execution.

## 2. Packages

`agl.json` declares package name, semantic version, language version, entry module, and dependencies. A dependency is either a local path or Git URL pinned to an immutable revision. `agl.lock` is canonical JSON ordered by dependency name and records source, revision, and content integrity. Local content is hashed from sorted relative paths and bytes. Git entries require both a pinned revision and integrity before materialization.

Exported API compatibility compares module-interface declarations. Removing or changing a public signature/effect contract is breaking; adding a public declaration is compatible. A public registry is not part of 0.6.

## 3. Tooling

The canonical formatter emits an equivalent program in declaration order normalized by kind/name. The compiler JSON-lines protocol supports check, format, lower, effects, completion, hover, definition, references, and rename. The LSP maps those operations to standard editor requests and publishes source diagnostics. `tree-sitter-agl` supplies incremental syntax parsing and highlighting. The CLI emits Bash, Zsh, and Fish completions and structured JSON summaries.

## 4. Embedding and extensions

The Rust extension API version is `1`. Stable contracts cover task handlers, tool handlers, model adapters, hosts, event stores, policy resolvers, and graders. Descriptors must match the host API version before registration. Native compile-time registration remains the default.

The language implementation and extension surface are Rust-only. The former Python subprocess bridge, protocol envelopes, and CLI `--plugin` option have been removed. Hosts register task and tool handlers through native registries. WASI components remain deferred until evidence shows native contracts are insufficient.

## 5. Deployment security

A deployment policy can restrict effects, tools, network hosts, and filesystem roots and can require approval for external writes. Static summaries expose each pipeline's inferred effects, external-write status, and approval boundary. Secrets are redacted before trace/event persistence. Native Rust is trusted in-process; future WASI extensions would receive explicit capabilities only.

## 6. Compatibility

Imports, explicit visibility, package manifests, and 0.6 tooling require `language "0.6";`. Programs from 0.2–0.5 keep their top-level declarations effectively public for compatibility.
