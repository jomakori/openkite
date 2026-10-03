# syntax=docker/dockerfile:1

# ─────────────────────────────────────────────────────────────────────
# Stage 1 — wasm-bindgen: build the wasm hydration bundle the browser
# pulls in after the SSR pass paints the page.
#
# wasm-bindgen CLI is a fixed-binary release artifact; we curl a pinned
# release into the stage so the build does not depend on cargo install
# resolving on every release.
# ─────────────────────────────────────────────────────────────────────
FROM rust:1.98-bookworm AS wasm-bindgen
ARG WASM_BINDGEN_VERSION=0.2.127
# The CLI is a fixed-binary release per host architecture. This stage builds
# for the TARGET platform (linux/arm64 for this cluster), so the archive has to
# follow TARGETARCH — fetching the x86_64 archive into an arm64 image produces a
# binary the container cannot execute.
ARG TARGETARCH
RUN case "${TARGETARCH:-amd64}" in \
      amd64) wb_arch=x86_64 ;; \
      arm64) wb_arch=aarch64 ;; \
      *) echo "unsupported TARGETARCH: ${TARGETARCH:-<unset>}" >&2; exit 1 ;; \
    esac \
 && curl -fsSL \
  "https://github.com/rustwasm/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/wasm-bindgen-${WASM_BINDGEN_VERSION}-${wb_arch}-unknown-linux-musl.tar.gz" \
  | tar -xz -C /usr/local/bin --strip-components=1 \
      "wasm-bindgen-${WASM_BINDGEN_VERSION}-${wb_arch}-unknown-linux-musl/wasm-bindgen"

# ─────────────────────────────────────────────────────────────────────
# Stage 2 — manifests: the workspace manifests and nothing else.
#
# Only Cargo.toml files reach the builder, so the dependency compile
# below is keyed on Cargo.lock rather than on the sources. Docker's COPY
# cannot preserve directory structure from a glob (`COPY crates/*/ 
# Cargo.toml` flattens), so the pruning happens in a throwaway stage and
# the result is copied across — that COPY's cache key is the checksum of
# the manifests it copies, which is exactly the key we want.
# ─────────────────────────────────────────────────────────────────────
FROM alpine:3.24 AS manifests
WORKDIR /manifests
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN find crates -type f ! -name Cargo.toml -delete

# ─────────────────────────────────────────────────────────────────────
# Stage 3 — builder: the SSR + hydrate host and the wasm client.
#
# The container image runs `openkite-web` (an axum process) which
# server-renders the console from `crates/openkite-web` and serves the
# wasm hydration bundle from `$OPENKITE_WEB_ROOT`. After OKT-137 the
# prebuilt React bundle is gone — the crate IS the UI.
#
# Keep the toolchain at or above the workspace's minimum: kube 4.x
# (kube-runtime 4.2.0) declares edition 2024, which Cargo only accepts
# from 1.85. A lower pin fails the build with "feature `edition2024` is
# required" and publishes no image, which leaves every image pin in the
# cluster unfetchable.
# ─────────────────────────────────────────────────────────────────────
FROM rust:1.98-bookworm AS builder
WORKDIR /src
COPY --from=manifests /manifests/ ./
COPY --from=wasm-bindgen /usr/local/bin/wasm-bindgen /usr/local/bin/wasm-bindgen

# Install the wasm32 target the hydration client compiles against.
RUN rustup target add wasm32-unknown-unknown

# Compile the dependency graph against stub sources. This layer depends
# on the manifests only, so a source change reuses it and the step below
# recompiles just the workspace crates. `|| true`: the stubs do not
# satisfy every bin target and cargo stops once the graph it can build is
# built — the point is to pay for the ~600 external crates here.
RUN for d in crates/*/; do \
      mkdir -p "$d/src/bin"; \
      : > "$d/src/lib.rs"; \
      printf 'fn main() {}\n' > "$d/src/main.rs"; \
      printf 'fn main() {}\n' > "$d/src/bin/client.rs"; \
    done \
 && cargo build --release -p openkite-web --locked || true

# Real sources replace the stubs; only the workspace crates recompile.
COPY crates crates
RUN find crates -name '*.rs' -exec touch {} + \
 && cargo build --release -p openkite-web --locked

# Build the hydration client. The `hydrate` feature pulls in
# dioxus-web + wasm-bindgen glue; the resulting wasm is what the SSR
# page boots in the browser.
#
# `openkite-web-client` is a [[bin]] of openkite-web, not a package of its own,
# so `-p openkite-web-client` fails ("did not match any packages") and the target
# is named with --bin. `-p openkite-web` is not optional either: with no package
# selector cargo resolves the WHOLE virtual workspace for this target, which
# pulls the desktop stack in with it (dioxus -> dioxus-fullstack ->
# async-tungstenite -> tungstenite -> native-tls -> openssl-sys), and openssl-sys
# cannot build for wasm32 — pkg-config refuses to cross-compile it, and the
# failure lands 10 s in at:
#   "Could not find openssl via pkg-config: pkg-config has not been configured
#    to support cross-compilation".
RUN cargo build --release -p openkite-web --bin openkite-web-client \
      --target wasm32-unknown-unknown --features hydrate --locked

# Stage the wasm-bindgen output the way the SSR HTML references it
# (`/openkite-web-client.js` plus its sibling `.wasm`).
RUN mkdir -p /bundle && \
    wasm-bindgen \
      --target web \
      --no-typescript \
      --out-dir /bundle \
      target/wasm32-unknown-unknown/release/openkite-web-client.wasm

# ─────────────────────────────────────────────────────────────────────
# Stage 4 — runtime: the SSR host and its wasm assets, listening on 8080.
#
# Slim Debian image; nothing else needs to be in the container.
# 8080 matches the openkite-preview chart's service.targetPort.
# ─────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/openkite-web /usr/local/bin/openkite-web
COPY --from=builder /bundle /bundle
ENV OPENKITE_ADDR=0.0.0.0:8080
ENV OPENKITE_WEB_ROOT=/bundle
# The console document boots the hydration client from $OPENKITE_WEB_ROOT and
# names the two wasm-bindgen outputs below as it does. A stage that writes them
# anywhere else, or under another name, ships an image whose page loads no
# client and reports nothing. Assert the coupling here, so the build fails
# instead of the browser.
RUN test -f "${OPENKITE_WEB_ROOT}/openkite-web-client.js" \
 && test -f "${OPENKITE_WEB_ROOT}/openkite-web-client_bg.wasm" \
 || { \
      echo "ERROR: ${OPENKITE_WEB_ROOT} carries no hydration client to serve."; \
      echo "Expected openkite-web-client.js and openkite-web-client_bg.wasm there."; \
      echo "Refusing to build a console image that boots no client."; \
      exit 1; \
    }
EXPOSE 8080
USER nobody
ENTRYPOINT ["/usr/local/bin/openkite-web"]
