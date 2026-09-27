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
