# OpenKite Architecture

> Code paths in this document are relative to the desktop crate,
> `crates/openkite-desktop/` (e.g. `src/lib.rs` means `crates/openkite-desktop/src/lib.rs`).
>
> The shareable halves have since extracted: the console (`components/`,
> `design/`, `secrets/`, `shell.rs`, `theme*`) is `crates/openkite-ui`, and the
> kube-side runtime (`bridge/`, `state/`, `push/`, `config/`) is
> `crates/openkite-host`. The map below is the desktop crate's; completing the
> document for the three-crate split is its own change.

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

## Native RSX surfaces (OKT-127)

The browser console renders from `crates/openkite-ui` and is served by
`crates/openkite-web` (SSR + wasm hydration in the preview image — see
`crates/openkite-web/src/routes.rs::ssr_root`). The native
RSX surfaces in `crates/openkite-desktop/src/views/` and the plugin
wildcard route are kept on purpose, because the crate does not yet
expose an equivalent:

| Surface | Path | Reason native |
| --- | --- | --- |
| Logs viewer | `src/views/logs.rs` (`Route::Logs`) | `openkite-ui` has no log viewer; the console's `App` only paints capabilities. Retirement would delete a working UI. |
| Terminal exec | `src/views/terminal.rs` (`Route::Terminal`) | Pod/container picker + xterm.js host + reconnect state machine on top of the vendored bundle. No crate replacement exists; exec transport is deferred to Phase 1. |
| Pod detail inspector | `src/views/pod_detail.rs` (mounted in `AppShell`) | 5-tab slide-over driven by `SELECTED_POD`; uses `kube::Api::log_stream` and the container-state mapper that no crate view duplicates. |
| Plugin wildcard | `Route::Plugin` → `Plugin` dispatcher | Rust SDK routes (`ROUTE_TABLE`) + JS plugin renderers (`JsRouteSlot`); plugin routing is a host concern, not a console concern. |

The keep-native declaration lives at
`crates/openkite-desktop/src/router.rs` (module doc + `console_route`)
and at the top of each view module, so the rationale is visible where
a reader is most likely to look when deciding to delete a route.
- **Webview RAM floor** — the WebKitGTK webview carries a ~200 MB baseline per
  window; this is a Dioxus-desktop cost, not app state.
- **Eval-bridge throughput** — the Dioxus↔JS eval bridge used for the terminal
  (xterm.js) is chatty; high-volume terminal output must be chunked (~8 KB) and
  coalesced (~16 ms) to stay smooth.
- **Reflector memory** — every watched resource kind holds its full object state
  in memory; very large clusters (tens of thousands of objects) scale RAM
  proportionally.
- **CI is the compiler of record** — local cargo builds are not run in the
  containerized dev loop; validation is push → GitHub Actions logs.
