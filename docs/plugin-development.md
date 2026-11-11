# Plugin Development

OpenKite plugins come in two shapes and both feed the same registration
surface, so a contribution looks the same to the shell wherever it came from:

- **Rust crates compiled in**, through `openkite-plugin-sdk` (the SDK below).
- **JS bundles loaded from disk** by the desktop host (`~/.openkite/plugins/`),
  described under [JS plugins](#js-plugins-v11).

The two hosts are not symmetric: the desktop host discovers and evaluates
bundles, the web host serves the registration surface but evaluates nothing.
The decision record is in [plugin-architecture.md](./plugin-architecture.md);
the message-level bridge reference is in [plugin-api.md](./plugin-api.md). This
page is the how-to.

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

## JS plugins (v1.1)

### Layout

```text
~/.openkite/plugins/argocd/
  manifest.json
  main.js
  style.css          # optional
```

`manifest.json` is validated by `crates/openkite-desktop/src/plugin_js.rs`:

| field | required | rules |
|---|---|---|
| `name` | yes | `[A-Za-z0-9_-]` only; the identity every registration is attributed to |
| `version` | yes | non-empty; informational |
| `entry` | yes | relative path ending `.js`, no `..`, must stay inside the plugin dir |
| `style` | no | relative path ending `.css`, same rules |
| `description` | no | free text |
| `author` | no | free text |
| `sidebar` | no | `[{ label, icon, route }]`; parsed into `SidebarEntry` rows, but nothing installs them — runtime `openkite.registerSidebar` is the path that works |

### What a bundle gets

The desktop host sets `window.__openkite_plugin` to your manifest `name` and
then evaluates your entry file in the webview, so every `openkite.*` call is
attributed to you and posted to `/openkite`
(`crates/openkite-desktop/src/router.rs`, `crates/openkite-ui/src/plugin_api.rs`).

The surface is the same string on both hosts — the desktop inlines it in
`bootstrap_head` (`crates/openkite-desktop/src/lib.rs`), the web document inlines
it in `crates/openkite-web/src/ssr.rs`:

| call | effect |
|---|---|
| `openkite.registerSidebar({ label, icon?, route })` | nav entry — the shell builds your section (`crates/openkite-ui/src/shell.rs`) |
| `openkite.registerRoute({ path, title? })` | declares a route you own; `path` starts with `/` |
| `openkite.registerStatusItem({ label, color? })` | status-bar widget |
| `openkite.registerStyles(css)` | the plugin's stylesheet (replaces the previous one) |
| `openkite.registerControl({ label, action, unsupported? })` | a button the shell paints for you |
| `openkite.onControl((action, plugin) => …)` | click handler for the buttons you declared |
| `openkite.api.list(kind, ns)` | pods, deployments, … in a namespace |
| `openkite.api.get(kind, ns, name)` | one object |
| `openkite.api.watch(kind, ns)` | snapshot for now; reflector-backed views are not wired |
| `openkite.api.logs(name, ns, container)` | pod logs, through the typed `Api<Pod>` |
| `openkite.api.exec(…)` | refused — `exec is not supported yet` |
| `openkite.subscribe(kindOrOpts, handler)` | returns an `unsubscribe()` function |

`openkite.registerRouteRenderer` is the one call that does not work yet: it
posts kind `route_renderer`, which `apply_register` rejects as an unknown kind,
and the bootstrap's `_renderRoute` is a no-op returning a no-op unmount. No host
mounts a plugin-drawn route body. See the gap list in
[plugin-architecture.md](./plugin-architecture.md).

Where the hosts differ:

| | desktop | web |
|---|---|---|
| runs your bundle | yes, in the wry webview | no — nothing scans `~/.openkite/plugins` |
| `window.openkite` | yes | yes, from the SSR document |
| kube reads | the process identity (kubeconfig context) and its RBAC | same mechanism: `Client::try_default()`, i.e. the pod's SA in-cluster |
| plugin route body | the route chrome with an empty body: the host advertises plugin routes (`Capabilities::in_process()`, `crates/openkite-api/src/capability.rs`) but no renderer is ever registered, so there is no mount node to fill | `data-unsupported="plugin-route"` panel: `Capabilities::server_side()` sets `plugins: false`, and `PluginRouteView` renders the refusal instead of an unfillable slot (`crates/openkite-ui/src/components/route_views.rs`) |
| reload after editing a file | restart the app — bundles are collected once at boot (`crates/openkite-desktop/src/lib.rs`); `watch_plugins` exists in `crates/openkite-desktop/src/plugin_js.rs` but nothing installs it | n/a |
| control clicks | `document::eval` of the crate's `control_dispatch_js`, which calls `openkite._control` → your `onControl` (`crates/openkite-desktop/src/router.rs`) | the identical script from the page's own context (`crates/openkite-web/src/app.rs`); no bundle installed a handler, so it resolves to nothing |

Practical consequence: a plugin that only registers entries, reads data and
declares controls works on both hosts. A plugin that wants to paint its own
markup inside a route has nowhere to run it yet.

### Declaring capability so both hosts agree

You never draw the button. You declare it, and the shell decides whether it is
live:

```js
openkite.registerControl({ label: "Port-forward", action: "forward" });
openkite.registerControl({
  label: "Port-forward",
  action: "forward",
  unsupported: "Needs the desktop bundle",   // reason shown on the disabled button
});
```

`crates/openkite-ui/src/components/route_views.rs` renders `unsupported: Some(_)`
as a disabled button with `data-unsupported="plugin"` and the reason in
`title`; a view with no control handler at all gets `data-unsupported="unwired"`
and `This host did not wire this control.` Same component, same attributes,
same reason text on both hosts — that is what "consistent `data-unsupported`"
means here.

Declare `unsupported` whenever the action needs your bundle running. A click
calls the page's `openkite._control(plugin, action)`, which forwards to your
`onControl` handler **if one is installed** and otherwise returns silently
(`crates/openkite-ui/src/plugin_api.rs`). The shell cannot tell the two apart:
it enables the button because it has a route owner to hand clicks to
(`crates/openkite-ui/src/components/route_views.rs`), and only your handler
decides whether anything happens. On desktop your bundle always installs one;
on web nobody does unless a page-side script does — so without `unsupported`
the button looks live on both hosts and answers on one.

### Install locally

Desktop:

```sh
mkdir -p ~/.openkite/plugins/hello
cp manifest.json main.js ~/.openkite/plugins/hello/
```

Discovery happens at boot — `collect_bundles` in
`crates/openkite-desktop/src/lib.rs` — and there is no install command. Editing
a bundle afterwards needs a restart: `watch_plugins` and `scan_and_reconcile`
live in `crates/openkite-desktop/src/plugin_js.rs` and
`scan_and_reconcile` is covered by `crates/openkite-desktop/tests/plugin_js.rs`,
but neither has a production caller. Enable or
disable in `~/.openkite/config.toml` (`crates/openkite-host/src/config.rs`):

```toml
enabled_plugins = []        # empty = all enabled; otherwise an allowlist
disabled_plugins = ["hello"] # wins over enabled_plugins
```

Web: nothing to install. No directory is read. To exercise the same calls, run
them in the page — devtools console or a `<script>` on your own page:

```js
openkite.registerSidebar({ label: "Hello", route: "/hello" });
const pods = await openkite.api.list("pods", "default");
```

Registration calls are fire-and-forget: they return nothing and report failures
with `console.warn("openkite register <kind> failed:", err)`
(`crates/openkite-ui/src/plugin_api.rs`). `openkite.api.*` returns a promise.
Registrations land in the server's `RegistrationStore` and appear on the next
render.

### Debugging a plugin that does not load

- **A disabled nav entry appears where your plugin should be.** Discovery failed
  — bad manifest, missing `entry`, or an unreadable file. The entry is named
  after your *directory* (the only identifier a broken manifest still has),
  rendered as an inert row with `data-unsupported="plugin-load"`, the reason in
  `title`, and the reason inlined as a `.nav-note` span
  (`crates/openkite-desktop/src/plugin_js.rs` builds the failure,
  `crates/openkite-ui/src/shell.rs` makes the row,
  `crates/openkite-ui/src/components/shell.rs` paints it).
- **Nothing appears at all.** The plugin is disabled in
  `~/.openkite/config.toml`, or the directory is not under
  `~/.openkite/plugins` (`plugins_dir()` in `crates/openkite-desktop/src/plugin_js.rs`).
- **Console shows `openkite register <kind> failed: …`.** The bridge rejected the
  payload — wrong field type, or a registration kind it does not accept
  (`crates/openkite-host/src/bridge.rs`).
- **Registration rejected with a path message.** `PluginRegistration::validate`
  (`crates/openkite-ui/src/plugin_api.rs`) requires every route and renderer
  path to start with `/` and contain no space, so
  `route path '/x y' must start with / and have no spaces` comes back as a
  failed `register route` and logs through
  `console.warn("openkite register route failed:", err)`. The rejection is
  per-registration: that payload is not stored, and the store keeps whatever it
  had for you.
- **Stylesheet ignored, entry disabled with a CSS reason.** `crates/openkite-ui/src/plugin_css.rs`
  refuses, rather than fake-scopes, `@import`, `@charset`, `@namespace`,
  `@font-face`, `@keyframes`, `@property`, any selector targeting `html`/`body`,
  a sheet containing `</style` or `<!--`, and anything over `MAX_STYLE_BYTES`
  (256 KiB).
- **Nav entry works, route is blank.** Expected today on both hosts, for
  different reasons: no host mounts a bundle into a route body. Desktop paints
  the route chrome with an empty body (the host advertises plugin routes but no
  renderer exists, so `PluginRouteView` has no mount node —
  `crates/openkite-ui/src/components/route_views.rs`); web paints
  `data-unsupported="plugin-route"` because `Capabilities::server_side()` sets
  `plugins: false` (`crates/openkite-api/src/capability.rs`). Check the chrome,
  not your JS.

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

Static (Rust) plugin:

1. Create a crate depending on `openkite-plugin-sdk`.
2. Implement `OpenKitePlugin` (metadata, entries, routes, optional hook).
3. Add an optional path dependency + feature flag to `openkite/Cargo.toml`.
4. Register it in `crates/openkite-desktop/src/plugin_host.rs`'s static loader.
5. Add a `tests/` integration test for any pure logic.
6. Document it (this file) and bump the SDK if the contract changed.

JS plugin (desktop host):

1. `manifest.json` with a `name` limited to `[A-Za-z0-9_-]`, a non-empty
   `version`, and an `entry` ending `.js` — paths relative and `..`-free.
2. Call `openkite.register*` at top level. You never pass a plugin id: every
   call is attributed to your manifest `name` (or `window.__openkite_plugin`).
3. Declare `unsupported` on any control that depends on your bundle running.
4. Do not rely on `registerRouteRenderer` or `openkite.api.exec` — both are
   refused today (see *Debugging* above).
5. Ship CSS through the manifest `style` (or `openkite.registerStyles`), not an
   injected `<style>` element: only sheets that reach the store get scoped, and
   get refused-with-reason when they cannot be.
6. Confirm the nav entry renders on both hosts; a plugin-owned route body will
   still be empty.
