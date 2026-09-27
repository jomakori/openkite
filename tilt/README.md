# The Tilt loops _(tilt)_

Run the application on a workstation, or preview a pull request in the cluster.

## Overview

The Tiltfile at the repository root defines two loops, and it is the only
configuration either one needs: no helper script, no make target. Both install
the
app chart from the GitOps repository, which is the same spec production and
staging
render from, so a preview cannot drift from what production runs.

| Loop | Runs on | Starts from | Serves |
| --- | --- | --- | --- |
| preview | a CI runner or a workstation, joined to the cluster | the newest release the branch is based on | `pr<N>-openkite.maklab.net` |
| dev | a workstation | the local workspace | the console and its bridge on `localhost` |

The preview loop needs the Kubernetes API and nothing else: no image build, no
registry write, no Docker daemon.

## Usage

Preview the branch a checkout is on:

```sh
OPENKITE_PR=123 OPENKITE_DEV_LOOP=0 KUBECONFIG=... tilt up
```

In CI the same loop runs as `tilt ci`, which deploys, waits for readiness,
syncs, and
exits. The `preview` label on a pull request is what starts it, and removing the
label or closing the pull request uninstalls the environment. The label
contracts are
in [`CONTRIBUTING.md`](../CONTRIBUTING.md).

Run the loops that need a display, on a workstation:

```sh
tilt up
```

`OPENKITE_DEV_LOOP=0` omits them, which is what a runner sets.

## Reference

Base selection. `git tag --merged HEAD --sort=-v:refname --list 'v*' | head -1`
gives
the newest release tag whose commit is an ancestor of the head. Never the newest
release: standing a branch on a release that came after it would preview code
the
branch never had. `OPENKITE_BASE_TAG` overrides the result, and a checkout with
no tag
fails with the command that fixes it.

Install. `helm_resource` from `ext://helm_resource` runs `helm upgrade
--install` on
`apps/helm` from the GitOps repository, at `pr<N>` coordinates: the environment
`openkite-pr<N>`, the host `pr<N>-openkite.maklab.net`, the base release as the
image
tag, both Doppler configs, and the Cloudflare Access gate. Tilt owns the release
it
created, so health, logs, port-forwards, and `tilt down` work on it.

Overlay. The branch's browser bundle is synced into the served root, so a
reviewer
sees the change on top of the released build. The chart must be checked out next
to
this repository, at `OPENKITE_CHART` if it is elsewhere.

The chart values a preview passes are asserted against the rendered output
before the
install runs. Helm ignores a value key the chart revision does not know, and a
preview
that trusts its own flags can therefore install at production's coordinates.

The dev loop:

- `openkite-bundle` builds the browser bundle with the repository's own
  `web/build.sh`.
- `openkite-host` runs the server binary, `cargo run -p openkite-web`, which
  serves the
  console and answers the bridge the console calls. It reads the cluster through
  `KUBECONFIG` and listens on `8090`, leaving `8080` to `dx`.
- `openkite-ui` runs `dx serve`, the Dioxus dev server, for hot reload of the
  UI.
- `cargo-check` runs `cargo check --workspace` on change.

## Limits

A preview proves the branch's UI on a released base. It does not prove:

- the image, since none is built: `Dockerfile` changes, package additions, and
  anything `docker build` does belong to staging;
- Rust that runs in the container, since a file sync moves files and not
  compiled
  behaviour: `crates/openkite-web` is a server process, and today's released
  image
  carries no host binary to replace, so a server-crate change is proven by
  staging;
- dependencies, migrations, new environment values, and the release pipeline:
  staging.

A preview is not reconciled. Nothing re-applies the chart while the environment
lives,
and a container that restarts keeps the released bundle until the job runs
again,
because the synced files live in the container.

The dev loop's desktop crates link WebKitGTK and GTK, so a workstation running
`dx serve` needs the native packages CI installs
(`.github/actions/rust-setup/action.yml`). The browser target needs none of
them.

## Related

- The label contracts: [`CONTRIBUTING.md`](../CONTRIBUTING.md)
- The chart this installs: the `apps/helm` chart in the GitOps repository
- What CI does with these loops:
  [`.github/workflows/README.md`](../.github/workflows/README.md)
- The chart's own values: `apps/helm/README.md` in the GitOps repository
