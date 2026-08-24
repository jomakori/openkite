# OpenKite Plugin Architecture — Decision

> **Status:** accepted (spike OKT-2) · **Date:** 2026-08-21
> **Decision:** **STATIC-first** for v1. Dylib loading is **experimental only**.
> WASM (Zed-style) is the **v2 candidate** if third-party plugin demand grows.

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

## Risks & limitations

- Adding/removing plugins requires a rebuild (v1).
- Experimental dylib path is fragile by nature and may be dropped.
- WASM v2 requires an SDK capability redesign; budget accordingly if adopted.

## v2 — JS plugins in the webview (decision 2026-08-24, OKT-28)

Research (Lens/Freelens + Headlamp) → decision: **external plugins are JS
bundles evaluated in the wry webview** (Headlamp model, adapted). The Rust
SDK + static registry remain for native plugins; the dylib path stays
experimental. See `src/plugin_js.rs` (OKT-45) for the loader foundation.

- Plugin = `~/.openkite/plugins/<name>/` containing `manifest.json`
  (name/version/entry/author/sidebar) + one entry `.js`.
- Load: host scans + validates manifests, evaluates bundles in the webview;
  plugins register UI via the eval bridge (`openkite.register*` + `openkite.api.*`,
  OKT-46) proxied to kube-rs (RBAC-scoped).
- **Hot reload**: the host watches the plugins dir (`notify`), rescans, and
  diffs (`scan_and_reconcile`) — Added/Removed/Changed plugins are re-evaluated
  and re-rendered **without a restart** (exceeds Headlamp/Freelens, which cap
  production reload at app restart).
- Trust: plugins run in the app's JS context — same trust model as Headlamp;
  Settings (OKT-42) will offer enable/disable + allowlist.
- Trade-off accepted: plugin code is JS. Revisit WASM if third-party demand
  justifies an SDK capability redesign.
