# Web bundle (`web/dist/`)

The per-PR preview image serves a **prebuilt static web bundle** from this
directory's `dist/` subfolder. The image recipe is the repo-root `Dockerfile`;
the build/push pipeline is `.github/workflows/pr-image.yml`.

## Dependency on OKT-73 (not yet landed)

The bundle is produced by the web target build, which is gated behind
**OKT-73 — "Feature-gate desktop-only code for web target"**. Until that
lands, `web/dist/` does not exist and the `pr-image` workflow **fails on
purpose** at its "Build web bundle" step, with a message pointing back here.
It refuses to publish an image with no UI behind it.

When OKT-73 lands, expose the build as an executable `web/build.sh`. The
workflow runs it when present, then requires `web/dist/index.html` to exist.
The build is expected to emit:

    web/dist/index.html   <- required; the workflow and Dockerfile both check it
    web/dist/...          <- hashed JS/CSS/assets

## Container

`Dockerfile` is multi-stage:

1. `bundle` — stages `web/dist/` and fails loudly if `index.html` is absent.
2. `runtime` — `nginxinc/nginx-unprivileged:1.31-alpine` serving the bundle on
   port **8080** (non-root, uid 101), with an SPA fallback (`web/nginx.conf`) so
   client-side routes survive a hard refresh. 8080 matches the `openkite-preview`
   chart's `service.targetPort`, so the image and chart stay in agreement.

## Local check

Without the bundle, the build is expected to fail:

    docker build .        # -> "web/dist/index.html is missing"

With a bundle in place:

    docker build -t openkite-web .
    docker run --rm -p 8080:8080 openkite-web
