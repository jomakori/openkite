# syntax=docker/dockerfile:1

# ─────────────────────────────────────────────────────────────────────
# Stage 1 — bundle: stage the prebuilt web bundle; fail loudly if absent.
#
# The bundle is produced by the web target build in the PR pipeline
# (.github/workflows/pr-image.yml), not inside this image. That build
# depends on OKT-73 (feature-gate desktop-only code for the web target);
# see web/README.md. Validating here too means a local `docker build .`
# cannot quietly publish an image with nothing to serve.
# ─────────────────────────────────────────────────────────────────────
FROM alpine:3.24 AS bundle
WORKDIR /bundle
COPY web/ ./web/
RUN test -f web/dist/index.html || { \
      echo "ERROR: web/dist/index.html is missing — there is no bundle to serve."; \
      echo "The preview image serves the prebuilt web bundle from web/dist/."; \
      echo "Building it depends on OKT-73 (feature-gate desktop-only code);"; \
      echo "see web/README.md. Refusing to build a UI-less image."; \
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
EXPOSE 8080
