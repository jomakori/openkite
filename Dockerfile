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
      echo "The preview image serves the prebuilt web bundle from web/dist/."; \
      echo "Build it with web/build.sh (see web/README.md)."; \
      echo "Refusing to build a UI-less image."; \
      exit 1; \
    }

# ─────────────────────────────────────────────────────────────────────
# Stage 2 — runtime: static server for the validated bundle.
#
# Uses the UNPRIVILEGED nginx image (runs as uid 101, listens on 8080), so
# the preview pod never runs as root and never needs CAP_NET_BIND_SERVICE.
# 8080 is also what the openkite-preview chart targets
# (service.targetPort), so the two stay in agreement.
# ─────────────────────────────────────────────────────────────────────
FROM nginxinc/nginx-unprivileged:1.31-alpine AS runtime
COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=bundle /bundle/web/dist /usr/share/nginx/html

# The served root must be writable by the container user, because that is where a
# PR preview's Live Update sync lands and the sync writes as the user the
# container runs as (uid 101). The base image ships the directory root:root 755,
# where uid 101 gets EACCES on the first sync — verified on a live preview pod
# (`drwxr-xr-x root root /usr/share/nginx/html`, `touch: Permission denied`).
# chown needs root; the image drops back to nginx immediately.
USER root
RUN chown -R nginx:nginx /usr/share/nginx/html
USER nginx

EXPOSE 8080
