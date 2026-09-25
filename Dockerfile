# syntax=docker/dockerfile:1

# ─────────────────────────────────────────────────────────────────────
# Stage 1 — bundle: stage the prebuilt web bundle; fail loudly if absent.
#
# The bundle is produced by web/build.sh, which the image pipeline
# (.github/workflows/build-image.yml — pr-image.yml and release.yml alike) runs
# before this build, not inside this image. Validating here too means a local
# `docker build .` cannot quietly publish an image with nothing to serve. See
# web/README.md.
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

# `ca-certificates` is not in trixie-slim and the host will not boot without a
# system root store: its rustls client aborts on an empty one, in-cluster or not
# (the API server is HTTPS). `/home/openkite` is where the host writes its own
# config file, and uid 65532 cannot create it under WORKDIR.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /home/openkite \
    && chown 65532:65532 /home/openkite

WORKDIR /app
COPY --from=bundle /bundle/web/dist /app/web/dist
# Refuse a runtime image that has no console in it.
RUN test -f /app/web/dist/index.html

ENV OPENKITE_WEB_ROOT=/app/web/dist \
    HOME=/home/openkite

USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/openkite-web"]
