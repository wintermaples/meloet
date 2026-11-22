"""Split a built sound pack folder into one tar per instrument, a common tar and the catalog
(docs/architecture.md 23-1 and 23-9).

python3 -I tools/sounds/split.py <pack dir> --out <dir> --pack <version>
                                 [--previous <catalog.json>] [--catalog <path>]

<pack dir> is what tools/sounds/build.py wrote (pack.json, credits.json, licenses/, one folder per
instrument). Writes into <out>:
  <id>-<version>.tar     instrument.json and samples/*.flac of one instrument, at the top of the tar
  common-<version>.tar   credits.json and licenses/<id>/...
  catalog.json           {"format": "meloet-sounds-catalog", "version": 1, "pack": ..., "common": {...},
                          "instruments": [{id, version, file, url, bytes, sha256, credits}, ...]}
The version of a part is made from its content (the sha256 of every file in it), so a part that did
not change keeps its version, its file name and, with --previous, the url of the Release it was first
put in. The tars are plain ustar, not compressed (FLAC is compressed already); entries are sorted and
carry no owner or time, so the same folder gives the same bytes. build-log.json files are left out.
--catalog also writes the catalog there (tools/sounds/catalog.json is the one built into meloet).
"""

import argparse
import hashlib
import json
import os
import re
import tarfile

RELEASES = "https://github.com/wintermaples/meloet/releases/download/"
SKIP = {"build-log.json"}
VERSION_HEX = 16
PACK_RE = re.compile(r"^[0-9]{4}\.[0-9]{1,2}\.[0-9]{1,3}$")


def files_under(root):
    out = []
    for d, dirs, files in os.walk(root):
        dirs.sort()
        for f in sorted(files):
            if f in SKIP:
                continue
            full = os.path.join(d, f)
            if os.path.islink(full):
                raise SystemExit(f"symbolic link in the pack: {full}")
            out.append(os.path.relpath(full, root).replace(os.sep, "/"))
    return sorted(out)


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def content_version(root, rels):
    h = hashlib.sha256()
    for rel in rels:
        h.update(f"{rel}\0{sha256_of(os.path.join(root, rel))}\n".encode())
    return h.hexdigest()[:VERSION_HEX]


def clean(info):
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    info.mtime = 0
    info.mode = 0o755 if info.isdir() else 0o644
    return info


def write_tar(path, pieces):
    """pieces: (absolute path on disk, name in the tar), files only; their folders are added first."""
    dirs = set()
    for _, name in pieces:
        parts = name.split("/")[:-1]
        for i in range(1, len(parts) + 1):
            dirs.add("/".join(parts[:i]))
    with tarfile.open(path, "w", format=tarfile.USTAR_FORMAT) as tar:
        for d in sorted(dirs):
            info = tarfile.TarInfo(d)
            info.type = tarfile.DIRTYPE
            tar.addfile(clean(info))
        for full, name in sorted(pieces, key=lambda p: p[1]):
            tar.add(full, arcname=name, recursive=False, filter=clean)


def part(root, rels, part_id, out, release, previous):
    version = content_version(root, rels)
    file = f"{part_id}-{version}.tar"
    path = os.path.join(out, file)
    write_tar(path, [(os.path.join(root, rel), rel) for rel in rels])
    old = previous.get((part_id, version))
    entry = {
        "id": part_id,
        "version": version,
        "file": file,
        "url": old["url"] if old else release + file,
        "bytes": os.path.getsize(path),
        "sha256": sha256_of(path),
    }
    if old and (old["bytes"], old["sha256"]) != (entry["bytes"], entry["sha256"]):
        raise SystemExit(f"{file}: same version as in --previous, but other bytes")
    return entry


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pack_dir")
    ap.add_argument("--out", required=True)
    ap.add_argument("--pack", required=True, help="the pack's version, for example 2026.12.0")
    ap.add_argument("--previous", help="the catalog of the pack before: unchanged parts keep its url")
    ap.add_argument("--catalog", help="also write the catalog here")
    a = ap.parse_args()
    if not PACK_RE.match(a.pack):
        raise SystemExit(f"--pack {a.pack}: not like 2026.12.0")
    root = os.path.abspath(a.pack_dir)
    with open(os.path.join(root, "pack.json"), encoding="utf-8") as f:
        pack = json.load(f)
    previous = {}
    if a.previous:
        with open(a.previous, encoding="utf-8") as f:
            old = json.load(f)
        for e in [old["common"], *old["instruments"]]:
            previous[(e["id"], e["version"])] = e
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    release = f"{RELEASES}sounds-{a.pack}/"

    common_rels = ["credits.json"] + ["licenses/" + r for r in files_under(os.path.join(root, "licenses"))]
    common = part(root, common_rels, "common", out, release, previous)

    instruments = []
    for inst_id, json_rel in pack["instruments"].items():
        folder = os.path.dirname(json_rel)
        if not folder or "/" in folder:
            raise SystemExit(f"{inst_id}: {json_rel} is not <folder>/instrument.json")
        inst_root = os.path.join(root, folder)
        rels = files_under(inst_root)
        if os.path.basename(json_rel) != "instrument.json" or "instrument.json" not in rels:
            raise SystemExit(f"{inst_id}: no instrument.json")
        with open(os.path.join(inst_root, "instrument.json"), encoding="utf-8") as f:
            credits = json.load(f).get("credits", [])
        entry = part(inst_root, rels, inst_id, out, release, previous)
        entry["credits"] = credits
        instruments.append(entry)

    catalog = {
        "format": "meloet-sounds-catalog",
        "version": 1,
        "pack": a.pack,
        "common": common,
        "instruments": instruments,
    }
    text = json.dumps(catalog, ensure_ascii=False, indent=1) + "\n"
    targets = [os.path.join(out, "catalog.json")] + ([a.catalog] if a.catalog else [])
    for t in targets:
        with open(t, "w", encoding="utf-8") as f:
            f.write(text)
    total = common["bytes"] + sum(e["bytes"] for e in instruments)
    print(f"{len(instruments)} instruments + common, {total} bytes, pack {a.pack}: {', '.join(targets)}")


if __name__ == "__main__":
    main()
