# Plugin API (JS plugins, OKT-46)

External OpenKite plugins are **JS bundles** in `~/.openkite/plugins/<name>/`
(see `plugin-architecture.md` for the v2 decision). The host injects the
`window.openkite` global before evaluating a bundle; plugins register UI and
call the cluster through it. Everything is promise-based: each call is a
same-origin `fetch` POST to `/openkite`, served by the bridge runtime
(`src/bridge.rs`, OKT-46).

## Manifest

```json
{
  "name": "argocd",
  "version": "0.1.0",
  "entry": "main.js",
  "style": "style.css",
  "description": "ArgoCD dashboard",
  "author": "you"
}
```

- `name`: `[a-zA-Z0-9-_]` — also the per-plugin key for registrations.
- `entry`: relative `.js` file inside the plugin dir (validated: no `..`, no
  absolute paths).
- `style`: optional relative `.css` file inside the plugin dir (same `..`/
  absolute-path validation as `entry`). Rewritten at load time — see
  "CSS isolation" below.

## CSS isolation

A plugin's `style` is rewritten before it ever reaches the page: every
selector is scoped so it can only match inside that plugin's own subtree,
under `[data-plugin="<name>"]`. The host sets that attribute on the plugin's
root DOM node (the sidebar section, the status item, the routed mount node),
so a plugin only needs to style its own markup — it never needs to add the
attribute itself.

**What a plugin's CSS can reach:**

- Plain selectors and comma-separated selector lists are rewritten as
  descendants of the plugin root, e.g. `.panel { color: red }` becomes
  `[data-plugin="argocd"] .panel { color: red }`. Two plugins declaring the
  same class never collide — each is scoped under its own attribute.
- `:root` is mapped onto the plugin root itself, so custom properties
  (`--accent: …`) apply only within that plugin's subtree, not globally.
- `@media`, `@supports` and `@container` are kept as-is; the rules nested
  inside them are scoped the same way as top-level rules.

**What a plugin's CSS cannot reach, and is rejected outright (the plugin's
nav entry is disabled with the stated reason, never a silent no-op or a
thrown error in the shell):**

- `@font-face` and `@keyframes` (`-webkit-`/`-moz-` prefixed too) — both
  declare *names* that are global by the CSS spec; nesting the rule under an
  attribute selector does not scope the name itself, so two plugins could
  still collide on a font family or animation name. Rather than claim an
  isolation it cannot deliver, the rewriter refuses these at-rules. If a
  plugin needs a custom font or animation, give it a name namespaced to the
  plugin (e.g. `argocd-spin`) and accept that it is a global resource.
- `@import` — it reaches outside the plugin's own declared files, which the
  manifest's `entry`/`style` path validation already forbids elsewhere.
- Any selector whose first simple selector is `html` or `body` — no
  attribute scope can make `[data-plugin="x"] html` or `[data-plugin="x"]
  body` match anything, since neither element is ever a descendant of the
  plugin's own root.

A plugin whose `style` fails to load (missing file) or fails to scope (one
of the rejected shapes above) is disabled with that reason surfaced in the
host's plugin diagnostics; the rest of the shell, and every other plugin,
keeps working.

## Registration API

Called once at load; the host re-evaluates the bundle on hot reload, so
plugins should register idempotently (duplicates overwrite).

| Function | Payload | Effect |
|---|---|---|
| `openkite.registerSidebar(item)` | `{label, icon?, route?}` | Sidebar entry under a plugin section |
| `openkite.registerRoute(route)` | `{path, title?}` | Routed plugin view; `path` must start with `/` |
| `openkite.registerStatusItem(item)` | `{label, color?}` | Status-bar widget |
| `openkite.registerStyles(css)` | `"…css…"` | The plugin's own stylesheet (v1.1) |
| `openkite.registerControl(control)` | `{label, action, unsupported?}` | Control on the plugin's route head (v1.1) |
| `openkite.registerRenderer(renderer)` | `{path}` | Routes the plugin's JS paints itself (v1.0) |

### Styles (v1.1)

`registerStyles` takes CSS text, not a `<link>`. The host **scopes** it before
it reaches the page: every rule gets `[data-plugin="<name>"]` prepended to its
selectors, so a plugin's sheet cannot touch the console chrome, and two plugins
shipping `.card` do not collide. `@media`/`@supports`/`@container` blocks
recurse; comments are stripped. `@import`, `@charset`, `@namespace`,
`@font-face`, `@keyframes`, `@property` and any `html`/`body` selector are
rejected with a reason (see the list above), so a plugin never ships CSS the
host cannot actually isolate.

The same attribute is on the plugin's nav section and on the controls the host
paints for it, so `[data-plugin="argocd"] .app-card` and a bare `.app-card`
reach the same elements — both mean "inside this plugin's surface".

### Controls (v1.1)

A control is a button on the plugin's route head — the plugin declares it, the
host paints it, the plugin owns the action. Wire the plugin's side once:

```js
openkite.onControl((action) => {
  if (action === "sync") syncAll();
});
```

The host hands each click back as `action`. Declare `unsupported` when the
plugin cannot honour a control in this environment: the button still renders,
inert, with the reason as its tooltip — a surface that silently drops what it
cannot do is worse than one that says so.

## Cluster API

All calls return `Promise<result>`; rejections carry a human-readable error.

| Function | Args | Maps to |
|---|---|---|
| `openkite.api.list(kind, ns?)` | `"pods"`, `"default"` / `null` | `Api::list` |
| `openkite.api.get(kind, ns, name)` | `"deployments"`, `"default"`, `"web"` | `Api::get` |
| `openkite.api.watch(kind, ns?)` | | reflector/watch stream |
| `openkite.api.logs(name, ns, container?)` | | `Api::log_stream` |
| `openkite.api.exec(name, ns, container?, cmd)` | `["sh","-c","ls"]` | exec (PTY later) |

RBAC: calls execute with the app's kube client and the user's
`~/.kube/config` permissions — plugins get no raw cluster credentials.

## Envelope

Transport: same-origin `fetch("POST /openkite")` handled by the dioxus
asset-handler registry (see `plugin-architecture.md`; the wry ipc channel
cannot carry custom methods in dioxus 0.7.x). POST body:
`{id, plugin, request: {op, …}}` — the bridge reads `plugin` from
`window.__openkite_plugin` (set by the host before each eval). Response:
`{status: "ok", result}` or `{status: "error", error}`. `op` values:
`register`, `list`, `get`, `watch`, `logs`, `exec` (`exec` is deferred —
calls reject with a clear error until it lands).

## Example

```js
openkite.registerSidebar({ label: "Applications", icon: "grid", route: "/argocd/apps" });
openkite.registerRoute({ path: "/argocd/apps", title: "ArgoCD Applications" });

async function syncStatus(name) {
  const app = await openkite.api.get("applications", "argocd", name);
  openkite.registerStatusItem({ label: `ArgoCD: ${app.status.sync.status}`, color: app.status.sync.status === "Synced" ? "green" : "red" });
}
```

## Trust model

Plugins run in the same JS context as the app (Headlamp-style). Installing a
plugin means trusting it with read/write access to what the kube client can
do. The Settings plugin manager (OKT-42) shows a trust warning + enable
allowlist.
