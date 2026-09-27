# The Tilt loops

Tilt runs two loops in this repository: a preview of a pull request, and the
developer loop on a workstation. Both are declared in the `Tiltfile` at the
repository root. There is no helper script and no Makefile: the `Tiltfile` is
the configuration, and the chart it installs lives in the GitOps repository.

```
Tiltfile                both loops (root, because Tilt requires it there)
web/build.sh            browser bundle -> web/dist
```

## The preview loop

A preview is an environment for one pull request, served at
`https://pr<N>-openkite.maklab.net`. It runs the released image and overlays the
pull request's own browser bundle, so it answers one question: does this branch
render what its author thinks it renders.

The `preview` label on a pull request is the whole contract.

- Adding the label deploys the environment from the release tag the branch is
  based on, and syncs the branch's `web/dist` into it.
- Removing the label, or closing the pull request, uninstalls the release and
  deletes the environment.
- A failed deploy removes the label, so the label always means the environment
  exists.

Run it by hand the same way:

```sh
OPENKITE_PR=123 OPENKITE_DEV_LOOP=0 \
  KUBECONFIG=... tilt up
```

Continuous integration runs `tilt ci` in place of `tilt up`: the same loop,
headless,
ending when the resources are healthy. The environment outlives the run.

What the loop does:

1. **Base selection.** `git tag --merged HEAD --sort=-v:refname --list 'v*'`
   picks
   the newest release tag whose commit is an ancestor of the head. An ancestor
   is
   required rather than the newest release, because a branch based on an older
   commit would otherwise preview code that never existed on it. Override with
   `OPENKITE_BASE_TAG`; without a tag the `Tiltfile` stops and asks for
   `git fetch --tags`.
2. **Install.** `helm_resource` installs the GitOps app chart `apps/helm`, the
   same
   chart that renders the staging and production environments, at the
   coordinates of
   this pull request: environment `openkite-pr<N>`, host
   `pr<N>-openkite.maklab.net`, image `ghcr.io/jomakori/openkite:<base>`, the
   Cloudflare Access gate. Because Tilt runs `helm upgrade --install`, the
   resources
   are first-class to Tilt: health, logs and port-forwards work, and `tilt down`
   is
   `helm uninstall`.
3. **Overlay.** `live_update` syncs `web/dist` into the directory the container
   serves. `initial_sync()` places the whole bundle when the container starts,
   so the
   environment serves a working site from the first second, and `sync()` keeps
   it
   current as the sources change; `openkite-web` rebuilds the bundle on change.

`OPENKITE_CHART` (default `../gke_GitOps/apps/helm`) points at a local checkout
of
the GitOps repository. The `Tiltfile` stops with that instruction when the path
is
absent.

The environment is created with `--create-namespace`, and `helm uninstall`
leaves a
namespace behind: delete it with `kubectl delete ns openkite-pr<N>`.

### One component owns an environment

The chart is the deployment in every rung, so a preview may not also be rendered
by
the ApplicationSet that serves the GitOps cluster: those applications reconcile
with
`selfHeal`, which would undo every file the preview syncs. A pull request
carries an
ArgoCD-managed environment or a Tilt-managed one, never both.

### What the image must allow

The container serves as uid 101 and Live Update writes as that user, so the
directory
the container serves has to be writable by it. The runtime stage of the root
`Dockerfile` chowns the served root to the container user for this reason; an
image
that does not leaves the first sync failing with a permission error.

## The dev loop

```sh
tilt up            # cargo-check + openkite-host + dx serve
```

All three resources run on the host toolchain. The `Tiltfile` builds no image,
so
the dev loop needs no Docker daemon and no registry.

- `cargo-check` runs `cargo check --workspace` on change.
- `openkite-host` runs the server binary, `cargo run -p openkite-web`, on port
  8090.
  It serves the console bundle and the bridge the console calls, reading the
  cluster
  through `KUBECONFIG`, so `http://localhost:8090` is the whole application on a
  workstation rather than a set of static assets. It restarts on change.
- `openkite-ui` runs `dx serve` in `crates/openkite-desktop`, the Dioxus dev
  server
  with hot reload.
- The desktop crates link WebKitGTK and GTK, so the workstation needs the native
  packages that `.github/actions/rust-setup/action.yml` installs. The browser
  target,
  `npm run build:web`, needs none of them.

`OPENKITE_DEV_LOOP=0` omits all three, which is what a runner sets: `dx serve`
needs
a display.

## What a preview cannot prove

A preview overlays files on the released image, so it cannot prove anything that
a
file cannot change.

- **Rust that runs in the container.** The released image serves static assets
  and
  carries no host binary to replace, so the `Tiltfile` has no sync step for the
  server crate. A change to it is proven by staging.
- **The image.** A preview builds none: `Dockerfile` changes, added packages and
  anything else `docker build` does belong to staging.
- **Dependencies, migrations, environment variables and the release pipeline.**
  Staging.
