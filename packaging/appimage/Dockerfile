FROM docker.io/library/node:22-bookworm AS node
FROM docker.io/library/rust:1-bookworm

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates file gsettings-desktop-schemas libayatana-appindicator3-dev \
       libfuse2 libgdk-pixbuf2.0-bin libssl-dev libudev-dev libwebkit2gtk-4.1-dev \
       librsvg2-bin patchelf pkg-config wget \
    && rm -rf /var/lib/apt/lists/* \
    && npm install -g pnpm@10.24.0 @tauri-apps/cli@2.12.2

COPY build.sh /usr/local/bin/build-padsmith-appimage
ENTRYPOINT ["bash", "/usr/local/bin/build-padsmith-appimage"]
