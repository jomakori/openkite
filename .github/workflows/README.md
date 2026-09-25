# CI workflows

Continuous integration, packaging, and release automation. GitHub Actions only:
nothing here runs on a developer host (the `openkite` Dioxus link OOMs a small
container — see [`e2e/visual/cluster/README.md`](../../e2e/visual/cluster/README.md)).

Four workflows, one job each of them owns:

| Workflow | Trigger | Jobs |
|---|---|---|
| [`lint-test.yml`](lint-test.yml) | `pull_request` (drafts included), push `main` | `changes` · `fmt` · `clippy` · `test` · `build` · `bundle-freshness` · `cross-platform` · `web-target` · `coverage` · `report` |
| [`build-e2e.yml`](build-e2e.yml) | `pull_request` (not draft), push `main` | `changes` · `binaries` (6 targets) · `e2e-binary` · `desktop-e2e` · `user-flows` · `bridge-guard` · `desktop-e2e-connected` · `visual-regression` · `gate` · `report` |
| [`preview.yml`](preview.yml) | `pull_request` on the `preview` label | `image` · `ready` · `prune` · `report` |
| [`release.yml`](release.yml) | push `main` (Rust/web/CI paths), dispatch | `analyze` · `locate` · `packages` · `publish` · `image` · `image-rebuild` · `manifest` · `tap` · `choco` · `report` |

Everything shared lives in [`../actions/`](../actions) as composite actions:
`rust-setup` (toolchain + the one apt list + cache), `build-image` (the one image
build and push), `build-release-artifacts` (one native target), `notify-failure`
(the `ci-failure` issue), `release-digest` (asset digests).

Validate a change locally before pushing:

```bash
pre-commit run --files <changed files>
actionlint .github/workflows/<file>
yamllint .github/workflows .github/actions   # config: ../../.yamllint.yml
```

## Draft → ready replaces the merge queue

A PR opens as a draft: `lint-test.yml` runs (fast feedback, nothing heavy) and
`build-e2e.yml` runs nothing expensive. Marking it ready fires `ready_for_review`
and starts the six-target matrix, the e2e binary, the five suites, and the
`gate`. Later pushes while ready keep it running; a return to draft stops it
again. Every expensive job carries the same job-level guard
(`github.event.pull_request.draft == false` plus the path filter) — never a
per-step one — so a required context can only report a real result.

A draft cannot be merged, so nothing skips the gate: the `gate` context exists
exactly when the PR is mergeable. The cost, stated plainly: a PR marked ready
before it is genuinely ready pays a full matrix and the e2e suites. `draft` is a
workflow control here, not just a signal.

## Preview — one label, no image no preview

`gke_GitOps` runs an `ApplicationSet` that turns **labelled** open PRs into
preview environments, rendering `ghcr.io/jomakori/openkite:pr-<N>` into that
PR's own namespace. The label is the switch, and `preview.yml` owns it:

| Image outcome | Label | Result |
|---|---|---|
| built and pushed as `pr-<N>` + `pr-<N>-<sha>` | left in place | preview Application created, deployed |
| build fails | `preview` removed | preview Application removed, namespace pruned |

The label is a human input: apply `preview` to a PR and the workflow builds the
PR head. Nothing in CI creates the label. If a build fails the label is withdrawn
so the deploy switch never lies about an image that does not exist — re-apply it
after a fix. A PR from a fork has a read-only token (no image, no label) and so
never gets a preview.

Consequence to expect: a preview only exists once `preview.yml` has gone green
for the PR head. If the preview URL 404s, check the label before debugging the
cluster:

```bash
gh pr view <N> --json labels --jq '.labels[].name'
```

## Teardown — what a closed PR leaves behind

| Artifact | Owner | On close or merge |
|---|---|---|
| Namespace, Application, workload | the `ApplicationSet` in `gke_GitOps` | pruned by the generator |
| Image versions `pr-<N>` and `pr-<N>-<sha>` | `preview.yml` (`prune`) | a **merged** PR keeps its sha-pinned version and loses only the mutable `pr-<N>`; a closed-unmerged PR loses both |

Registry cleanup has no backstop to configure: GitHub's package retention rules
are **organisation-only** and `openkite` is a user-owned package. The weekly
sweep that used to be the backstop is gone with `pr-artifacts-cleanup.yml`; sweep
by hand through the same script:

```bash
./.github/scripts/prune-pr-image.sh --sweep     # every closed PR, --merged semantics per PR
./.github/scripts/prune-pr-image.sh 131         # one closed-unmerged PR
./.github/scripts/prune-pr-image.sh --merged 130  # one merged PR: keep the sha pin
```

