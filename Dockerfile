# syntax=docker/dockerfile:1

# ─────────────────────────────────────────────────────────────────────
# Stage 1 — bundle: stage the prebuilt web bundle; fail loudly if absent.
#
# The bundle is produced by web/build.sh, which the PR pipeline
# (.github/workflows/pr-image.yml) runs before this build, not inside this
# image. Validating here too means a local `docker build .` cannot quietly
# publish an image with nothing to serve. See web/README.md.
# ─────────────────────────────────────────────────────────────────────
FROM alpine:3.24 AS bundle
WORKDIR /bundle
COPY web/ ./web/
RUN test -f web/dist/index.html || { \
      echo "ERROR: web/dist/index.html is missing — there is no bundle to serve."; \
      echo "The image serves the prebuilt web bundle from web/dist/."; \
      echo "Build it with web/build.sh (see web/README.md)."; \
      echo "Refusing to build a UI-less image."; \
      exit 1; \
    }

# ─────────────────────────────────────────────────────────────────────
# Stage 2 — runtime: the console host (crates/openkite-web) serving the
# bundle from disk on :8080, as an unprivileged numeric uid.
# ─────────────────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

# The host binary, compiled by the image workflow (build-image.yml).
COPY target/release/openkite-web /usr/local/bin/openkite-web

# A writable HOME for the host's own config file.
RUN mkdir -p /home/openkite && chown 65532:65532 /home/openkite

WORKDIR /app
COPY --from=bundle /bundle/web/dist /app/web/dist
# Refuse a runtime image that has no console in it.
RUN test -f /app/web/dist/index.html

ENV OPENKITE_WEB_ROOT=/app/web/dist \
    HOME=/home/openkite

USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/openkite-web"]
