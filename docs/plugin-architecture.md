# OpenKite Plugin Architecture — Decision

> **Status:** accepted (spike OKT-2) · **Date:** 2026-08-21
> **Decision:** **STATIC-first** for v1. Dylib loading is **experimental only**.
> WASM (Zed-style) is the **v2 candidate** if third-party plugin demand grows.

## Current shape — two hosts, shared crates

OpenKite has two hosts. Both own the same plugin contract; only one of them
runs plugin code.

```text
            crates/openkite-plugin-host      ← types only, no host deps
            SidebarItem · RouteSpec · StatusItem · RendererSpec · PluginControl
            re-exported by crates/openkite-ui/src/plugin_api.rs
                              │  (call sites never changed)
        ┌─────────────────────┼──────────────────────────┐
        ▼                     ▼                          ▼
  desktop host          shared UI                  web host
  openkite-desktop      openkite-ui                openkite-web
  wry webview           route_views.rs  ← route     SSR shell + hydrate,
  + /openkite channel     resolution, controls      browser API at POST /openkite
  discovers + evals JS  plugin_css.rs   ← scoper    no discovery, no JS eval
                        shell.rs        ← nav/status
        └─────────────────────┼──────────────────────────┘
                              ▼
                 crates/openkite-host/src/bridge.rs
                 Bridge::handle_post + one RegistrationStore
```

| Responsibility | Owner |
|---|---|
| Plugin data types (`SidebarItem`, `RouteSpec`, `StatusItem`, `RendererSpec`, `PluginControl`) | `crates/openkite-plugin-host/src/lib.rs`, re-exported from `crates/openkite-ui/src/plugin_api.rs` |
| Discovery, manifest validation, bundle collection, JS evaluation | `crates/openkite-desktop/src/plugin_js.rs` (desktop only) |
| Hot-reload watcher | `crates/openkite-desktop/src/plugin_js.rs` — defined and unit-tested, **not installed** by the host |
| Bridge: request envelope, `register` merge, kube ops, subscriptions | `crates/openkite-host/src/bridge.rs` |
| Route ownership/resolution, plugin route view, control button | `crates/openkite-ui/src/components/route_views.rs` |
| CSS scoper and its refusal vocabulary | `crates/openkite-ui/src/plugin_css.rs` |
| Host shells | desktop: `crates/openkite-desktop/src/router.rs`; web: `crates/openkite-web/src/app.rs`, `crates/openkite-web/src/routes.rs` |

What each host provides:

- **Desktop** — a wry webview with the desktop bridge channel. The page's
  `window.openkite` *is* the bridge client: same-origin `fetch` POSTs to
  `/openkite`, answered by the asset handler registered in
  `crates/openkite-desktop/src/router.rs` and dispatched into
  `crates/openkite-host/src/bridge.rs`. (Called IPC here for short; the wire is
  `fetch`, not `window.ipc.postMessage` — see *Transport* below for why.)
- **Web** — a server-rendered shell plus an HTTP API: `tower`/`axum` routes in
  `crates/openkite-web/src/routes.rs` render the same `openkite-ui` components
  and expose `POST /openkite` as the bridge endpoint, with the same
  `OPENKITE_BRIDGE_JS` bootstrap inlined into the document
  (`crates/openkite-web/src/ssr.rs`).

Two facts worth stating plainly, because the old single-host text blurred them:

- **Plugin JS runs in the desktop webview only.** `collect_bundles` +
  `window.__openkite_plugin` + `eval` live in
  `crates/openkite-desktop/src/plugin_js.rs` and
  `crates/openkite-desktop/src/router.rs`; the web crate never calls them, and
  its bridge starts with an empty store.
- **Bundles are discovered on disk** at `~/.openkite/plugins/<name>/`
  (`plugins_dir()` in `crates/openkite-desktop/src/plugin_js.rs`), gated by
  `enabled_plugins` / `disabled_plugins` in `~/.openkite/config.toml`
  (`crates/openkite-host/src/config.rs`). The web host reads no plugin
  directory — a web plugin contributes through the page's own
  `openkite.register*` calls, which the browser can make because the bootstrap
  is served with the document.

