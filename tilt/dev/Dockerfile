# syntax=docker/dockerfile:1
#
# Dev image for the laptop loop (the root Tiltfile's `docker_build`). It carries
# the toolchain and the system libraries a Rust build of this workspace needs, so
# a laptop iterates with `cargo check`/`cargo build` in a container instead of
# installing the webview stack on the host.
#
# The system package list is the SAME list CI installs
# (.github/actions/rust-setup/action.yml). Keep the two in step: a dependency
# that CI has and this image lacks is a check that passes here and fails in CI,
# which is the failure mode this image exists to prevent.
#
# The workspace is bind-mounted at /app by the Tiltfile (with named volumes for
# target/ and the cargo registry); nothing is COPYed in, so an edit to the source
# does not invalidate the image.
FROM rust:1-bookworm

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev \
      libgtk-3-dev \
      libglib2.0-dev \
      libayatana-appindicator3-dev \
      librsvg2-dev \
      libxdo-dev \
      libssl-dev \
      pkg-config \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
