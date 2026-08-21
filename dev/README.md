# Dev environment

One-command dev loop with Tilt + k3d. The cluster is **ephemeral** — sprung up
on demand and torn down when you're done, never left running.

## Prereqs

- Docker (OrbStack works)
- [Tilt](https://tilt.dev) (`brew install tilt`)
- [k3d](https://k3d.io) (`brew install k3d`)

## Quickstart

```bash
./dev/k3d-create.sh                     # spring up the cluster + registry
KUBECONFIG=dev/.kube/config tilt up      # compile-check + deploy sample workloads
# ... develop ...
tilt down                               # stop Tilt
./dev/k3d-down.sh                       # tear down the cluster + registry
```

## What `tilt up` gives you

- **`cargo-check`** — `cargo check --workspace` inside the `openkite-dev`
  container, re-running on every source change (cargo registry + target
  cached in named volumes, so only the first run downloads deps).
- **`openkite-desktop`** — `dx serve` (Dioxus dev server + hot reload).
- **Sample workloads** — nginx, podinfo, and a crashlooping pod in the
  `openkite-dev` cluster for the UI to render against.

## Cleanup

```bash
./dev/k3d-down.sh
```