Verify against the registry, never the PR list:

```bash
gh api /users/jomakori/packages/container/openkite/versions --paginate \
  -q '[.[].metadata.container.tags[]]|length'
```

## Build once, reuse at release (OKT-104)

The six native packages are built once per commit by `build-e2e.yml` and reused:

```
PR (artifact-affecting)                     release (push to main)
  build-e2e.yml                               release.yml
        │ six native builds                    │
        │ uploads openkite_<os>_<arch>          ▼
        └──────────── artifacts ──────────► locate: the PR build for this commit
                                                │ reuse? ── yes ─► download from the PR run
                                                │             no ─► packages (fallback rebuild,
                                                │                    version embedded)
                                                ▼
                                          publish: normalize asset names
                                          (refuse a partial set) → tag → attach
```

### Locating the PR build

Squash merges mint a brand-new commit SHA, so the PR build cannot be found by
the merged SHA alone. `locate` maps the merged commit back to its PR head SHA
through the API, finds the newest **successful** `build-e2e` run for that SHA,
and requires a complete, unexpired six-asset set. Missing, expired, cancelled, or
a direct push to `main` → `reuse=false` and the `packages` fallback rebuilds.
Publishing is never blocked by artifact GC (90-day retention), a force-push, or a
failed PR job.

The selector and the asset normalisation live in [`.github/scripts/`](../scripts)
so the release path and its verification run the **same** code:

- `locate-release-artifacts.sh <merged-sha>` — reuse/find decision, plus the PR's
  sha-pinned image tag for the image promotion.
- `normalize-release-assets.sh <dist-dir> <version>` — rewrite the version token
  in every asset name and refuse a partial set.

## Release fail-safe

`publish` runs the preflight unconditionally, and it runs **before** anything is
tagged or uploaded: `normalize-release-assets.sh` requires all six targets by
default and fails the job otherwise. A dry run is allowed through with whatever
(possibly empty) set the download produced precisely so the gate can be exercised
without publishing; the tagging, uploading and image steps are gated off for it.

`targets` may only be narrowed by an explicit `workflow_dispatch` — a partial
release is a conscious act, never an accident of a cancelled job.

## The image the release ships

The release promotes the image the PR already built. `locate` maps the merged
commit to its PR and to that PR's sha-pinned tag (`pr-<N>-<sha>`); `image` runs
`docker buildx imagetools create --tag ghcr.io/jomakori/openkite:v<semver>
ghcr.io/jomakori/openkite:pr-<N>-<sha>`. A rebuild from the tag happens only when
there is no preview image to promote (no PR for the commit, or the tag was never
published), in the separate `image-rebuild` job.

`manifest` then attaches `openkite_<semver>_release-manifest.md` to the release:
the six package digests plus the image digest at `v<semver>`.

## Required contexts

Seven contexts keep their exact names:

`Check formatting` · `Lint with clippy` · `Run tests` · `Build` · `Coverage gate`
· `plugin-sdk cross-platform (ubuntu-latest)` · `Multi-platform build`

Path filtering lives in a `changes` job, not in `on.pull_request.paths`: a
workflow skipped by a top-level path filter reports no check at all, so a
docs-only PR would sit on "Expected" forever. `changes` always runs and always
reports; the heavy jobs skip on a docs-only PR, which is a skipped (not a
fail-open green) context.

## Post-merge surfacing

Each of the four workflows carries a `report` job. On a push-to-main run it opens
or updates exactly **one** `ci-failure` issue per workflow
(`[ci] <workflow> is red on main`, body updated in place rather than a comment per
failure), and on a green push-to-main run it closes that workflow's issue.

This is the post-merge fail-safe: a green *required-check* set does not prove the
workflows a merge triggered actually succeeded, and `release.yml` is
path-filtered, so a workflow-only merge never runs it.

## Process rule: verify the workflows a change can trigger

Write down the workflows your diff can trigger — `lint-test`, `build-e2e`,
`preview`, `Release` — and say which ones you actually observed. "The PR used to
pass" is not the same statement as "these workflows passed on this diff", and
only the second one is evidence.

## Version handling

`Cargo.toml` stays at `[workspace.package] version = "0.0.0"` on `main`. The
release **fallback** build embeds the analyzed version into the binary at build
time (OKT-103); a reused PR artifact was built version-agnostically and its
release asset name is rewritten at publish time. The filename is not what the
binary reports — it is the release asset convention. See
[`docs/release-runbook.md`](../../docs/release-runbook.md) for recovery.
