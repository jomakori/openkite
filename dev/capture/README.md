# Visual capture

Reproducible screenshots and GIFs of the running app, used by the README and by
PRs that change what a user sees. The harness boots the **real desktop binary**
under Xvfb + openbox, drives it with `xdotool`, and writes the media to a shared
volume.

This directory is the single source of truth for the committed images in
[`docs/media/`](../../docs/media). No mockups, no fixtures: every image is the
real wry/WebKitGTK webview on a live cluster.

## Why in-cluster

A Dioxus link step needs more than ~2.5 GiB and OOM-kills a small host
container. The capture therefore runs as a one-shot Kubernetes Job (6 GiB
limit) and **never** builds the `openkite` binary on the review machine:

> **Never run `cargo build` / `cargo test` / `cargo clippy` for the `openkite`
> binary on the review host.** `cargo fmt` and the plugin-SDK checks are fine.
> The Hermes agent pod is capped at ~3 GiB; an OOM there restarts the gateway.

## Files

| File | Role |
|---|---|
| `capture.sh` | Wrapper: seed ConfigMap → apply Job → wait → log → `kubectl cp` media into `docs/media/`. |
| `capture-job.yaml.tmpl` | Job manifest template. Every `@@PLACEHOLDER@@` is filled by `capture.sh`. |
| `capture-script.sh` | Job payload: build, boot routes, drive the UI, build GIFs, print frame-diff evidence. |

## Prerequisites

All identifiers are parameters — the defaults target the throwaway `ok-debug`
namespace on the dev k3d cluster, not a production cluster. Create:

1. **A throwaway Job namespace** (default `ok-debug`). It must **not** be
   GitOps-managed: ArgoCD self-heal reverts manual deploys, so a Job applied to
   a managed namespace is deleted out from under the run (and Tilt must never
   target one either). `ok-debug` is intentionally not targeted by any
   Application.

2. **A kubeconfig Secret** the app can use (default `openkite-kubeconfig`).
   Create it **out of band and never commit it**. `capture.sh` can seed it from
   a local file when you are bootstrapping:

   ```sh
   export OK_CAPTURE_KUBECONFIG=/path/to/kubeconfig   # or pre-create the Secret
   ```

3. **A scratch namespace** for the short-lived sample workloads that animate
   the resource table (`OK_CAPTURE_SCRATCH_NS`, default = the Job namespace —
   `ok-debug`). The Job creates and then deletes the sample pods there.

4. **A PersistentVolumeClaim** for the cargo cache and the media
   (`OK_CAPTURE_PVC`, default `openkite-cargo-cache`). Mounting it at `/data`
   reuses an existing `cargo/` + `target/` tree, so repeat runs build in ~1 min
   instead of a cold ~15 min. Size it ~20 GiB.

5. RBAC that lets the Job (a) read the kubeconfig Secret, (b) create/delete pods
   in the scratch namespace, and (c) read pods/events so the app renders live
   data.

## Run

```sh
export KUBECONFIG=/path/to/admin-kubeconfig
export OK_CAPTURE_SCRATCH_NS=ok-debug

./dev/capture/capture.sh
```

The wrapper prints the Job log path (`.capture.log`) and copies the media into
`docs/media/`. Re-runs delete and recreate the Job, so the same command is
idempotent. Run `./dev/capture/capture.sh --help` for every `OK_CAPTURE_*` knob.

### Stills only

Turn the interactive flows off and capture just the routes:

```sh
OK_CAPTURE_FLOWS=0 ./dev/capture/capture.sh
```

### Several one-off routes in a single run

Pass `route:slug` pairs; each boots a fresh app instance and writes `<slug>.png`:

```sh
OK_CAPTURE_EXTRA_ROUTES='/cluster:cluster /terminal:terminal' \
  ./dev/capture/capture.sh      # -> docs/media/cluster.png, docs/media/terminal.png
```

### Routes after OKT-98

The React console now serves the core browse routes; `/logs` and `/terminal`
are deliberately still the native shell. The defaults capture the console on
`/` and the native shell on `/logs`. Both `/` and `/workloads` open the console
on **Pods** (`src/router.rs: console_route`).

## Gotchas (each cost real time)

