#!/usr/bin/env bash
# Makes (or adds to) the apt repository of meloet: a tree of static files that can be served
# as it is (dists/, pool/, meloet.pgp). See docs/development/apt-repo.md.
#
#   MELOET_APT_KEY_ID=<key id> scripts/make-apt-repo.sh <deb> <output dir>
#
# reprepro runs in an Ubuntu 24.04 container. The same deb goes into two suites, noble
# (Ubuntu 24.04) and resolute (Ubuntu 26.04). The tree is made again from scratch each time,
# from the debs already in <output dir>/pool and the new one, so the output dir itself is the
# only state (a checkout of the published branch is enough).
#
# The reprepro of Ubuntu (5.3) keeps only one version of a package in a suite, so the suites
# list the newest version. The deb files of the 4 versions before it stay in pool/ (for a
# download by hand); older ones are removed.
#
# MELOET_APT_KEY_ID: the key that signs the Release files (required). It must sign without
# asking for a passphrase.
# GNUPGHOME: the GnuPG home that holds the key (default ~/.gnupg). It is copied to a
# temporary folder for the container and the copy is removed at the end; the original is
# not changed.
# MELOET_APT_IMAGE: the name of the image (default meloet-apt-repo).
# MELOET_DOCKER_SG=1: run docker through `sg docker` (as in scripts/build-deb.sh).
#
# Only dists/, pool/ and meloet.pgp in <output dir> are replaced; other files there
# (index.html, .nojekyll, CNAME …) are left as they are.
set -euo pipefail

keep=5

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

die() {
  echo "make-apt-repo: $*" >&2
  exit 1
}

[[ $# -eq 2 ]] || die "usage: MELOET_APT_KEY_ID=<key id> $0 <deb> <output dir>"
deb="$1"
out="$2"
[[ -f "$deb" ]] || die "no such deb: $deb"
[[ "$deb" == *.deb ]] || die "not a .deb file: $deb"
[[ -n "${MELOET_APT_KEY_ID:-}" ]] || die "MELOET_APT_KEY_ID is not set (the id of the signing key)"
gnupghome="${GNUPGHOME:-$HOME/.gnupg}"
[[ -d "$gnupghome" ]] || die "no GnuPG home at $gnupghome (set GNUPGHOME)"
image="${MELOET_APT_IMAGE:-meloet-apt-repo}"

mkdir -p "$out"
deb="$(cd "$(dirname "$deb")" && pwd)/$(basename "$deb")"
out="$(cd "$out" && pwd)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -m 700 "$tmp/gnupg"
mkdir "$tmp/new"
# Sockets and locks belong to the agent running outside; the container starts its own.
(cd "$gnupghome" && tar --exclude='./S.*' --exclude='*.lock' -cf - .) | tar -xf - -C "$tmp/gnupg"

docker build -q -t "$image" - <<'EOF' >/dev/null
FROM ubuntu:24.04
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends reprepro gnupg \
 && rm -rf /var/lib/apt/lists/*
EOF

# The script run in the container. /new.deb is the new deb, /old the output dir (read only),
# /new the new tree.
# shellcheck disable=SC2016
inner='
set -euo pipefail
die() { echo "make-apt-repo: $*" >&2; exit 1; }
gpg --batch --list-secret-keys "$KEY" >/dev/null 2>&1 \
  || die "no secret key \"$KEY\" in the GnuPG home"

field() { dpkg-deb -f "$1" "$2"; }
newpkg="$(field /new.deb Package)"
newver="$(field /new.deb Version)"
newarch="$(field /new.deb Architecture)"

# Lines of "package<TAB>version<TAB>path relative to /old (or the new deb)".
list=()
list+=("$newpkg	$newver	/new.deb")
if [[ -d /old/pool ]]; then
  while IFS= read -r -d "" f; do
    rel="${f#/old/}"
    p="$(field "$f" Package)"; v="$(field "$f" Version)"; a="$(field "$f" Architecture)"
    if [[ "$p" == "$newpkg" && "$v" == "$newver" && "$a" == "$newarch" ]]; then
      cmp -s "$f" /new.deb \
        || die "$rel is already in the tree as $p $v, with other content; raise the version"
      continue
    fi
    list+=("$p	$v	$rel")
  done < <(find /old/pool -type f -name "*.deb" -print0)
fi

# Newest first, by the rules of dpkg.
n=${#list[@]}
for ((i = 1; i < n; i++)); do
  for ((j = i; j > 0; j--)); do
    IFS="	" read -r _ a _ <<<"${list[j-1]}"
    IFS="	" read -r _ b _ <<<"${list[j]}"
    dpkg --compare-versions "$b" gt "$a" || break
    t="${list[j-1]}"; list[j-1]="${list[j]}"; list[j]="$t"
  done
done

mkdir -p /tmp/repo/conf
for suite in noble resolute; do
  printf "Origin: meloet\nLabel: meloet\nCodename: %s\nArchitectures: amd64\nComponents: main\nDescription: meloet\nSignWith: %s\n\n" \
    "$suite" "$KEY" >>/tmp/repo/conf/distributions
done

declare -A count
for line in "${list[@]}"; do
  IFS="	" read -r p v path <<<"$line"
  c=$((${count[$p]:-0} + 1))
  count[$p]=$c
  if ((c == 1)); then
    [[ "$p" != "$newpkg" || "$path" == /new.deb ]] \
      || die "$newpkg $newver is older than $v, which is already in the tree"
    src="$path"; [[ "$path" == /new.deb ]] || src="/old/$path"
    # The deb of Tauri has no Section or Priority, and reprepro needs them.
    sec="$(field "$src" Section)"; pri="$(field "$src" Priority)"
    for suite in noble resolute; do
      reprepro -b /tmp/repo --outdir /new --silent -S "${sec:-sound}" -P "${pri:-optional}" \
        includedeb "$suite" "$src"
    done
  elif ((c <= KEEP)); then
    mkdir -p "/new/$(dirname "$path")"
    cp "/old/$path" "/new/$path"
  else
    echo "make-apt-repo: removed $path" >&2
  fi
done

gpg --batch --export "$KEY" >/new/meloet.pgp
[[ -s /new/meloet.pgp ]] || die "could not export the public key \"$KEY\""
gpgconf --kill gpg-agent || true
'

docker run --rm --user "$(id -u):$(id -g)" \
  -v "$deb:/new.deb:ro" -v "$out:/old:ro" -v "$tmp/gnupg:/gnupg" -v "$tmp/new:/new" \
  -e GNUPGHOME=/gnupg -e HOME=/tmp -e KEY="$MELOET_APT_KEY_ID" -e KEEP="$keep" \
  "$image" bash -c "$inner"

rm -rf "$out/dists" "$out/pool" "$out/meloet.pgp"
mv "$tmp/new/dists" "$tmp/new/pool" "$tmp/new/meloet.pgp" "$out/"
find "$out/dists" "$out/pool" -type f -name '*.deb' -o -type f -name 'InRelease' | sort
