# Contributing

OpenKite is being rebuilt from the v2 spec. This file covers the workspace the
rebuild starts from.

## Workspace map

| Crate | Owns |
| --- | --- |
| `crates/openkite-engine` | Collection: interest registry, informer supervision, row projections, coalescing, delta encoding, cluster transport. |
| `crates/openkite-api` | The wire contract: delta frames, row and command types shared by the engine and the shells. |
| `crates/openkite-ui` | The console: Dioxus 0.7 surfaces, rendered by either shell. |
| `crates/openkite-desktop` | The native shell: webview window, engine over an in-process channel. |
| `crates/openkite-web` | The browser shell: axum server, server-rendered console, WebSocket deltas. |
| `crates/openkite-cli` | The binary entry point for scripted and headless use. |

Data flows one way: engine → api → ui → shell.

## Crate boundary rule

**`openkite-engine` must never depend on `dioxus` or `openkite-ui`; CI enforces
it.** The engine runs headless in the CLI and in tests on a machine with no
webview stack, and it has to survive a UI framework change. The
`Guard the crate boundary` job fails when `cargo tree -p openkite-engine` lists
either.

## CI gates

All four must pass, every pull request:

1. `Check formatting` — `cargo fmt --all -- --check`
2. `Lint with clippy` — `cargo clippy --workspace --all-targets -- -D warnings`
3. `Run tests` — `cargo test --workspace`
4. `Guard the crate boundary` — no `dioxus` or `openkite-ui` under the engine

## Local commands

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo tree -p openkite-engine
```

`pre-commit install` wires the same format, check and clippy passes into the
commit.

## Conventions

- Commits: Conventional Commits, imperative, subject ≤72 characters.
- PRs: use `.github/pull_request_template.md`.
- Secrets: environment variables or Doppler only — never commit credentials.