### 1. Throwaway namespace, out-of-band Secret, never GitOps

The Job runs in `ok-debug` with the `openkite-cargo-cache` PVC and a kubeconfig
Secret that **must never be committed**. Do not point Jobs or Tilt at a
GitOps-managed namespace: ArgoCD self-heal reverts manual deploys.

### 2. Do not decode the pod log; copy artifacts with `kubectl cp`

The payload script runs under `set -x`. An earlier version emitted the artifacts
as base64 on stdout, but the shell trace interleaves with the base64 stream, so
decoding the log fails with:

```
number of data characters cannot be 1 more than a multiple of 4
```

The fix is structural: the media is written to the shared volume and fetched
with `kubectl cp` from a tiny `alpine` fetch pod that mounts the same PVC
(`capture.sh` does this). The log is for evidence only.

### 3. In-cluster only — the agent pod cannot build a Dioxus link

A Dioxus link needs >2.5 GiB and the Hermes agent pod is capped at ~3 GiB; a
build there OOMs and restarts the gateway. Always run the Job in-cluster, never
a local `cargo build`.

### 4. Photograph the X root; prefer clicks and cluster data over keys

- **Screenshot the X root window** (`import -window root`), then crop to the app
  window's client geometry. The webview does not reliably photograph by window
  id.
- **Prefer coordinate clicks and cluster-side data changes** over `xdotool key`
  into the webview. The React table is not bound to `Page_Down`/`Down`/`Up`, and
  an earlier GIF built from keyboard paging was nearly static (~18 bytes between
  frames). The only keyboard path we rely on is `ctrl+p` to open the palette
  after a click focuses the webview (the same path `e2e/run-desktop-e2e.sh`
  uses).
- The layout is deterministic: 252px sidebar, 56px topbar, 20px view padding.
  The click coordinates in `capture-script.sh` are derived from `web/src/theme.css`.
- The most convincing "it is alive" loop is cluster-side: create sample pods in
  the scratch namespace and delete them while capturing, so badge counts, status
  chips and table rows change.

## What it produces

| Media | Surface | Motion |
|---|---|---|
| `console-shell.png` | React console shell: sidebar + count badges, breadcrumbs, status | still |
| `console-workloads.png` | Resource table + log dock on `/workloads` | still |
| `native-logs-route.png` | Native shell on `/logs` (not ported) | still |
| `resource-table.gif` | Live, paginated pods table + per-namespace counts | sample pods created then deleted |
| `inspector.gif` | Inspector slide-over from a selected table row | row click → open → scrim close |
| `toast.gif` | Toast acknowledgement (*Resources refreshed*) | refresh → toast → auto-dismiss |
| `table-controls.gif` | Sortable header + compact-density toggles | header/density clicks |
| `log-dock.gif` | Inline log dock | pause / collapse / clear clicks |
| `command-palette.gif` | Native command palette over the console | `ctrl+p` open → filter → Escape |
| `native-menu-bar-{shown,hidden}.png` | OS menu-bar visibility (OKT-99) | palette action |
| `titlebar-{system,light,dark}.png` | OS decoration theme (OKT-100) | palette action |

> **Known blind spot:** the `titlebar-*` stills come from the headless Xvfb +
> openbox session, which does not paint the OS decoration theme, so the
> System/Light/Dark captures are pixel-identical (ImageMagick `AE ≈ 10`). The
> override is real but has to be photographed on a real desktop to be shown;
> treat these as a capture stub, not evidence of the change.

### Motion evidence

The GIFs are not stills: the Job prints `max_ae=` (the largest count of
differing pixels between consecutive frames) for each one, and the app log
carries the `rows=` delta for the resource table. A healthy GIF reports
`max_ae` in the tens of thousands; a value near zero means the capture did not
actually move and must be fixed, not shipped.

## Media budget

GitHub renders committed images inline only when they stay small. The harness
resizes GIFs to 900 px wide and quantises to 200 colours; keep to:

- GIF: **< 2 MB** target, **< 5 MB** hard ceiling.
- PNG: **< 500 KB**.

If a GIF exceeds budget, trim frames or reduce `OK_CAPTURE_SAMPLE_PODS` rather
than committing something GitHub refuses to render.
