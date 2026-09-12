# CI workflows

Continuous integration, packaging, and release automation. GitHub Actions only:
nothing here runs on a developer host (the `openkite` Dioxus link OOMs a small
container — see [`dev/capture/README.md`](../../dev/capture/README.md)).

| Workflow | Trigger | Role |
|---|---|---|
| [`lint-test.yml`](lint-test.yml) | PR, push `main` | fmt / clippy / test / build / bundle-freshness / cross-platform / coverage, plus the `check-portable-sed.sh` hygiene gate. |
| [`e2e.yml`](e2e.yml) | PR, push `main`, dispatch | Desktop E2E, user flows, bridge guard, visual-regression baselines. |
| [`pr-image.yml`](pr-image.yml) | PR on `web/**` | Build the console bundle and publish a PR preview image to GHCR. |
| [`build-artifacts.yml`](build-artifacts.yml) | PR on artifact-affecting paths, `merge_group` | Build the six native release packages **once per commit** as a PR, and gate the **merge queue** with a build-only run on the synthetic merge group (calls the reusable workflow below). |
| [`build-release-artifacts.yml`](build-release-artifacts.yml) | `workflow_call` | Reusable six-target native build + optional `cargo-packager` + optional upload. No cross-compilation: packaging needs `hdiutil` (DMG) and WiX/NSIS (Windows). |
| [`release.yml`](release.yml) | push `main` on Rust paths, dispatch | semantic-release tag/notes, then download-and-attach the PR-built packages (or rebuild as a fallback), then update the Homebrew tap and Chocolatey package. |

## Build once, reuse at release (OKT-104)

`release.yml` used to rebuild all six native targets every time a release fired.
Now the packages are built once per commit by the PR workflow and reused:

```
PR (code-affecting)                         release (push to main)
  build-artifacts.yml                         release.yml
        │ calls                                     │
        ▼                                           ▼
  build-release-artifacts.yml                 prepare: locate the PR build
        │ 6 native builds + packager            │   for this commit (see below)
        │ uploads openkite_<os>_<arch>          │
        └────────────── artifacts ─────────────►│ reuse? ── yes ─► download from the PR run
                                                │                   │
                                                │             no ──► call build-release-artifacts.yml
                                                │                   (version embedded) ─┐
                                                ▼                                       │
                                          publish: normalize asset names ◄──────────────┘
                                          (refuse a partial set) → tag → attach
```

### Locating the PR build

Squash merges mint a brand-new commit SHA, so the PR build cannot be found by
the merged SHA alone. `prepare` maps the merged commit back to its PR head SHA
through the API, finds the newest **successful** `build-artifacts` run for that
SHA, and requires a complete, unexpired six-asset set. Missing / expired /
cancelled → `reuse=false` and the rebuild fallback engages. Publishing is never
blocked by artifact GC (90-day retention), a force-push, or a failed PR job.

The selector and the asset normalisation live in
[`.github/scripts/`](../scripts) so the release path and its verification run
the **same** code:

- `locate-release-artifacts.sh <merged-sha>` — reuse/find decision.
- `normalize-release-assets.sh <dist-dir> <version>` — rewrite the version token
  in every asset name and refuse a partial set.

> The artifact-set completeness check is deliberate and local to OKT-104. The
> broader release fail-safe gate is owned by **OKT-106**; this does not try to
> reproduce it.

## Multi-platform merge gate

The branch's existing required checks (fmt, clippy, tests, build, coverage,
plugin-SDK cross-platform) all run on `ubuntu-latest`, so **none of them
compiled for macOS or Windows** — the GNU-sed-on-BSD-sed defect that took
`Release` red on five consecutive pushes was invisible until a macOS build ran.
`build-artifacts.yml` now reports one stable required context,
**`Multi-platform build`**, that a broken change cannot satisfy, and carries a
`merge_group` trigger so a GitHub merge queue can re-run the six-target native
matrix on its synthetic ref.

