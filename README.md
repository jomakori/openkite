<div align="center">
  <img src="docs/media/banner.svg" alt="OpenKite — Kubernetes from above" width="880">

  <p align="center">
    <img src="https://img.shields.io/github/actions/workflow/status/jomakori/openkite/lint-test.yml?logo=githubactions&logoColor=white&label=CI" alt="CI">
    <img src="https://img.shields.io/github/license/jomakori/openkite?logo=opensourceinitiative&logoColor=white&label=License" alt="License">
  </p>
</div>

## What is it?

OpenKite is a Kubernetes IDE in Rust: a Dioxus 0.7 console over a kube-rs engine,
served as a desktop window and as a browser host from the same crates.

## Status — rebuild in progress

v1 was retired: its crates, end-to-end suites, packaging and release workflows are
gone, and the repository holds a v2 workspace skeleton instead — six crates
(`engine`, `api`, `ui`, `desktop`, `web`, `cli`) that the rebuild fills in.

The browser shell is real but thin: `openkite-web` serves a placeholder page and a
`/healthz` endpoint, and every release tag builds an arm64 image that the cluster
runs. Everything else in the workspace is still a stub.

## Running the shell

```bash
cargo run -p openkite-web          # http://localhost:8080, PORT overrides
```

Releases are cut by pushing a `v*` tag: the `Release` workflow builds the
`linux/arm64` image, pushes `ghcr.io/jomakori/openkite:<tag>` (and `:latest`), and
opens the GitHub release.

## Documentation

- [DESIGN.md](DESIGN.md) — the token contract the console is styled from
- [docs/design-surfaces.md](docs/design-surfaces.md) — which surface consumes which
  tokens, and the reference lines it is graded against
- [CONTRIBUTING.md](CONTRIBUTING.md) — the workspace map, the crate boundary rule,
  the CI gates

The design reference itself is vendored, unmodified, in
[`docs/reference/opendesign-console/`](docs/reference/opendesign-console/).

## License

MIT + Apache-2.0 (dual) — see `LICENSE-MIT` and `LICENSE-APACHE`.
