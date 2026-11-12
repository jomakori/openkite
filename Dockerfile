# syntax=docker/dockerfile:1

FROM rust:1.90-bookworm AS build
ARG OPENKITE_VERSION=dev
ENV OPENKITE_VERSION=${OPENKITE_VERSION}
WORKDIR /src
COPY . .
RUN cargo build --release -p openkite-web

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/openkite-web /usr/local/bin/openkite-web
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/openkite-web"]
