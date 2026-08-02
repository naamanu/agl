# Releasing the Rust implementation

## Required checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
python3 -m unittest discover -s tests
cargo bench --bench runtime
cargo package --locked
```

The manually triggered **Live Provider Smoke** GitHub workflow must pass once for OpenAI and once for Anthropic before a production release. Configure repository secrets `OPENAI_API_KEY` and `ANTHROPIC_API_KEY`; optional repository variables `AGL_OPENAI_MODEL` and `AGL_ANTHROPIC_MODEL` select models without code changes.

## Release sequence

1. Update `CHANGELOG.md` and confirm the version in `Cargo.toml`.
2. Run the required checks and both live-provider workflows.
3. Merge the reviewed branch into `main`.
4. Create an annotated `v0.2.0` tag from the merge commit.
5. Run `cargo publish --dry-run`, then publish only with explicit maintainer approval.
6. Create the GitHub release from the tag using the matching changelog section.

Publishing and tagging are intentionally manual external actions.
