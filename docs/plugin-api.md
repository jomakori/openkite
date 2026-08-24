# Plugin API (JS plugins, OKT-46)

External OpenKite plugins are **JS bundles** in `~/.openkite/plugins/<name>/`
(see `plugin-architecture.md` for the v2 decision). The host injects the
`window.openkite` global before evaluating a bundle; plugins register UI and
call the cluster through it. Everything is promise-based and rides wry's ipc
channel (`window.ipc.postMessage`).

## Manifest

```json
{
  "name": "argocd",
  "version": "0.1.0",
  "entry": "main.js",
  "description": "ArgoCD dashboard",
  "author": "you"
}
```

- `name`: `[a-zA-Z0-9-_]` — also the per-plugin key for registrations.
- `entry`: relative `.js` file inside the plugin dir (validated: no `..`, no
  absolute paths).

## Registration API

Called once at load; the host re-evaluates the bundle on hot reload, so
plugins should register idempotently (duplicates overwrite).

| Function | Payload | Effect |
|---|---|---|
| `openkite.registerSidebar(item)` | `{label, icon?, route?}` | Sidebar entry under a plugin section |
| `openkite.registerRoute(route)` | `{path, title?}` | Routed plugin view; `path` must start with `/` |
| `openkite.registerStatusItem(item)` | `{label, color?}` | Status-bar widget |

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

Inbound from JS: `{channel: "openkite", id, plugin, request: {op, …}}` — the
host stamps `plugin` from `window.__openkite_plugin` (set before each eval).
Outbound: `{channel: "openkite", id, result}` or `{channel: "openkite", id,
error}`. `op` values: `list`, `get`, `watch`, `logs`, `exec`.

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
