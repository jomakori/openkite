# Dev environment

One-command dev loop with Tilt + k3d.

## Prereqs

- Docker (OrbStack works)
- [Tilt](https://tilt.dev) (`brew install tilt`)
- [k3d](https://k3d.io) (`brew install k3d`)

## Quickstart

```bash
./dev/k3d-create.sh                     # create the openkite-dev cluster + registry
KUBECONFIG=dev/.kube/config tilt up      # compile-check + deploy sample workloads
```

## What `tilt up` gives you

- **`cargo-check`** — `cargo check --workspace` inside the `openkite-dev`
  container, re-running on every source change (cargo registry + target
  cached in named volumes, so only the first run downloads deps).
- **Sample workloads** — nginx, podinfo, and a crashlooping pod in the
  `openkite-dev` cluster for the UI to render against.

## Cleanup

```bash
k3d cluster delete openkite-dev
```
