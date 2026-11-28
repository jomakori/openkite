<div align="center">
  <img src="docs/media/banner.svg" alt="OpenKite — Kubernetes from above" width="880">

  <p align="center">
    <img src="https://img.shields.io/github/actions/workflow/status/jomakori/openkite/lint-test.yml?logo=githubactions&logoColor=white&label=CI" alt="CI">
    <img src="https://img.shields.io/github/license/jomakori/openkite?logo=opensourceinitiative&logoColor=white&label=License" alt="License">
  </p>
</div>

## What is it?

OpenKite is a Kubernetes IDE in Rust: a Dioxus 0.7 console over a kube-rs engine,
served as a browser host today and as a desktop window from the same crates.

## Status

v1 was retired and the v2 workspace replaced it: six crates — `engine`, `api`,
`ui`, `desktop`, `web`, `engine-testkit` — that the rebuild has since filled in.

The browser host is the working product. `openkite-web` server-renders the console
at `/` (plus `/table`, `/events` and `/styles/console.css`) over the engine's demo
fixtures, exposes `/healthz`, and honours `PORT`. The console is the real thing:
workload table with selection and bulk actions, log dock, resource inspector, nav
tree, command palette, overview with capacity meters and the usage drill-down, and
the settings pane with 24 presets, plus the YAML manifest editor. The Argo CD
application view is specified and styled ahead of the live cluster wiring.

The engine holds the cluster side: informer caches, refcounted interest, coalesced
update batches, pod exec, port-forward and log streaming, plus the workload actions.
Its conformance suite runs against a kind cluster and diffs exec behaviour against
`kubectl`.

`openkite-desktop` is the remaining stub — the native window over the in-process
engine channel is the next milestone; until it lands the web host is the only way to
run the console.

## Running it

```bash
cargo run -p openkite-web            # http://localhost:8080, PORT overrides
cargo test --workspace               # what CI runs, minus the volume benchmark
```

The container build compiles `openkite-web` with the `fixtures` feature, so the
image serves the demo cluster without needing a kubeconfig.

## Releases

Releases are dispatched, not tagged by hand: `Release` derives the next version
with `svu`, pushes the tag, and retags the image the pull request's `Staging` run
already built rather than rebuilding it. `Flatpak` and `Chocolatey` are queued from
the same dispatch. Every pull request also gets a `Preview` deployment on the dev
cluster.

## Documentation

- [DESIGN.md](DESIGN.md) — the token contract the console is styled from
- [docs/design-surfaces.md](docs/design-surfaces.md) — which surface consumes which
  tokens, and the reference vocabulary it is graded against
- [docs/architecture.md](docs/architecture.md) — crates, the collection model, the
  deferral list
- [CONTRIBUTING.md](CONTRIBUTING.md) — the workspace map, the crate boundary rule,
  the CI gates

The design reference itself is vendored, unmodified, in
[`docs/reference/opendesign-console/`](docs/reference/opendesign-console/).

## License

MIT + Apache-2.0 (dual) — see `LICENSE-MIT` and `LICENSE-APACHE`.
