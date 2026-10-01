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
RUN curl -fsSL \
  https://github.com/rustwasm/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/wasm-bindgen-${WASM_BINDGEN_VERSION}-x86_64-unknown-linux-musl.tar.gz \
  | tar -xz -C /usr/local/bin --strip-components=1 \
      wasm-bindgen-${WASM_BINDGEN_VERSION}-x86_64-unknown-linux-musl/wasm-bindgen

# ─────────────────────────────────────────────────────────────────────
# Stage 2 — builder: the SSR + hydrate host and the wasm client.
#
# The container image runs `openkite-web` (an axum process) which
# server-renders the console from `crates/openkite-web` and serves the
# wasm hydration bundle from `$OPENKITE_WEB_ROOT`. After OKT-137 the
# prebuilt React bundle is gone — the crate IS the UI.
# ─────────────────────────────────────────────────────────────────────
# Keep the pin at or above the workspace's minimum: kube 4.x (kube-runtime
# 4.2.0) declares edition 2024, which Cargo only accepts from 1.85 — a lower
# pin fails the build here with "feature `edition2024` is required" and
# publishes no image, leaving the cluster's tag unfetchable.
FROM rust:1.98-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY --from=wasm-bindgen /usr/local/bin/wasm-bindgen /usr/local/bin/wasm-bindgen

# Install the wasm32 target the hydration client compiles against.
RUN rustup target add wasm32-unknown-unknown

# Build the host binary first — its `default = ["ssr"]` feature pulls
# in dioxus-ssr, ciborium and base64.
RUN cargo build --release -p openkite-web --locked

# Build the hydration client. The `hydrate` feature pulls in
# dioxus-web + wasm-bindgen glue; the resulting wasm is what the SSR
# page boots in the browser.
RUN cargo build --release -p openkite-web-client --target wasm32-unknown-unknown --features hydrate --locked

# Stage the wasm-bindgen output the way the SSR HTML references it
# (`/openkite-web-client.js` plus its sibling `.wasm`).
RUN mkdir -p /bundle && \
    wasm-bindgen \
      --target web \
      --no-typescript \
      --out-dir /bundle \
      target/wasm32-unknown-unknown/release/openkite-web-client.wasm

# ─────────────────────────────────────────────────────────────────────
# Stage 3 — runtime: the SSR host and its wasm assets, listening on 8080.
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
EXPOSE 8080
USER nobody
ENTRYPOINT ["/usr/local/bin/openkite-web"]
