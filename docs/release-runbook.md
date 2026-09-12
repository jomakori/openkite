# Release runbook — "Release went red"

How to diagnose and recover when the [Release workflow](../.github/workflows/release.yml)
fails. It is accurate against the workflows as they are after OKT-103 (portable
version embed + Windows fix) and OKT-104 (build artifacts once, reuse at
release), plus the OKT-106 fail-safe in this repository.

`Release` is **not** a required check. It is path-filtered to `**.rs`,
`**/Cargo.toml` and `Cargo.lock`, so it can be red while every required check is
green — and a workflow-only merge never triggers it at all. That is the whole
reason this runbook exists.

## 1. Identify the failing target

```sh
# newest runs, newest first
gh run list --repo jomakori/openkite --workflow release.yml --limit 10

# which job failed, and each job's real conclusion
gh run view <run-id> --repo jomakori/openkite --json conclusion,jobs \
  --jq '.jobs[] | "\(.conclusion)\t\(.name)"'

# the failing steps' logs
gh run view <run-id> --repo jomakori/openkite --log-failed
```

Job names map to targets: `openkite_<os>_<arch>` (`analyze` = Determine
version, `prepare` = Locate reusable artifacts, `publish` = Publish release,
`build-artifacts` = Rebuild release artifacts (fallback)). A failure in one
`openkite_*` job fails the reusable build, so `publish` is skipped and no
release is cut — the fail-safe working as intended.

If `Publish release` itself failed with:

```
::error title=Incomplete release artifact set::missing: openkite_<version>_<os>_<arch>.<ext> ...
```

the artifact set was incomplete. Go to §3.

## 2. Re-run only the failing job

```sh
# failed job ids
gh run view <run-id> --repo jomakori/openkite --json jobs \
  --jq '.jobs[] | select(.conclusion == "failure") | "\(.databaseId)\t\(.name)"'

# re-run a single job (re-runs just that job, not the whole workflow)
gh api -X POST repos/jomakori/openkite/actions/jobs/<job-id>/rerun

# or re-run every failed job in the run
gh run rerun <run-id> --repo jomakori/openkite --failed
```

`publish` is gated on the build jobs, so re-running a recovered build job will
re-run the downstream jobs that were skipped.

## 3. Force a rebuild when artifacts are missing or expired

`release.yml` reuses the six packages a PR already built (`prepare` →
`locate-release-artifacts.sh`). Reuse requires a **successful `build-artifacts`
run for the PR head SHA** with all six assets present and unexpired. When it is
missing, expired (PR artifacts: 90 days; fallback: 7 days), cancelled, or the
merge was a direct push, the locator prints a `::notice title=Rebuilding release
artifacts::` and `reuse=false`; the `build-artifacts` fallback job then rebuilds
all six targets with the analyzed version embedded. **The rebuild is automatic —
there is no flag to set.**

To force it:

- Re-run the failed `Release` run (`gh run rerun <run-id> --repo
  jomakori/openkite --failed`), or dispatch it (see §5), which re-evaluates
  reuse from scratch.
- To repopulate the reusable set instead, re-run the PR's `build-artifacts`
  workflow while the PR still exists:
  `gh run rerun <build-artifacts-run-id> --repo jomakori/openkite`.
- A PR whose diff does not match `build-artifacts.yml`'s paths never built
  artifacts in the first place; the fallback covers it at release time.

## 4. Ship a hotfix

1. Branch from current `main`; make the minimal `fix(<scope>): …` commit.
2. Open the PR and verify every workflow the diff can trigger (see the
   [process rule](../.github/workflows/README.md#process-rule-verify-the-workflows-a-change-can-trigger)).
3. Merge. If the diff touched `**.rs`, `**/Cargo.toml` or `Cargo.lock`,
   `Release` fires; if it was a workflow-only or docs-only fix, it does **not** —
   go to §5 and dispatch it deliberately.
4. Check `Release` first, then the other runs the merge triggered, before
   calling it done.

## 5. The path-filter trap — trigger Release deliberately

Because `release.yml` only fires on Rust/Cargo changes, a workflow-only merge can
leave the release pipeline unverified indefinitely (this is exactly how five red
`Release` runs went unnoticed). To trigger it on the current `main`:

```sh
gh workflow run release.yml --repo jomakori/openkite --ref main
```

**What that does:** runs `analyze` (semantic-release dry-run), then `prepare`
(reuse or fallback rebuild), the preflight, and — if a release is due —
`publish`, which **tags and publishes a real release** and updates the Homebrew
tap and Chocolatey package. Do not use it casually.

To validate the pipeline **without publishing**:

```sh
# analyze + artifact preflight only: no tag, no upload, no package managers
gh workflow run release.yml --repo jomakori/openkite --ref main -f dry_run=true

# require only a subset of targets (also publishes) — a deliberate subset
gh workflow run release.yml --repo jomakori/openkite --ref main \
  -f targets=linux_amd64,macos_arm64
```

`dry_run=true` never publishes. Without it, a dispatch publishes. The default
(with no `targets`) requires all six artifacts, so an incomplete set fails the
preflight instead of producing a partial release.

## 6. Temporarily pin a version

There is no release-version override input on `release.yml`. The tagged version
is computed by semantic-release from commit messages (`.releaserc.json`), and
`Cargo.toml` stays at `[workspace.package] version = "0.0.0"` on `main` — branch
protection blocks the semantic-release-cargo bump commit, which is why the
fallback build embeds the analyzed version into the binary at build time
(OKT-103). So "pinning" has three distinct meanings here:

- **Hold a release back while a regression is fixed.** Do not merge
  release-triggering (`.rs` / `Cargo.toml` / `Cargo.lock`) changes; a
  workflow-only or docs merge will not cut a release. `Release` stays unverified
  until you dispatch it.
- **Pin a dependency or packaging tool that a bad upstream release broke** (the
  usual reason a release goes red after an unrelated bump). Pin the crate in the
  lockfile and commit it — `cargo update -p <crate> --precise <version>` — or pin
  the tool in `.github/workflows/build-release-artifacts.yml`:
  `cargo install cargo-packager --version <x> --locked`.
- **Force a specific version to be embedded.** The reusable workflow's `version`
  input is authoritative for the fallback path; it is `workflow_call`-only, so
  wire it through a caller (e.g. a temporary `workflow_dispatch` shim) if you
  must build a pinned version. As a last resort, create the release manually with
  `gh release create vX.Y.Z --title … --notes …` and upload the assets — the next
  semantic-release run computes its next version from that tag.

## 7. Verify the recovery

After any fix, confirm the **real** conclusions, not just that the run is green:

```sh
gh run view <run-id> --repo jomakori/openkite --json conclusion,jobs \
  --jq '.jobs[] | "\(.conclusion)\t\(.name)"'
```

`skipped` means "not verified". A green required-check set is not evidence that
`Release` succeeded. Failures on `main` also open a tracking issue via
[`main-failure-tracker.yml`](../.github/workflows/main-failure-tracker.yml); the
issue links the failing job and its logs.
