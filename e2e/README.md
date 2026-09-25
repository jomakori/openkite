# End-to-end tests

Two layers, one boundary:

- `crates/openkite-desktop/tests/` — in-process Rust, run by `cargo nextest`. Fast, no X server.
- `e2e/` — launches the **built binary** under a virtual X display and drives the X11 layer.
  Slower, and much closer to what a user does.

## Layout

```
e2e/
├─ desktop/      run.sh · flows/{user_flows.bats, flows_lib.bash}
├─ visual/       capture.sh · compare.sh · run.sh · baselines/*.png
│  └─ cluster/   the one-shot in-cluster capture Job (README lives there)
├─ bridge/       guard.sh · fixtures/{manifest.json, guard.js}
└─ artifacts/    every suite writes here (the only ignored output path)
```

A suite directory is named after what it tests — never after a CI job or a ticket.

## Suites

### `desktop/run.sh <binary> <artifact-dir>`

Boots the real desktop binary under Xvfb, asserts a window appears, screenshots the shell, and
exercises the Ctrl+P palette the webview handles through `document::eval`.
Requires `xvfb`, `xdotool`, `imagemagick`, `dbus-x11`.

```sh
# from e2e/, with the binary built from the workspace root (target/ is workspace-wide)
./desktop/run.sh ../target/debug/openkite artifacts/desktop
```

The connected variant is the same script against a real kubeconfig — that is the whole difference:

```sh
KUBECONFIG="$HOME/.kube/config" ./desktop/run.sh ../target/debug/openkite artifacts/connected
```

### `desktop/flows/` — user flows as a bats suite

```sh
ART_ROOT=artifacts/flows bats --report-formatter junit --output artifacts/junit \
  --tempdir artifacts/tmp desktop/flows/user_flows.bats
```

`--tempdir` must **not** exist beforehand: bats creates it and exits with
`BATS_RUN_TMPDIR … already exists`. Only the `--output` directory is pre-made.

### `visual/` — baselines and the gate

`capture.sh` produces the ten surface screenshots. `run.sh` captures a fresh set and compares it
against the committed baselines with `compare.sh` (ImageMagick).

```sh
./visual/run.sh ../target/debug/openkite visual/baselines artifacts/visual
```

A change that alters what a user sees refreshes `visual/baselines/` in the same PR: a moved
baseline is a decision, not an accident.

`visual/cluster/` is the OOM-safe producer — a one-shot Job that builds the app in-cluster and
fetches the media into `docs/media/`. See its [README](visual/cluster/README.md).

### `bridge/guard.sh <binary> <artifact-dir>`

Installs the test-only plugin in `bridge/fixtures/` into an isolated `$HOME`, then asserts the
`/openkite` dispatch answered with a well-formed JSON envelope, that the Dioxus-side registration
mirror wrote, and that **no panic** reached `app.log`.

```sh
./bridge/guard.sh ../target/debug/openkite artifacts/bridge
```

## Artifacts

Every suite writes under `e2e/artifacts/<suite>/` — the single ignored output root. The CI jobs
upload from it: `artifacts/desktop`, `artifacts/flows`, `artifacts/junit`, `artifacts/bridge`,
`artifacts/connected`, `artifacts/visual`.

## Do not build the binary on a review host

The Dioxus link step wants more than ~2.5 GiB and OOM-kills a small container. Let CI build it, or
use the capture Job in `visual/cluster/`.
