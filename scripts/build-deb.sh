#!/usr/bin/env bash
# Makes the deb of meloet in an Ubuntu 24.04 container (packaging/Dockerfile) and puts it in
# target/deb/. Builds `-p meloet-app`, `-p meloet-plughost` and `-p meloet-mcp` only, with no
# features beyond the default ones.
#
#   scripts/build-deb.sh
#
# What it builds is the files git knows here (committed, changed, and new ones that are not
# ignored), copied to target/deb-work/src; ui/node_modules and target/ are not taken.
# target/deb-work keeps the container's cargo registry and target dir for the next time.
#
# MELOET_DEB_IMAGE: the name of the image (default meloet-deb-build).
# MELOET_DOCKER_SG=1: run docker through `sg docker` (a user who is in the docker group only
# for the moment).
set -euo pipefail

docker() {
  if [[ "${MELOET_DOCKER_SG:-}" == 1 ]]; then
    local args
    args="$(mktemp)"
    printf '%s\0' "$@" >"$args"
    sg docker -c "xargs -0 -a '$args' docker" || { rm -f "$args"; return 1; }
    rm -f "$args"
  else
    command docker "$@"
  fi
}

root="$(cd "$(dirname "$0")/.." && pwd)"
image="${MELOET_DEB_IMAGE:-meloet-deb-build}"
work="$root/target/deb-work"
out="$root/target/deb"

docker build -t "$image" "$root/packaging"

rm -rf "$work/src"
mkdir -p "$work/src" "$work/home" "$work/bin" "$out"
(cd "$root" && git ls-files -z -co --exclude-standard \
  | tar --null -T - --ignore-failed-read -cf -) | tar -xf - -C "$work/src"
rm -rf "$work/src/ui/node_modules" "$work/src/ui/dist"

docker run --rm --user "$(id -u):$(id -g)" \
  -v "$work:/work" \
  -e HOME=/work/home -e CARGO_HOME=/work/cargo -e CARGO_TARGET_DIR=/work/target \
  -e npm_config_cache=/work/npm-cache -e CARGO_PROFILE_RELEASE_STRIP=symbols \
  -w /work/src "$image" bash -euo pipefail -c '
    triple="$(rustc -vV | sed -n "s/^host: //p")"
    cargo fetch --locked
    # The license texts for crates that ship none come from the spdx crate (tools/licenses/inventory.mjs).
    mkdir -p /work/spdx/src && touch /work/spdx/src/lib.rs
    printf "[package]\nname = \"spdx-texts\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[dependencies]\nspdx = \"=0.13.4\"\n[workspace]\n" >/work/spdx/Cargo.toml
    cargo fetch --manifest-path /work/spdx/Cargo.toml
    (cd ui && npm ci --no-audit --no-fund && npm run build)
    cargo build --release --locked -p meloet-plughost
    cargo build --release --locked -p meloet-mcp
    cp /work/target/release/meloet-plughost "/work/bin/meloet-plughost-$triple"
    cp /work/target/release/meloet-mcp "/work/bin/meloet-mcp-$triple"
    rm -rf /work/target/release/bundle/deb
    cd crates/meloet-app
    ../../ui/node_modules/.bin/tauri build --bundles deb \
      --config "{\"bundle\":{\"externalBin\":[\"/work/bin/meloet-plughost\",\"/work/bin/meloet-mcp\"]}}" -- --locked
  '

rm -f "$out"/*.deb
cp "$work"/target/release/bundle/deb/*.deb "$out/"
ls -l "$out"/*.deb
