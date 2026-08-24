# OpenKite Architecture

OpenKite is a single-binary Rust desktop app: **Dioxus 0.7** (UI) + **kube-rs 4**
(cluster access). No JS build step, no Node toolchain. This document covers the
component layout, the data flow from cluster to pixels, and the key decisions.

## Component map

```
src/
├── lib.rs          crate root — module wiring + run() bootstrap
├── main.rs         thin shim: openkite::run()
├── cluster/        kubeconfig loading, context switching, kube::Client factory
├── state/          reflectors → reactive state (ResourceState<T>)
├── views/          route-level Dioxus components (workloads, …)
├── components/     reusable UI primitives (ResourceTable, StatusBadge, …)
├── router/         route table + plugin route/section registration
├── runtime/        shared GlobalSignal<Option<Client>> bridging run() → views
├── plugin_host/    static plugin registry, lifecycle, panic containment
├── config/         ~/.openkite/config.toml
├── workloads/      pure mapping: WorkloadKind, columns, row mappers, status
├── logs/           LogStream, LineBuffer, FollowState
├── secrets/        mask(), MaskedSecret, mask_all()
├── theme/          Theme contract, 5 built-ins, Zed import, persistence
├── fuzzy/          command-palette fuzzy matcher
├── metrics/        sparkline SVG generation
├── prometheus/     Prometheus service detection
└── crates/plugin-sdk/   the author-facing plugin contract
```

## Data flow

```
kubeconfig ──▶ ClusterState ──▶ kube::Client
                                    │
                     ┌──────────────┼────────────────┐
                     ▼              ▼                ▼
              reflector(Pod)   reflector(Depl)   reflector(…)   (state/resources.rs)
                     │              │                │
                     ▼              ▼                ▼
              ResourceState<T> = Store<T> + Signal<Vec<Arc<T>>, SyncStorage>
                     │              │                │
                     └──────────────┴────────────────┘
                                    ▼
                          Dioxus views render
                    ResourceTable (sort/filter/window)
```

- **Reflectors** (`kube::runtime::reflector`) watch a resource kind and keep a
  local `Store<T>`. A background task drives snapshots into a cross-thread
  `Signal` (`SyncStorage`), so the UI re-renders on every cluster event without
  holding a lock across the render.
- **`drive_reflector`** (in `state/resources.rs`) is the testable seam: it takes
  a `Store<T>`, a watch stream, and an `FnMut` snapshot callback, so the wiring
  is verified in `tests/resources.rs` with no live API server.
- **Views consume pure mappers** (`workloads::*_row`) rather than touching kube
  types directly, which keeps row/status logic unit-testable in `tests/`.

## Module conventions

- **Lib + bin split** — all logic lives in the `openkite` lib crate; `main.rs`
  is a 4-line shim. This is what makes `tests/` integration tests possible.
- **Tests separated from code** — integration tests in `tests/`, never inline
  `#[cfg(test)]` blocks in source.
- **Comments describe code context, never tickets** — ticket references belong
  in PR bodies and commit messages only.
- **Pure logic first** — cluster-independent logic (mapping, matching, buffering,
  serialization) is implemented and tested before the Dioxus view is wired.

## Plugin host

Static-first (see [plugin-architecture.md](./plugin-architecture.md) for the full
decision record). The host:

1. loads feature-gated plugin crates into a `PluginRegistry`,
2. wraps every plugin call in `catch_unwind` so a panicking plugin degrades to a
   toast, never a crash,
3. fans out lifecycle events (`on_cluster_connect`) and installs plugin routes +
   sidebar sections into the router.

Dylib loading is experimental and opt-in; WASM is the v2 candidate.

## Key decisions

| Decision | Rationale |
|---|---|
| Dioxus 0.7 + kube-rs 4 | single-language, single-binary desktop app |
| Reflectors → `Signal` (not shared `RwLock`) | reactive re-render, no lock across render |
| Static plugins first | ABI safety, one Dioxus runtime, zero double-runtime UB |
| Lib+bin split + `tests/` | idiomatic Rust, integration-testable |
| `latest` k8s-openapi (v1_36) | tracks current API; field optionality differs per version (e.g. `CronJob.spec` is non-`Option`) |
| CodeMirror 6 / xterm.js vendored as static assets | no npm build step in the dev loop |

## Known limitations

- **Adding/removing a plugin requires a rebuild** (static-first v1). Enable/disable
  is persisted but still compiled in.
- **Experimental dylib plugins** require the *exact* same `rustc` and
  `openkite-plugin-sdk` version as the core build; mismatches are warned, not
  auto-resolved. This path may be removed if WASM lands.
- **Web/mobile are not built** — the app is desktop-first (Dioxus desktop +
  WebKitGTK on Linux).
- **Local shell terminal** (`portable-pty`) and **pod exec** (`kube` `ws`
  feature) are in progress; the terminal is not yet wired into a view.
- **Metrics** require a cluster with `metrics-server` (T1) / Prometheus (T2);
  absence is detected and rendered as a clean empty state.
- **CI is the compiler of record** — local cargo builds are not run in the
  containerized dev loop; validation is push → GitHub Actions logs.