Trust model: **Headlamp-like** — plugin JS shares the host JS context. There is
no sandbox, no per-plugin origin, and no capability token: a plugin can touch
anything the page can. Attribution is a page global, not a credential — the
host sets `window.__openkite_plugin` around each bundle and the bootstrap posts
it as the plugin name (`crates/openkite-ui/src/plugin_api.rs`), so a page that
can run script can claim any name. What limits a plugin is the cluster side,
where every kube call runs through `crates/openkite-host/src/bridge.rs` with the
process identity —
the kubeconfig context on desktop, the pod's service account when the web host
runs in-cluster (`in_cluster()` in `crates/openkite-web/src/host.rs`) — and that
identity's RBAC. UI-level containment is *scoping*, not security: CSS is
rewritten per plugin (`crates/openkite-ui/src/plugin_css.rs`) so one plugin's
rules only match inside its own subtree.

## Context

The original spec loaded plugins as Rust dylibs (`.so/.dylib/.dll`) scanned
from `~/.openkite/plugins/`, entered through
`extern "C" fn openkite_plugin_create() -> *mut dyn OpenKitePlugin`.

That design is unsound as written:

- `*mut dyn OpenKitePlugin` crosses `dlopen` — a Rust trait object whose vtable
  is only valid for the exact `rustc` version and exact `openkite-plugin-sdk`
  build that produced it. Any drift is UB.
- Each plugin dylib statically links its **own copy** of the SDK *and* Dioxus.
  N plugin copies ⇒ N Dioxus runtimes/global registries in one process.
- `Element` (VNode) is `!Send` and was created against a *different* Dioxus
  copy than the one rendering it. The SDK trait demands `Send + Sync`.
- UI handed across the boundary (`fn() -> Element`) cannot be rendered safely.

## Options evaluated

