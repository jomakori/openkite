# The Tilt loops

**Previews prove the change, staging proves the build, prod proves the release.**

Three rungs, three different questions. A preview answers "does this branch render
what I think it renders" in about a minute, from a released image, with no image
build and no registry write. Staging answers "does the artifact we would ship
work" — the real image, the real migrations, the real dependencies. Prod answers
"does the release work for everyone". A preview is not a smaller staging, and
nothing here replaces the other two rungs.

```
Tiltfile                both loops (root, because Tilt requires it there)
web/build.sh            browser bundle -> web/dist (existing repo entry point)
```

There is no helper script and no Makefile. The Tiltfile is the configuration: the
chart comes from the GitOps repo, Tilt installs it, Tilt tears it down.

## The preview loop

Run by the preview runner, and by hand like this:

```sh
OPENKITE_PR=123 OPENKITE_DEV_LOOP=0 \
  KUBECONFIG=... tilt up                 # https://pr123-openkite.maklab.net
```

In CI the loop runs headless — `tilt ci` in place of `tilt up` — and the `preview`
label on the pull request is the whole contract:

- **Add the label.** The runner installs the chart at the PR's coordinates on the
  newest ancestor release tag and syncs the branch's `web/dist` in. Nothing is built
  and nothing is published: no image, no registry object, no Docker.
- **Remove the label, or close the PR.** The release is uninstalled and the
  environment deleted.
- **A failed deploy takes the label off**, so the label always means the environment
  exists.

The appset must not also generate an Application for a labelled PR: its applications
run with selfHeal, so ArgoCD would reconcile every sync away. One owner per
environment — here, Tilt.

What it does:

1. **Base selection.** `git tag --merged HEAD --sort=-v:refname --list 'v*'` — the
   newest release tag whose commit is an ancestor of the head, never simply the
   newest release, because standing a branch on main-after-it would preview code
   that never existed on that branch. No tag → the Tiltfile fails with the fix
   (`git fetch --tags`). Override with `OPENKITE_BASE_TAG`.
2. **Install.** `helm_resource` (`ext://helm_resource`) installs the GitOps app
   chart `apps/helm` — the same spec prod and staging render from — at `pr<N>`
   coordinates: namespace `openkite-pr<N>`, host `pr<N>-openkite.maklab.net`,
   image `ghcr.io/jomakori/openkite:<base>`, both environment doppler configs, the
   Cloudflare Access gate. Tilt runs `helm upgrade --install`, so the resources are
   first-class to Tilt: health, logs and port-forwards work, and `tilt down` is
   `helm uninstall`.
3. **Overlay.** `live_update` syncs `web/dist` into the served root.
   `initial_sync()` puts the whole bundle in place on pod start — the base image
   already carries a working copy, so the preview is a working site from the first
   second — and `sync()` keeps it current as `web/src` is edited. `openkite-web`
   reruns `web/build.sh` on change, so the overlay is the branch's real build.

`OPENKITE_CHART` (default `../gke_GitOps/apps/helm`) points at a local checkout of
the GitOps repo; the Tiltfile fails with that instruction when the path is absent.

The namespace is created by `--create-namespace` and, like any Helm-created
namespace, is not removed by `helm uninstall`. After `tilt down`, delete it with
`kubectl delete ns openkite-pr<N>`.

### The one prerequisite a preview image must satisfy

The container runs as uid 101 (nginx) and Live Update's sync writes into the
container as that user. The released image keeps `/usr/share/nginx/html`
`root:root 755`, where uid 101 cannot write — checked on a live preview pod:

```
$ kubectl -n openkite-pr141 exec pod/openkite-production-… -- sh -c 'id; ls -ld /usr/share/nginx/html'
uid=101(nginx) gid=101(nginx) groups=101(nginx)
drwxr-xr-x    1 root     root            32 Sep 26 19:33 /usr/share/nginx/html
$ … touch /usr/share/nginx/html/.probe
touch: /usr/share/nginx/html/.probe: Permission denied
```

So the `runtime` stage of the root `Dockerfile` chowns the served root to the
container user. Until a release carries that line, a preview's first sync fails
with a permission error; rebase onto a release that has it, or use staging for
that branch.

## The dev loop

```sh
tilt up            # cargo-check + openkite-host + dx serve
```

All three run on the host toolchain; the Tiltfile builds no image, so the dev
loop needs no Docker daemon.

- `cargo-check` runs `cargo check --workspace` on change.
- `openkite-host` runs the real server binary (`cargo run -p openkite-web`) on
  port 8090, reading the cluster through `KUBECONFIG`. It serves the console
  bundle *and* the bridge the console calls, so `http://localhost:8090` is the
  whole app locally rather than static assets. It restarts on change.
- `openkite-ui` is `cd crates/openkite-desktop && dx serve` — the Dioxus dev
  server with hot reload.
- The desktop crates link WebKitGTK and GTK, so the host needs the native packages
  CI installs (`.github/actions/rust-setup/action.yml`); the browser target
  (`npm run build:web`) needs none of them.

`OPENKITE_DEV_LOOP=0` skips all three; that is what the preview runner sets, since
`dx serve` needs a display.

## What a preview cannot prove

- **Rust that runs in the container.** `crates/openkite-web` is a real server
  process; a file sync moves files, not compiled behaviour. Today's released image
  is a static server with no host binary to replace, so the Tiltfile carries no
  sync step for it — a server-crate diff is proven by staging.
- **The image.** The preview explicitly does not build one: `Dockerfile` changes,
  package additions, and anything `docker build` does are staging's job.
- **Dependencies, migrations, new environment, the release pipeline.** Staging.

The full ladder and the reasoning behind each rung live in
`.hermes/plans/env-ladder-v3.md`.
