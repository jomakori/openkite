# CI workflows

Continuous integration, packaging, and release automation. GitHub Actions only:
nothing here runs on a developer host (the `openkite` Dioxus link OOMs a small
container — see [`dev/capture/README.md`](../../dev/capture/README.md)).

| Workflow | Trigger | Role |
|---|---|---|
| [`lint-test.yml`](lint-test.yml) | PR, push `main` | fmt / clippy / test / build / bundle-freshness / cross-platform / coverage, plus the `check-portable-sed.sh` hygiene gate. |
| [`e2e.yml`](e2e.yml) | PR, push `main`, dispatch | Desktop E2E, user flows, bridge guard, visual-regression baselines. |
| [`pr-image.yml`](pr-image.yml) | PR on `web/**` | Build the console bundle and publish a PR preview image to GHCR. |
| [`build-artifacts.yml`](build-artifacts.yml) | PR on artifact-affecting paths | Build the six native release packages **once per commit** (calls the reusable workflow below). |
| [`build-release-artifacts.yml`](build-release-artifacts.yml) | `workflow_call` | Reusable six-target native build + `cargo-packager` + upload. No cross-compilation: packaging needs `hdiutil` (DMG) and WiX/NSIS (Windows). |
| [`release.yml`](release.yml) | push `main` on Rust paths, dispatch | semantic-release tag/notes, then download-and-attach the PR-built packages (or rebuild as a fallback), then update the Homebrew tap and Chocolatey package. `Publish release` runs a preflight that refuses a partial artifact set (see below). |
| [`main-failure-tracker.yml`](main-failure-tracker.yml) | `workflow_run` on `main` failure, dispatch | Open or update exactly one tracking issue per failed workflow on `main`, naming the failing job(s) and the log link. |

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

## Release fail-safe

`Publish release` starts with a preflight — `normalize-release-assets.sh <dist>
<version> [targets]` — that refuses a partial release:

- **Default (no input): all six targets are required.** A missing asset fails
  the job with `::error title=Incomplete release artifact set::missing: …` and
  nothing is tagged or uploaded. This is the only value a push-triggered run
  can produce, so a partial release is impossible by accident.
- **A deliberate subset requires the explicit `workflow_dispatch` input
  `targets`** (comma-separated, e.g. `linux_amd64,macos_arm64`). When set, only
  those assets are required and the others are pruned, so publishing fewer
  targets is a conscious act rather than a silent outcome.
- **`dry_run=true`** runs `analyze` + the preflight without tagging, uploading,
  or touching the Homebrew/Chocolatey package managers. It never publishes, so
  it is safe to use to exercise the gate.

The gate extends the OKT-104 completeness check in the same script instead of
adding a second, competing preflight.

## Post-merge surfacing

`main-failure-tracker.yml` runs when `lint-test`, `e2e`, or `Release` completes
on `main`. A failure opens exactly one issue per workflow, naming the workflow,
the failing job(s) and their log links; a later failure comments on that same
issue instead of filing a duplicate. Manual re-report:
`gh workflow run main-failure-tracker.yml -f run_id=<id> -f workflow_name=Release`.

## Process rule: verify the workflows a change can trigger

A green set of **required** checks is not evidence that the workflows a merge
triggers succeeded. `release.yml` is path-filtered and is not a required check,
so it can be red while every required check is green — the failure mode that let
`Release` fail on five consecutive pushes to `main` unnoticed.

**Before presenting a PR**, enumerate every workflow the diff can trigger (from
the trigger column above — include `lint-test`, `e2e`, `pr-image`, `build-artifacts`,
and `Release`, plus any path-filtered workflow whose paths the changed files
match) and read each job's **real** conclusion. A `success` status can hide a
skipped or unrun step:

```sh
gh run view <run-id> --repo jomakori/openkite --json conclusion,jobs \
  --jq '.jobs[] | "\(.conclusion)\t\(.name)"'
gh run view <run-id> --repo jomakori/openkite --log-failed
```

Treat `skipped` as "not verified", not "passed".

**After every merge to `main`**, list the runs the merge triggered and check
their conclusions — `Release` first:

```sh
gh run list --repo jomakori/openkite --commit "$(git rev-parse HEAD)" \
  --json name,conclusion,url --jq '.[] | "\(.conclusion)\t\(.name)\t\(.url)"'
```

A workflow-only merge (`.github/**`, docs) does **not** trigger `Release`, so the
release pipeline stays unverified until a Rust/Cargo change merges or someone
dispatches it deliberately — and a dispatch publishes. The red-release runbook is
[`docs/release-runbook.md`](../../docs/release-runbook.md).

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
