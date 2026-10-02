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
- **Terminal exec transport is not wired yet** (`portable-pty` for a local
  shell, `kube`'s `ws` feature for pod exec): the terminal surface renders the
  xterm.js host, captures typed input, and reports the bridge-pending state;
  the host-side exec channel is Phase 1.
- **Metrics** require a cluster with `metrics-server` (T1) / Prometheus (T2);
  absence is detected and rendered as a clean empty state.

## Shared-UI surfaces (OKT-136)

Every surface renders from `crates/openkite-ui`: the desktop mounts the crate
components in its wry webview, and the browser host serves the same crate
through `crates/openkite-web` (SSR + wasm hydration in the preview image — see
`crates/openkite-web/src/routes.rs::ssr_root`).

| Surface | Crate component | Host-side plumbing |
| --- | --- | --- |
| Logs viewer (`Route::Logs`) | `components::logs::LogsView` | the desktop host streams the pod's kube log into `runtime::LOGS_BUFFER` |
| Terminal (`Route::Terminal`) | `components::terminal::TerminalView` | the vendored xterm.js bundle in `crates/openkite-ui/assets/vendored/xterm/`; exec transport deferred to Phase 1 |
| Pod detail inspector (mounted in `AppShell`) | `components::pod_detail::PodDetail` | driven by `runtime::SELECTED_POD` |
| Plugin wildcard (`Route::Plugin`) | `components::route_views::PluginRouteView` | The route chrome, the JS mount node and the declaration of the plugin-route capability render from `openkite_ui` (OKT-156); what stays native is the `document::eval` that mounts a JS bundle into the webview, plus the host's own plugin tables (an SDK route's view is the plugin's chrome and mounts as-is). |

A surface renders only when the host reports it through the capability
descriptor (`openkite_api::capability::Capabilities`). `TerminalView` renders
`TerminalUnsupported` when `runtime::terminal_can_render()` is false — the
verdict the browser host reports (`Capabilities::server_side`, `terminal:
false`); the desktop publishes `Capabilities::in_process()` at boot
(`crates/openkite-desktop/src/lib.rs::run`) and renders all four. The
desktop-side declaration of the wildcard route lives in the module doc at
`crates/openkite-desktop/src/router.rs`.

`crates/openkite-desktop/src/views/` no longer exists — the terminal view was
its last module.
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
