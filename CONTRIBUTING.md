# Contributing

## Dev environment

- Rust **stable**, edition 2021. No local toolchain required for the Tilt path.
- Dev loop: see `dev/README.md` — ephemeral k3d cluster + Tilt
  (`cargo check` runs in a container; `dx serve` hot-reloads the desktop app).
  Tear down when done (`tilt down && ./dev/k3d-down.sh`).
- CI gates (all four must pass): `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build --release`.

## Workflow

- Tickets live in **Plane** (`OKT-*` core, `OKA-*` plugins); every PR
  references its ticket.
- One ticket per PR. Move the ticket across the kanban as you go.
- Branch → PR → CI green → squash-merge. No direct pushes to main for ticket
  work.

## Conventions

- Commits: `feat|fix|refactor|docs|ci|chore(<scope>): <message>`
  (e.g. `feat(plugin-sdk): add loader`).
- PRs: use `.github/pull_request_template.md`.
- SDK changes: bump the version per `VERSIONING.md`; never change the SDK's
  public API without a bump.
- Secrets: env vars / Doppler only — never commit credentials.
- Panics must never cross the plugin boundary (see `PluginRegistry`).

## Code style

- rustfmt defaults + clippy clean with `-D warnings`.
- `anyhow` for errors unless a custom type earns its keep.
