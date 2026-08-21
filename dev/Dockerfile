# Rust dev image for OpenKite: toolchain + Dioxus-desktop system deps.
#
# The Dioxus desktop stack (wry -> tao -> gtk/webkit) needs these system
# libraries at compile time, even for `cargo check`. Keep this in sync with
# the CI "Install system dependencies" step in .github/workflows/ci.yml.
FROM rust:1.98

RUN apt-get update && apt-get install -y --no-install-recommends \
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
