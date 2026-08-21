# OpenKite

**Kubernetes from above.**

An open-source Kubernetes IDE. Pure Rust — Dioxus 0.7 (desktop) + kube-rs.
Single language. Single binary. Desktop-first; web and mobile are
architecture-aware but not built yet.

## Status

Phase 1 (core dashboard + plugin system) — in development. See the
[spec](https://docs.google.com/document/d/1u8GwBW2lEYW1lhJVKS5uKD9tWNetBJtS9fWdgQK2D38/edit)
and the [OpenKite board](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f).

## Features (target)

- Cluster connect via kubeconfig, multi-context switching
- Resource views: Pods, Deployments, Services, ConfigMaps, Secrets, DaemonSets, StatefulSets, ReplicaSets, Jobs, CronJobs
- Virtualized resource tables, log viewer, YAML editor (CodeMirror 6), embedded terminal (portable-pty + xterm.js)
- Metrics: metrics-server (Tier 1) + auto-detected Prometheus (Tier 2)
- Plugin system via `openkite-plugin-sdk` (static-first; dylib experimental)
- Theme engine: 5 built-in themes + Zed theme JSON import

## Prerequisites

- Rust toolchain (rustup)
- Dioxus CLI: `cargo install dioxus-cli`
- Tilt: `curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash`
- A local K8s cluster: k3d (OrbStack/Docker)
- `helm` on PATH (for Helm operations inside the app)

## Development

```bash
k3d cluster create openkite-dev --registry-create openkite-registry:5050
tilt up        # Tilt UI at localhost:10350
```

Desktop-only loop: `dx serve` (subsecond hot reload).

## License

MIT + Apache-2.0 (dual).
