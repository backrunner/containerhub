# Contributing

ContainerHub is in active development. Small, focused changes are easier to
review. Open an issue before changing provider contracts, storage formats,
identity rules or the direction of the interface.

## Development

Start with the [README](README.md) and [architecture](docs/ARCHITECTURE.md).
Use explicit Demo mode or the isolated CLI fixtures while developing. Runtime
smoke examples can create and remove real containers; run them only against a
disposable environment you control.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build -p containerhub-bridge --locked
node tests/node-management.mjs
node tests/node-sdk-failures.mjs
```

For UI changes, keep the runnable prototype and native implementation aligned.
Check light/dark themes and 760×560, 1040×700 and 1440×900 windows. Use
`--validate-ui` for actual GPUI/Metal captures; prototype screenshots alone do
not verify the native application. Add tests for changed behavior, rather than
tests that repeat styling constants.

## Boundaries to preserve

- Runtime truth belongs in provider adapters; the UI and Node SDK share typed
  management contracts. Unsupported capability data must not look like zero.
- Running and healthy are different states. An uninspected Swarm container is
  not automatically healthy or running.
- After a timeout or restart, reconcile the original operation. Do not submit a
  replacement write with a new ID automatically.
- Credentials stay in Rust storage and cryptographic layers. Do not put secrets,
  local profiles, databases, diagnostic dumps or real host details in commits.
- Keep development credentials and FakeProvider features isolated from ordinary
  builds. Do not relax production enrollment to make a test pass.

Use Conventional Commits, for example `fix(docker): reject a replaced service`.
Describe the concrete change and the checks actually run. Distinguish fixtures,
local native rendering and real runtime or network testing.

Unless explicitly stated otherwise, intentionally submitted contributions are
licensed under Apache-2.0 as described in Section 5 of [LICENSE](LICENSE).
