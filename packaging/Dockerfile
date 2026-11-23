# The place where the deb is made (scripts/build-deb.sh). Ubuntu 24.04, so that the programs
# ask for no newer glibc than 24.04 has (docs/history/research/distribution-formats.md 2-1).
FROM ubuntu:24.04

ARG RUST_VERSION=1.100.0
ARG NODE_VERSION=26.13.1
ARG NODE_SHA256=5c063e46bab0fbc1c17cc445d7ff129cbdcd0471be6ed83b6ab864f8143bc864

# The same packages as "Setup" in README.md, and what the deb bundler and the checks use.
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      build-essential pkg-config ca-certificates curl xz-utils file dpkg-dev \
      libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
      libasound2-dev libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev libudev-dev \
      liblilv-dev libdbus-1-dev \
 && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:/opt/node/bin:$PATH
RUN curl -sSfL -o /tmp/rustup-init https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init \
 && curl -sSfL -o /tmp/rustup-init.sha256 https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init.sha256 \
 && echo "$(cut -d' ' -f1 /tmp/rustup-init.sha256)  /tmp/rustup-init" | sha256sum -c - \
 && chmod +x /tmp/rustup-init \
 && /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain "$RUST_VERSION" \
 && rm /tmp/rustup-init /tmp/rustup-init.sha256 \
 && chmod -R a+rX /opt/rustup /opt/cargo

RUN curl -sSfL -o /tmp/node.tar.xz "https://nodejs.org/dist/v$NODE_VERSION/node-v$NODE_VERSION-linux-x64.tar.xz" \
 && echo "$NODE_SHA256  /tmp/node.tar.xz" | sha256sum -c - \
 && mkdir /opt/node && tar -xJf /tmp/node.tar.xz -C /opt/node --strip-components=1 \
 && rm /tmp/node.tar.xz
