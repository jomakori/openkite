# Contributing

## Dev environment

- Rust **stable**, edition 2021. The desktop build needs the platform webview
  stack — on Linux the same packages the `e2e` workflow installs
  (`libwebkit2gtk-4.1-dev`, GTK, `libxdo`), nothing extra on macOS, WebView2 on
  Windows.
- **The cluster comes from a kubeconfig, not from a local one.** Previews and
  the
  connected E2E job both reach a cluster this way, so local development uses the
  same path: join the tailnet (`tag:k8s`),
  `tailscale configure kubeconfig <proxy-host>`, then run the app against it —
  `cd crates/openkite-desktop && dx serve`. There is no local cluster to create,
  keep alive, or tear down.
- **Two loops, one Tiltfile.** `tilt up` is the dev loop: a Docker image with
  the
  toolchain and CI's system packages, `cargo check --workspace` on change, and
  `dx serve` for the UI. `OPENKITE_PR=<N> tilt up` with `OPENKITE_DEV_LOOP=0` is
  the preview loop, and it needs no Docker daemon: it stands on the newest
  release tag that is an ancestor of the branch and overlays this branch's built
  artifacts. Previews prove the change, staging proves the build, prod proves
  the
  release — see [`tilt/README.md`](tilt/README.md).
- **Do not build the desktop binary on a small review host**: the Dioxus link
  step needs more than ~2.5 GiB and will OOM a ~3 GiB container. Use CI, or the
  one-shot capture Job in `e2e/visual/cluster/`.
- CI gates (all four must pass): `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build --release`.

## Environments

Each environment answers one question, and together they cover a change end to end.

| Environment | Question | Reached by |
| --- | --- | --- |
| preview | does this change do what I think? | the `preview` label, at `pr<N>-openkite.maklab.net` |
| staging | does this build? | the `staging` label, at `staging-openkite.maklab.net` |
| prod | does this release? | a merge to `main`, at `openkite.maklab.net` |

**Preview** puts the branch's browser bundle on top of the newest release the branch is
based on, installed from the app chart in the GitOps repository. Removing the label, or
closing the pull request, tears it down. It proves the change and not the build: no
image is built, so `Dockerfile` edits, package additions, dependencies, migrations, and
Rust that runs in the container are staging's business. The loop is
[`tilt/README.md`](tilt/README.md).

**Staging** builds and publishes the branch as an image and deploys it, so it is the
environment that proves dependency bumps, native libraries, migrations, and anything a
file overlay cannot carry.

**Prod** follows `main`: a merge cuts a release, the released tag is pinned in the app
spec, and the environment follows the pin.

Labels are requests, and only a person applies them. CI removes one it cannot honour —
a failed preview deploy takes its label off — so a label never claims an environment
that is not there.

## Workflow

- Tickets live in **Plane** (`OKT-*` core, `OKA-*` plugins); every PR
  references its ticket.
- One ticket per PR. Move the ticket across the kanban as you go.
- Branch → PR → CI green → squash-merge. No direct pushes to main for ticket
  work.
- **Green required checks are not enough.** The heavy checks are activated by
  the diff rather than run on every pull request, and GitHub reports a job
  skipped by a job-level condition as `Success` — so a green tick on `Run
  tests` can mean the job never ran. Before presenting a PR, enumerate
  the workflows the diff can trigger (`lint-test`, `build-test-staging`, `preview`,
  `Release`, plus any path-filtered workflow whose paths the
  diff matches) and read each job's real conclusion — a `success` status can
  hide a skipped or unrun step. When a change needs the heavier proof, ask for
  it with the environment labels above: `staging` builds the branch into an
  image. After a merge to `main`, check the runs it
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
## Documentation

Two audiences, documented separately. The project README addresses someone
deciding
whether to use OpenKite; everything under this heading addresses someone
changing it.

The contributor documents:

| Document | Answers |
| --- | --- |
| `CONTRIBUTING.md` | how to work here: setup, the environment ladder, the label contracts, review rules |
| `tilt/README.md` | how the local and preview loops run: what each one proves, and what it cannot |
| `.github/workflows/README.md` | what CI does: the four workflows, their triggers, artifacts, and the checks a merge requires |
| `e2e/README.md` | the end-to-end suites, their layout, and how to run one |
| `web/README.md` | the UI build targets and what each one ships |

An area README follows one order:

1. A title and a one-sentence purpose.
2. `## Overview` — what it is, where it runs, what it talks to, and the source
   of truth.
3. `## Usage` — the tasks a reader performs, in the order they perform them.
4. `## Reference` — the exhaustive detail: flags, values, jobs, files.
5. `## Limits` — what it does not do, and the constraints a change has to
   respect.
6. `## Related` — links out. Every topic has one home; other documents link to
   it
   rather than restating it.

Rules that hold for every document here:

- No ticket identifiers and no references to a change, a date, or a one-off
  investigation. A document describes the system as it stands; work tracking is
  not a
  source for it.
- No plan documents. Plans record a decision at a moment; a README states the
  result.
- Precise language: no "simply", "easy", or "just", no "several" or "some" where
  a
  number is known, and no "etc." in a list that can be completed.