> **Merge queue availability.** GitHub gates the merge queue to
> **organization-owned** repositories (public, or private on Enterprise Cloud).
> This repository is owned by a personal user account, so the ruleset API
> rejects a `merge_queue` rule with `422 Invalid rule 'merge_queue'` and the
> queue cannot be enabled here. The `merge_group` trigger and the build-only
> path are already in place and become live the moment the repo belongs to an
> organization (or a custom queue is adopted); until then the gate still
> enforces the six-target native build on every artifact-affecting PR.

```
PR (artifact-affecting)                 merge queue (when available)
  build-artifacts.yml                    build-artifacts.yml
  changes → build (package)              changes (always builds) → build (build-only)
        │                                      │
        └──────────────► gate ◄────────────────┘
                    required: Multi-platform build
```

- **Build-only in the queue.** A merge-group run calls the reusable workflow
  with `package: false` and `upload-artifacts: false`: a compile failure is the
  bug class the gate exists for. PR builds still package (so `release.yml` can
  reuse the set), and packaging stays off the queue until its cost profile is
  measured.
- **Path filters.** Filtering lives in the `changes` job, not
  `on.pull_request.paths`, because the gate is a **required** check: a workflow
  skipped by a top-level path filter reports no check at all, and a docs-only PR
  would sit forever on "Expected". The job always runs and the gate always
  reports; a docs-only PR simply skips the matrix. A `merge_group` has no cheap
  base/head pair to filter on and is the queue's whole purpose, so it always
  builds.
- **Whole-diff semantics.** `dorny/paths-filter` evaluates a pull request
  against the **whole** `base...head` diff, so any commit on an
  artifact-touching PR re-runs the matrix even if the latest commit is
  docs-only. This is accepted rather than gated per-commit: the six-target build
  is not commit-incremental, and `concurrency.cancel-in-progress` keeps only the
  newest PR run alive.
- **Cost.** The gate adds no run on docs-only PRs (the matrix is skipped) and
  reuses the existing per-target `Swatinem/rust-cache` keys. A future queue run
  is build-only, does not upload, and is limited to one synthetic merge at a
  time (see the ruleset parameters) so at most one extra six-target build is in
  flight. macOS and Windows runners are billable, so six targets stay but
  packaging and upload are removed from the queue path.

## Version handling

semantic-release computes the version on `main` **after** merge, so a PR-built
artifact cannot know it. The artifact is therefore **version-agnostic**: the
build does not embed a version in the PR path, and `release.yml` supplies it at
publish time (`prepare` + `normalize-release-assets.sh`). The reusable workflow
still accepts a `version` input, which the **fallback rebuild** passes so a
version-correct artifact can always be produced.

Audit of every version consumer:

| Consumer | Where the version comes from | Effect of a reused (version-agnostic) artifact |
|---|---|---|
| About / settings UI — native status footer | `env!("CARGO_PKG_VERSION")` in `src/router.rs` (`status_bar_model`) | Shows the workspace version compiled into the source tree (`0.0.0`), not the release tag. |
| About / settings UI — React console status bar | `env!("CARGO_PKG_VERSION")` served via `src/react_spike.rs` (`SpikeContext.version`) | Same: the console receives the compiled value. |
| `env!("CARGO_PKG_VERSION")` call sites | compile-time constant | Cannot be rewritten at publish time: a sealed DMG / NSIS installer / AppImage cannot be re-versioned on the Ubuntu publish runner, and the matrix deliberately does not cross-compile. |
| Release asset filename `openkite_<semver>_<os>_<arch>.<ext>` | `normalize-release-assets.sh` at publish time | Correct: the PR artifact carries a placeholder token that is rewritten to the analyzed version. |
| Homebrew cask / formula (`jomakori/homebrew-tap`) | release version via `release-digest` + semantic-release | Unaffected — the tap is updated from the release tag, not from the binary. (The formula builds from source, so it already compiled `0.0.0` before this change.) |
| Chocolatey `openkite.nuspec` / `chocolateyinstall.ps1` | release version at pack time | Unaffected — the nuspec version, installer `$version`, and checksums all come from the release tag. |

The one real cost is the About/status version for reused artifacts. Making it
exact would require re-versioning the binary after package sealing, which is
infeasible cross-platform on the Ubuntu publish runner; the fallback rebuild is
the version-correct path when it matters.
