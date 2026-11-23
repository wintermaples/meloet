#!/usr/bin/env bash
# Makes the AppImage of meloet (a trial form, docs/development/appimage.md) in the same Ubuntu
# 24.04 container as the deb (packaging/Dockerfile) and puts it in target/appimage/. Builds
# `-p meloet-app`, `-p meloet-plughost` and `-p meloet-mcp` only, with no features beyond the
# default ones.
#
#   scripts/build-appimage.sh
#
# What it builds is the files git knows here, as for scripts/build-deb.sh. target/appimage-work
# keeps the container's cargo registry, target dir and the tools below for the next time.
#
# The AppImage bundler of tauri-cli (tauri-bundler 2.10.6 in @tauri-apps/cli 2.12.6,
# src/bundle/linux/appimage/linuxdeploy.rs) downloads its tools into ~/.cache/tauri when they
# are not there, partly from "continuous" releases, and checks nothing. They are downloaded
# here instead, each pinned by its sha256, and put where the bundler looks first. The AppImage
# runtime is pinned the same way (appimagetool would download the newest "continuous" one).
#
# MELOET_DEB_IMAGE: the name of the image (default meloet-deb-build, the deb's).
# MELOET_DOCKER_SG=1: run docker through `sg docker` (as for scripts/build-deb.sh).
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
work="$root/target/appimage-work"
out="$root/target/appimage"

docker build -t "$image" "$root/packaging"

rm -rf "$work/src"
mkdir -p "$work/src" "$work/home" "$work/bin" "$work/tools" "$out"
(cd "$root" && git ls-files -z -co --exclude-standard \
  | tar --null -T - --ignore-failed-read -cf -) | tar -xf - -C "$work/src"
rm -rf "$work/src/ui/node_modules" "$work/src/ui/dist"

docker run --rm --user "$(id -u):$(id -g)" \
  -v "$work:/work" \
  -e HOME=/work/home -e CARGO_HOME=/work/cargo -e CARGO_TARGET_DIR=/work/target \
  -e npm_config_cache=/work/npm-cache -e CARGO_PROFILE_RELEASE_STRIP=symbols \
  -w /work/src "$image" bash -euo pipefail -c '
    # name, sha256, url. AppRun and linuxdeploy are the ones tauri-bundler 2.10.6 asks for
    # (MIT, AppImageKit and linuxdeploy). The appimage plugin (MIT) carries appimagetool and
    # mksquashfs; they only build. The runtime (MIT, with libfuse 3.15.0 LGPL-2.1,
    # squashfuse 0.5.2 BSD-2-Clause, zstd, zlib, mimalloc and musl linked in) is the head of
    # the AppImage file.
    tools=(
      "AppRun-x86_64 f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64"
      "linuxdeploy-07333c6-x86_64.AppImage 36a2d7e274d12e1050d0e9ecfe11d339ed54720b2bec464c286d53f8b07f5c62 https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy-07333c6/linuxdeploy-x86_64.AppImage"
      "linuxdeploy-plugin-appimage.AppImage 992d502a248e14ab185448ddf6f6e7d25558cb84d4623c354c3af350c25fccb3 https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/1-alpha-20250213-1/linuxdeploy-plugin-appimage-x86_64.AppImage"
      "runtime-x86_64 2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
    )
    mkdir -p /work/home/.cache/tauri
    for tool in "${tools[@]}"; do
      read -r name sum url <<<"$tool"
      if ! echo "$sum  /work/tools/$name" | sha256sum -c --status - 2>/dev/null; then
        curl -sSfL -o "/work/tools/$name" "$url"
        echo "$sum  /work/tools/$name" | sha256sum -c -
      fi
      # The bundler writes into its copy of linuxdeploy (tauri-bundler, prepare_tools), so the
      # copies are made again each time from the checked files.
      install -m 755 "/work/tools/$name" "/work/home/.cache/tauri/$name"
    done
    export LDAI_RUNTIME_FILE=/work/home/.cache/tauri/runtime-x86_64
    # The gtk plugin copies with `cp --archive`; keeping the owner (root) fails on the
    # mounted /work for a user who is not root (EOVERFLOW, which cp does not pass over).
    mkdir -p /work/shim
    printf "#!/bin/sh\nexec /usr/bin/cp \"\$@\" --no-preserve=ownership\n" >/work/shim/cp
    chmod 755 /work/shim/cp
    # libjbig is GPL-2.0-or-later; only the TIFF image loader needs it, and it takes the PC copy.
    export LINUXDEPLOY_EXCLUDED_LIBRARIES="libjbig.so*"

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
    rm -rf /work/target/release/bundle/appimage
    cd crates/meloet-app
    PATH="/work/shim:$PATH" ../../ui/node_modules/.bin/tauri build --verbose --bundles appimage \
      --config ../../packaging/appimage/tauri.appimage.json \
      --config "{\"bundle\":{\"externalBin\":[\"/work/bin/meloet-plughost\",\"/work/bin/meloet-mcp\"]}}" -- --locked
  '

appdir="$work/target/release/bundle/appimage/meloet.AppDir"
if compgen -G "$appdir/usr/lib/libjbig.so*" >/dev/null; then
  echo "libjbig (GPL) is inside the AppImage" >&2
  exit 1
fi
rm -f "$out"/*.AppImage
cp "$work"/target/release/bundle/appimage/*.AppImage "$out/"
ls -l "$out"/*.AppImage
