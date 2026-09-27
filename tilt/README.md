# The Tilt loops

**Previews prove the change, staging proves the build, prod proves the release.**

Three rungs, three different questions. A preview answers "does this branch render
what I think it renders" in about a minute, from a released image, with no image
build and no registry write. Staging answers "does the artifact we would ship
work" — the real image, the real migrations, the real dependencies. Prod answers
"does the release work for everyone". A preview is not a smaller staging, and
nothing here replaces the other two rungs.

```
Tiltfile            both loops (root, because Tilt requires it there)
tilt/select-base.sh     which released tag a preview stands on          (no Docker)
tilt/preview-render.sh  the app spec at pr<N> coordinates               (no Docker)
tilt/preview-apply.sh   apply_cmd: render → kubectl apply → annotate    (no Docker)
tilt/preview-delete.sh  delete_cmd: tear the environment down           (no Docker)
tilt/build-bundle.sh    the artifacts the overlay syncs (web bundle)    (no Docker)
tilt/build-server.sh    compile-and-swap for Rust in the container       (no Docker)
tilt/dev/Dockerfile     dev image for the laptop loop                    (Docker)
```

## The preview loop

Run by the preview runner, and by hand like this:

```sh
tilt/build-bundle.sh                     # web/dist -> tilt/out/bundle
OPENKITE_PR=123 OPENKITE_DEV_LOOP=0 \
  KUBECONFIG=... tilt up                 # https://pr123-openkite.maklab.net
```

What it does:

1. **Base selection** (`tilt/select-base.sh`). The base is the newest release tag
   whose commit is an ancestor of the head — never simply the newest release,
   because standing a branch on main-after-it would preview code that never
   existed on that branch. A branch that predates every release tag falls back to
   the newest release, and says so on stderr. The chosen tag is recorded on the
   namespace as `openkite.maklab.net/preview-base`, so an operator reads what an
   environment stands on instead of inferring it from an image tag.
2. **Render** (`tilt/preview-render.sh`). The GitOps app chart (`apps/helm`, the
   spec prod renders from) at `pr<N>` coordinates: namespace `openkite-pr<N>`,
   host `pr<N>-openkite.maklab.net`, image `ghcr.io/jomakori/openkite:<base>`,
   Cloudflare Access gate on. Every coordinate is asserted against the rendered
   text: helm ignores a value key the chart revision does not know, so without
   those guards a stale chart would render this preview into
   `openkite-production`.
3. **Apply** (`tilt/preview-apply.sh`, the Tiltfile's `apply_cmd`).
   `kubectl apply` over the Kubernetes API. No Docker daemon, no image build, no
   registry write, no per-PR image to prune.
4. **Overlay** (`live_update`). `initial_sync()` puts the whole
   `tilt/out/bundle` into the served root on pod start — the base image already
   carries a working copy of the bundle, so the preview is a working site from
   the first second — and `sync()` keeps it current as the branch is edited.

`tilt down`, or removing the preview label, runs `delete_cmd`
(`tilt/preview-delete.sh`): the rendered objects go, then the namespace — but only
if it carries the base annotation, so `tilt down` cannot delete an environment
some other owner created.

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
tilt up            # dev image + cargo-check + dx serve
```

- `openkite-dev` (`docker_build`, `tilt/dev/Dockerfile`) carries stable Rust and
  the same system packages CI installs (`.github/actions/rust-setup/action.yml`);
  the workspace is bind-mounted at `/app`, so the image survives source edits.
- `cargo-check` runs `cargo check --workspace` inside that image, on change.
- `openkite-ui` is `cd crates/openkite-desktop && dx serve` — the Dioxus dev
  server with hot reload, on the host because it is a GUI.

`OPENKITE_DEV_LOOP=0` omits the two Docker-backed resources; that is what the
preview runner (which has no Docker daemon) sets, since a `docker_build` in the
Tiltfile is otherwise built by `tilt up`.

## What a preview cannot prove

- **Rust that runs in the container.** `crates/openkite-web` is a real server
  process; a file sync moves files, not compiled behaviour. The honest path is
  compile-and-swap (`tilt/build-server.sh`): compile in a temporary in-cluster
  build pod on the architecture that will run it, pull the binary out, sync it,
  `restart_container()`. Today's released image is a static server with no host
  binary to replace, so the Tiltfile carries no sync step for it yet — pretending
  otherwise would sync into a path that does not exist. Until that image ships,
  a server-crate diff is proven by staging.
- **The image.** The preview explicitly does not build one: `Dockerfile` changes,
  package additions, and anything `docker build` does are staging's job.
- **Dependencies, migrations, new environment, the release pipeline.** Staging.

The full ladder and the reasoning behind each rung live in
`.hermes/plans/env-ladder-tilt-staging-prod-v2.md`.
