# Versioning & Releases

OpenKite follows [Semantic Versioning](https://semver.org/).

## Pre-1.0 (current: `0.1.0`)

While the workspace is `0.x`:

- **Patch** (`0.0.x`) — bug fixes and additive, non-breaking additions.
- **Minor** (`0.x.0`) — new features; pre-1.0 this may also carry breaking
  changes, documented in the release notes.

## Crate versions

- Root `openkite` binary: `version.workspace = true` — bump via
  `[workspace.package]` in `Cargo.toml`.
- `openkite-plugin-sdk`: independently versioned — it is the **stable contract**
  between core and plugins. Full policy in `crates/plugin-sdk/src/lib.rs`;
  summary: major = breaking (plugins must recompile), minor = features,
  patch = fixes/additive.

**Any PR touching the SDK's public API must bump the SDK version in the same
PR** and note it in the PR body (see the template).

## MSRV

- CI runs Rust **stable**; no MSRV pin is enforced.
- Crates target edition 2021.

## Releases

1. Bump the relevant version(s) (commit: `chore(release): vX.Y.Z`).
2. Tag `vX.Y.Z` and push — `release.yml` builds the release binary and drafts
   a GitHub release.
3. Notes summarize user-facing changes from conventional commits.
