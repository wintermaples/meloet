# The apt repository

With an apt repository, people who use meloet get new versions with `sudo apt update && sudo apt upgrade`, like any other package. The repository is a tree of static files signed with our key, made by `scripts/make-apt-repo.sh` and served from GitHub Pages.

The deb does **not** add apt settings by itself: it has no `postinst` and writes nothing to `/etc/apt/`. People who want the repository add it by hand (see [What a user writes](#what-a-user-writes)). People who install the deb from the Releases page get no updates from apt.

## The tree

```
dists/noble/      Ubuntu 24.04: InRelease, Release, Release.gpg, main/binary-amd64/Packages(.gz)
dists/resolute/   Ubuntu 26.04: the same
pool/main/m/meloet/meloet_X.Y.Z_amd64.deb
meloet.pgp        the public key (binary), for /etc/apt/keyrings/meloet.pgp
```

- The same deb is in both suites (it is built on Ubuntu 24.04 and runs on 26.04).
- Only amd64.
- The suites list only the newest version: the reprepro of Ubuntu (5.3) keeps one version of a package in a suite. The deb files of the 4 versions before it stay in `pool/` for a download by hand; older ones are removed, so the tree stays far under the 1 GB limit of GitHub Pages (about 20 MB per version).

## Make the key (once)

Only the owner does this, on their own PC. The key signs the Release files; apt on users' PCs checks that signature.

Keep the key in a GnuPG home of its own, used only for the repository. The script runs gpg in a container where nothing can ask for a passphrase, so the key has none; protect the folder instead (it stays on your PC, and is never put in a git repository).

```bash
export GNUPGHOME="$HOME/.local/share/meloet-apt-gnupg"
install -d -m 700 "$GNUPGHOME"
gpg --batch --passphrase '' --quick-gen-key 'meloet apt repository <your address>' ed25519 sign never
gpg --list-secret-keys --with-colons | awk -F: '/^fpr/ { print $10; exit }'   # the fingerprint
```

- Use the fingerprint (40 hex digits) as `MELOET_APT_KEY_ID`.
- gpg also writes a revocation certificate to `$GNUPGHOME/openpgp-revocs.d/<fingerprint>.rev`. Copy it and the whole `$GNUPGHOME` to a safe place (for example a USB stick kept offline). Without the key, no new version can be published in the same repository; users would have to get a new key by hand.
- The key does not expire (`never`), so users never have to fetch it again. If it is ever stolen, publish the revocation and a new key.

## Make or update the tree

```bash
GNUPGHOME="$HOME/.local/share/meloet-apt-gnupg" MELOET_APT_KEY_ID=<fingerprint> \
  MELOET_DOCKER_SG=1 scripts/make-apt-repo.sh target/deb/meloet_X.Y.Z_amd64.deb <output dir>
```

- Needs docker. `MELOET_DOCKER_SG=1` runs docker through `sg docker`, as in `scripts/build-deb.sh`; leave it out if you can run `docker` directly. The image (`meloet-apt-repo`, Ubuntu 24.04 with reprepro and gnupg) is built the first time.
- `GNUPGHOME` (default `~/.gnupg`) is copied to a temporary folder for the container, and the copy is removed at the end. The original is not changed.
- `<output dir>` is the tree. When it already has a tree, the script reads the debs in its `pool/`, adds the new deb, and makes `dists/`, `pool/` and `meloet.pgp` again from them (the output dir is the only state; a checkout of the published branch is enough). Other files there (`.nojekyll`, `index.html`, `CNAME` …) are left as they are.
- The script stops with an error when:
  - an argument, `MELOET_APT_KEY_ID` or the GnuPG home is missing, or the key is not in the GnuPG home;
  - the tree already has the same version with other content (raise the version; apt would refuse a file whose hash changed);
  - the new deb is older than the newest one in the tree.
- Running it again with the same deb changes nothing but the dates and signatures.

To try a tree before publishing, serve it by `file://` in a container:

```bash
docker run --rm -v "$PWD/<output dir>:/repo:ro" ubuntu:26.04 bash -c '
  install -d /etc/apt/keyrings && cp /repo/meloet.pgp /etc/apt/keyrings/meloet.pgp
  printf "Types: deb\nURIs: file:///repo\nSuites: resolute\nComponents: main\nSigned-By: /etc/apt/keyrings/meloet.pgp\n" \
    >/etc/apt/sources.list.d/meloet.sources
  apt-get update && apt-get install -y meloet'
```

(Use `ubuntu:24.04` and `Suites: noble` for 24.04.)

## Put the tree on GitHub Pages

The tree lives on a branch of its own, `gh-pages`, with one commit only: old debs would otherwise stay in the git history and the repository would keep growing.

The first time:

```bash
git worktree add --orphan -b gh-pages ../meloet-apt
touch ../meloet-apt/.nojekyll
```

Then, on GitHub: Settings → Pages → "Deploy from a branch", branch `gh-pages`, folder `/ (root)`. The address is `https://<user>.github.io/<repository>/`.

Each release:

```bash
git -C ../meloet-apt fetch origin gh-pages && git -C ../meloet-apt reset --hard origin/gh-pages   # not the first time
GNUPGHOME=… MELOET_APT_KEY_ID=… MELOET_DOCKER_SG=1 \
  scripts/make-apt-repo.sh target/deb/meloet_X.Y.Z_amd64.deb ../meloet-apt
cd ../meloet-apt
git add -A
git commit --amend -m 'apt repository'   # the first time: git commit -m 'apt repository'
git push --force origin gh-pages
```

- `.nojekyll` keeps GitHub from running Jekyll on the tree.
- A GitHub Pages site can hold up to 1 GB.
- After the push, wait until the Pages deployment ends, then try it as a user would (below).

## What a user writes

Two files: the key at `/etc/apt/keyrings/meloet.pgp`, and a deb822 `.sources` file. `Signed-By` makes apt trust this key for this repository only.

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://<user>.github.io/<repository>/meloet.pgp | sudo tee /etc/apt/keyrings/meloet.pgp >/dev/null
sudo tee /etc/apt/sources.list.d/meloet.sources >/dev/null <<EOF
Types: deb
URIs: https://<user>.github.io/<repository>/
Suites: $(. /etc/os-release && echo "${UBUNTU_CODENAME:-$VERSION_CODENAME}")
Components: main
Architectures: amd64
Signed-By: /etc/apt/keyrings/meloet.pgp
EOF
sudo apt update
sudo apt install meloet
```

- `Suites` becomes `noble` on Ubuntu 24.04 and `resolute` on 26.04 (`UBUNTU_CODENAME` first, for systems built on Ubuntu). Other versions have no suite and `apt update` reports an error for this repository.
- `gpg --show-keys /etc/apt/keyrings/meloet.pgp` shows the fingerprint, to compare with the one in the README.
- To stop using the repository: `sudo rm /etc/apt/sources.list.d/meloet.sources /etc/apt/keyrings/meloet.pgp`.

## Draft for the READMEs

The owner adds this after the repository is published, with the real address and fingerprint. Not in the READMEs yet.

`README.md`, under "Install":

> ### Install with apt (updates come with the system)
>
> Type these lines in a terminal once. After that, `sudo apt update && sudo apt upgrade` also brings new versions of meloet.
>
> *(the commands of [What a user writes](#what-a-user-writes))*
>
> The key's fingerprint is `<fingerprint>`. To stop: `sudo rm /etc/apt/sources.list.d/meloet.sources /etc/apt/keyrings/meloet.pgp`.

`README.ja.md`, under 「入れかた」:

> ### apt で入れる（新しい版がシステムの更新といっしょに来ます）
>
> 端末で一度だけ、次を打ちます。そのあとは `sudo apt update && sudo apt upgrade` で meloet の新しい版も入ります。
>
> *（[What a user writes](#what-a-user-writes) のコマンド）*
>
> 鍵の指紋は `<fingerprint>` です。やめるときは `sudo rm /etc/apt/sources.list.d/meloet.sources /etc/apt/keyrings/meloet.pgp`。