| Option | ABI safe | Crash isolation | Packaging | UI across boundary | kube::Client sharing | Build complexity |
|---|---|---|---|---|---|---|
| **Static crates** (feature-gated workspace/path deps) | ✅ trivially | ⚠️ panic containment needed | ✅ single binary | ✅ one runtime, identical types | ✅ direct | ✅ none |
| Dylib + Rust trait objects (as spec'd) | ❌ none | ⚠️ | ✅ | ❌ double runtime | ⚠️ | ❌ toolchain lockstep |
| `abi_stable` dylibs | ⚠️ yes, via repr(C) mirrors | ⚠️ | ✅ | ⚠️ still double runtime | ⚠️ | ⚠️ heavy SDK ceremony |
| **WASM/WASI** (Zed model, Extism) | ✅ host-managed | ✅ sandbox | ⚠️ runtime dep (wasmtime) | ⚠️ host-rendered capability API | ⚠️ via capability API | ⚠️ SDK redesign |
| Process isolation (VS Code/Freelens extension host, JSON-RPC) | ✅ | ✅ | ❌ contradicts single-binary | ❌ IPC surface | ⚠️ proxy | ❌ heaviest |

## Decision

### v1 — static plugins (chosen)

Plugins are Rust crates compiled into the binary, enabled with feature flags:

```toml
# Cargo.toml (core)
[features]
default = []
plugin-argocd = ["dep:openkite-plugin-argocd"]

[dependencies]
openkite-plugin-argocd = { path = "../plugin-argocd", optional = true }
```

- One Dioxus runtime, identical types, zero ABI surface.
- Plugin host wraps every plugin call in a panic boundary
  (`catch_unwind`) so a panicking plugin degrades to a toast, never a crash.
- Enable/disable persisted in `~/.openkite/config.toml` (still loaded in the
  binary, just not activated) — the SDK trait contract is unchanged.
- Third-party plugins are consumed as path/git dependencies, documented in
  `plugin-development.md`. Rebuild required to add/remove — accepted trade-off
  for v1 (single maintainer, OSS).

### Experimental — dylib loader (behind a feature flag, opt-in)

Kept for evaluation only. Requirements when enabled:

- Plugin must be built with the **same rustc** and the **exact**
  `openkite-plugin-sdk` version as the core build.
- Compatibility matrix documented in the plugin README and `docs/`.
- Warn dialog on mismatch; failures degrade to toasts.
- May be removed if WASM lands.

### v2 candidate — WASM (Zed-style)

Zed migrated its native extension system to WASM for exactly the reasons in
the Context section. Path: **Extism** (host SDK + PDK) or raw wasmtime with a
versioned capability API. The SDK trait stays as the author-facing contract;
a WASM adapter implements it behind the host. Plugin UI becomes data/commands
rendered by the host, not components crossing the boundary. **Deferred** — not
needed until third-party plugin demand exists.

## Impact on tickets

- **Plugin SDK** — contract unchanged: `OpenKitePlugin` trait,
  `PluginContext { kube_client, discovery, theme, ui, runtime }` (+
  `tokio::runtime::Handle`). Keep the SDK minimal and stable; semver policy
  documented. `openkite_plugin_create` marked experimental.
- **Plugin Host** — static registry (feature-gated modules) is the primary
  path; lifecycle fan-out; panic containment; experimental `libloading`
  loader behind a flag.

Naming note: this section predates `crates/openkite-plugin-host`, which today
holds only the plugin data types. The static registry it describes is the
`plugin_host` *module* in `crates/openkite-desktop/src/plugin_host.rs`
(`PluginRegistry`), whose own header records that its lifecycle API has no live
consumer yet — JS plugins, not static ones, are what the hosts actually load
today.

## Risks & limitations

- Adding a *static* plugin requires a rebuild (v1); a JS plugin drops into
  `~/.openkite/plugins/` and is picked up without one (desktop only).
- Experimental dylib path is fragile by nature and may be dropped.
- WASM v2 requires an SDK capability redesign; budget accordingly if adopted.

## v2 — JS plugins in the webview (decision 2026-08-24, OKT-28)

Research (Lens/Freelens + Headlamp) → decision: **external plugins are JS
bundles evaluated in the wry webview** (Headlamp model, adapted). The Rust
SDK + static registry remain for native plugins; the dylib path stays
experimental. See `crates/openkite-desktop/src/plugin_js.rs` (OKT-45) for the
loader foundation. The loader is desktop-only: the web host serves the same
registration surface but evaluates nothing.

- Plugin = `~/.openkite/plugins/<name>/` containing `manifest.json`
  (name/version/entry/style/description/author/sidebar) + one entry `.js`.
- Load (desktop): host scans + validates manifests, evaluates bundles in the
  webview; plugins register UI via the eval bridge (`openkite.register*` +
  `openkite.api.*`, OKT-46) proxied to kube-rs (RBAC-scoped).
- Load (web): no scan, no evaluation. The bootstrap (`window.openkite`) ships
  with the SSR document and posts to `POST /openkite`, so a page-side script
  can register entries and read kube data with the same calls; nothing mounts
  a bundle for it.
- **Reload** (desktop): not wired. `watch_plugins` and `scan_and_reconcile`
  live in `crates/openkite-desktop/src/plugin_js.rs`; `scan_and_reconcile` is
  exercised by `crates/openkite-desktop/tests/plugin_js.rs`, but neither has a
  production caller (`watch_plugins` has no caller anywhere in the workspace),
  and `crates/openkite-desktop/src/lib.rs` collects bundles once at boot. So
  editing a bundle needs a restart. The design intent — re-evaluate on change,
  no restart, which is where this would exceed Headlamp/Freelens — is not
  shipped.
- Trust: plugins run in the app's JS context — same trust model as Headlamp.
  `enabled_plugins` / `disabled_plugins` in `~/.openkite/config.toml` gate what
  the desktop host evaluates (`crates/openkite-host/src/config.rs`); an empty
  `enabled_plugins` means "all enabled".
- Trade-off accepted: plugin code is JS. Revisit WASM if third-party demand
  justifies an SDK capability redesign.

## Transport — bridge runtime (OKT-46, decision 2026-08-29)

The OKT-46 spike wired the JS bridge over wry's `window.ipc.postMessage`.
That channel is **dead for custom messages in dioxus-desktop 0.7.10**:

- inbound ipc must parse as `{method, params}` (`IpcMessage`), so the
  bridge's `{channel, id, plugin, request}` envelope is dropped as
  unparseable before dispatch even fires;
- well-formed but unknown methods hit `IpcMethod::Other(_) => {}` —
  silently discarded with no host hook (`launch.rs`).

Replacement (locked): **same-origin `fetch` POST to `/openkite`**,
dispatched by the dioxus-desktop asset-handler registry
(`DesktopContext::register_asset_handler` / `use_asset_handler`; the first
URI path segment names the handler, dispatch precedes filesystem fallback,
POST bodies arrive intact). This is the sanctioned desktop custom-protocol
transport — the same seam Tauri v2's `invoke` uses. The handler runs on the
GLib main thread (`Callback` is `!Send`); kube work is spawned onto the
tokio runtime and the answer flows back through wry's `RequestAsyncResponder`
(`Send`), so long kube calls never block the UI thread.

Runtime shape (`crates/openkite-host/src/bridge.rs`):

- `Bridge::handle_post` maps `{id, plugin, request}` to a response; errors
  are always answered, never dropped.
- One `RegistrationStore`, shared by both hosts. `register` ops merge into it
  (appends per kind); nothing clears a plugin's entries today, because the
  reload path that would do it is not installed — see *v2* above.
- kube ops (`list`/`get`/`watch`/`logs`) run with the process identity and its
  RBAC — the kubeconfig context on desktop, the pod's service account in the
  web host. Bare kind strings ("pods") resolve via full `Discovery`
  (group-preferred version; core group first). `watch` returns a snapshot
  until reflector-backed plugin views land. `logs` go through the typed
  `Api<Pod>` (the `Log` marker is pod-only). `exec` is deferred (PTY later) and
  answers `exec is not supported yet`.
- Mounting is done on both hosts. Desktop registers the `/openkite` asset
  handler and inlines the bootstrap in the page head (`bootstrap_head`), so
  `window.openkite` exists before any bundle evaluates
  (`crates/openkite-desktop/src/router.rs`, `crates/openkite-desktop/src/lib.rs`);
  web routes `POST /openkite` to `bridge_post` and inlines the same bootstrap
  (`crates/openkite-web/src/routes.rs`, `crates/openkite-web/src/ssr.rs`).

## CSS isolation — scoping, not sandboxing (decision 2026-11-09)

Plugins share the app's JS context (Headlamp model); their CSS shared the
same global stylesheet scope too, so a plugin's `<style>` rules could leak
into the host or into another plugin, and host classes could leak back into
plugin markup. `docs/plugin-api.md` said nothing about styling.

Decision: a small, dependency-free rewriter — now
`crates/openkite-ui/src/plugin_css.rs` (`scope_css` + `scope_root`, pulled out of
`plugin_js.rs` so both hosts share it) — rewrites a plugin's declared `style`
at render time, prefixing every selector with `[data-plugin="<name>"]` so it
can only match inside that plugin's own subtree. No CSS parser crate added — the
workspace has none and the shapes plugins actually emit (selector lists,
`:root`, `@media`/`@supports`/`@container`) are small enough for a
hand-rolled, depth-aware splitter.

Rejected rather than fake-scoped: `@font-face` and `@keyframes` (names are
global by spec — nesting under an attribute selector does not scope the
*name*, so two plugins could still collide on a font or animation), `@import`
(reaches outside the plugin's own files), and any selector targeting
`html`/`body` (no attribute scope can reach the document root). These
return `Err(reason)` from `scope_css`, never a panic. Both hosts turn a
refusal into the same disabled-with-reason treatment: the desktop folds it in
at bundle collection (`crates/openkite-desktop/src/plugin_js.rs`), the web host
folds it in when it reads registrations (`crates/openkite-web/src/routes.rs`),
and the shell paints one named, inert nav entry per failure
(`crates/openkite-ui/src/shell.rs` builds the rows,
`crates/openkite-ui/src/components/shell.rs` renders them with
`data-unsupported="plugin-load"`). One plugin's unscopable CSS never blocks
its siblings or crashes the shell.

## What is still missing

- **G3 has no plugin route.** The parity check
  (`.github/workflows/build-test-staging.yml` → `e2e/parity/run.sh`) grades
  `/`, `/cluster`, `/workloads` and `/config` on desktop and web; it does not
  register a plugin, so it never grades a *plugin-owned* route on both hosts.
  Two hosts that resolve plugin routes through the same
  `crates/openkite-ui/src/components/route_views.rs` is an argument, not
  evidence — the check is the open gap.
- **No host mounts a plugin JS body into a route.** `registerRouteRenderer`
  posts kind `route_renderer` (`crates/openkite-ui/src/plugin_api.rs`), and
  `apply_register` in `crates/openkite-host/src/bridge.rs` accepts only
  `sidebar`, `route`, `status`, `style` and `control`, so the call answers
  `unknown registration kind 'route_renderer'`. `RendererSpec` and the
  `all_renderer_paths()` slot exist in the model and the desktop router reads
  them (`crates/openkite-desktop/src/router.rs`), but nothing registers one, so
  a plugin-owned path paints the chrome's empty body on desktop and the
  explicit unsupported panel on web.
- **`openkite.api.exec` is unrouted.** `crates/openkite-host/src/bridge.rs`
  answers `exec is not supported yet` on both hosts.
- **Hot reload is a library, not a behaviour.** `watch_plugins` has no caller
  anywhere in the workspace — not even a test — and `scan_and_reconcile` is
  called only from `crates/openkite-desktop/tests/plugin_js.rs`; the desktop
  collects bundles once at boot (`crates/openkite-desktop/src/lib.rs`). Treat
  "re-evaluated without a restart" as unshipped until something installs the
  watcher.
