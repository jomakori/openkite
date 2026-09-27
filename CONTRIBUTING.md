# Contributing

## Dev environment

- Rust **stable**, edition 2021. The desktop build needs the platform webview
  stack — on Linux the same packages the `e2e` workflow installs
  (`libwebkit2gtk-4.1-dev`, GTK, `libxdo`), nothing extra on macOS, WebView2 on
  Windows.
- **The cluster comes from a kubeconfig, not from a local one.** Previews and the
  connected E2E job both reach a cluster this way, so local development uses the
  same path: join the tailnet (`tag:k8s`),
  `tailscale configure kubeconfig <proxy-host>`, then run the app against it —
  `cd crates/openkite-desktop && dx serve`. There is no local cluster to create,
  keep alive, or tear down.
- **Do not build the desktop binary on a small review host**: the Dioxus link
  step needs more than ~2.5 GiB and will OOM a ~3 GiB container. Use CI, or the
  one-shot capture Job in `e2e/visual/cluster/`.
- CI gates (all four must pass): `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build --release`.

## Workflow

- Tickets live in **Plane** (`OKT-*` core, `OKA-*` plugins); every PR
  references its ticket.
- One ticket per PR. Move the ticket across the kanban as you go.
- Branch → PR → CI green → squash-merge. No direct pushes to main for ticket
  work.
- **Green required checks are not enough.** Before presenting a PR, enumerate
  the workflows the diff can trigger (`lint-test`, `build-e2e`, `preview`,
  `Release`, plus any path-filtered workflow whose paths the
  diff matches) and read each job's real conclusion — a `success` status can
  hide a skipped or unrun step. After a merge to `main`, check the runs it
  triggered, `Release` first (it is path-filtered and not a required check).
  Full procedure:
  [`.github/workflows/README.md`](.github/workflows/README.md#process-rule-verify-the-workflows-a-change-can-trigger).
  When `Release` is red, use the
  [release runbook](docs/release-runbook.md).

## Conventions

- Commits: `feat|fix|refactor|docs|ci|chore(<scope>): <message>`
  (e.g. `feat(plugin-sdk): add loader`).
- PRs: use `.github/pull_request_template.md`.
- SDK changes: bump the SDK version when its public API changes; never
  change the SDK's public API without a bump.
- Secrets: env vars / Doppler only — never commit credentials.
- Panics must never cross the plugin boundary (see `PluginRegistry`).

## Code style

- rustfmt defaults + clippy clean with `-D warnings`.
- `anyhow` for errors unless a custom type earns its keep.
