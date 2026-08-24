# Plugin Development

OpenKite plugins are Rust crates that extend the UI with routes, sidebar
sections, and cluster-aware behavior. The full decision record is in
[plugin-architecture.md](./plugin-architecture.md); this is the how-to.

## The SDK

Plugins depend on `openkite-plugin-sdk` (`crates/plugin-sdk`) and implement the
`OpenKitePlugin` trait:

```rust
pub trait OpenKitePlugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;              // name, version, description
    fn sidebar_entries(&self) -> Vec<SidebarEntry>;    // nav sections
    fn routes(&self) -> Vec<PluginRoute>;              // (path, render) pairs
    fn on_cluster_connect(&self, ctx: &PluginContext); // optional lifecycle hook
}
```

`PluginContext` gives a plugin access to the shared `kube::Client`, the theme,
the discovery surface, and a `tokio::runtime::Handle` for spawning work.

## Static registration (v1)

Plugins are compiled into the binary, enabled with feature flags:

```toml
# openkite/Cargo.toml
[features]
plugin-argocd = ["dep:openkite-plugin-argocd"]

[dependencies]
openkite-plugin-argocd = { path = "../plugin-argocd", optional = true }
```

The host loads every enabled plugin into the `PluginRegistry`; a plugin's
`sidebar_entries()` and `routes()` are installed into the router at boot, and
`on_cluster_connect` fires when a cluster connects.

**Adding a plugin = a rebuild.** Enable/disable is persisted in
`~/.openkite/config.toml`, but the crate is always compiled in.

## Experimental dylib loading

Opt-in and fragile. When enabled:

- The plugin dylib must be built with the **same `rustc`** and the **exact**
  `openkite-plugin-sdk` version as the core build.
- Version mismatches surface a warning; failures degrade to toasts.
- This path may be removed when WASM lands.

## Compatibility rules

- **Same rustc, exact SDK version** — a `dyn OpenKitePlugin` vtable is only
  valid across `dlopen` when both sides were built from identical toolchains.
- **SDK semver** — bump the SDK version whenever its public API changes; never
  change the public API without a bump.

## Checklist

1. Create a crate depending on `openkite-plugin-sdk`.
2. Implement `OpenKitePlugin` (metadata, entries, routes, optional hook).
3. Add an optional path dependency + feature flag to `openkite/Cargo.toml`.
4. Register it in `plugin_host`'s static loader.
5. Add a `tests/` integration test for any pure logic.
6. Document it (this file) and bump the SDK if the contract changed.
