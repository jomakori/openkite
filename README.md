<div align="center">
  <h1>OpenKite</h1>
  <p><em>Kubernetes from above.</em></p>

  <p align="center">
    <img src="https://img.shields.io/github/actions/workflow/status/jomakori/openkite/lint-test.yml?logo=githubactions&logoColor=white&label=CI" alt="CI">
    <img src="https://img.shields.io/github/license/jomakori/openkite?logo=opensourceinitiative&logoColor=white&label=License" alt="License">
    <img src="https://img.shields.io/github/stars/jomakori/openkite?logo=github&logoColor=white&label=Stars" alt="Stars">
    <img src="https://img.shields.io/github/last-commit/jomakori/openkite?logo=git&logoColor=white&label=Last%20commit" alt="Last commit">
  </p>
</div>

## Quick Links

- [What is it?](#what-is-it) · [Features](#features) · [Quickstart](#quickstart) · [Documentation](#documentation) · [Contributing](#contributing)

## What is it?

OpenKite is an open-source Kubernetes IDE in pure Rust — Dioxus 0.7 (desktop)
with kube-rs 4. One language, one binary, desktop-first.

## Features

- **Cluster connect** — kubeconfig loading, multi-context switching
- **Resource views** — Pods, Deployments, Services, ConfigMaps, Secrets, DaemonSets, StatefulSets, ReplicaSets, Jobs, CronJobs
- **Virtualized tables** — sortable, filterable, windowed (reflector-backed live state)
- **Pod detail** — containers, status, events
- **Log viewer** — follow/pause, capped buffer
- **Secret redaction** — masked by default, explicit reveal
- **Theme engine** — 5 built-in themes + Zed JSON import
- **Command palette** — fuzzy matcher (Cmd+K)
- **Metrics** — sparklines (metrics-server) + auto-detected Prometheus
- **Plugin system** — `openkite-plugin-sdk`, static-first (dylib experimental)

> YAML editor (CodeMirror 6) and embedded terminal (portable-pty + xterm.js)
> are in progress — see the [board](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f).

## Quickstart

```sh
# Desktop dev loop (hot reload)
cargo install dioxus-cli
dx serve

# Or via Tilt + ephemeral k3d cluster
k3d cluster create openkite-dev --registry-create openkite-registry:5050
tilt up

# Plain build
cargo build --release
```

## Documentation

- [Architecture](docs/architecture.md)
- [Plugin development](docs/plugin-development.md)
- [Theming](docs/theming.md)
- [Contributing](CONTRIBUTING.md)

## License

MIT + Apache-2.0 (dual) — see `LICENSE-MIT` and `LICENSE-APACHE`.
