"""Put a built sound pack folder into one tar file and print its sha256.

python3 -I tools/sounds/pack.py <pack dir> [--out <dir>] [--manifest]

The tar is meloet-sounds-<version>.tar (version = "pack" in <pack dir>/pack.json) and holds
meloet-sounds-<version>/... . The files inside are FLAC, so the tar is not compressed again.
Entries are sorted and carry no owner or time, so the same folder gives the same bytes.
build-log.json files (the build's own notes) are left out.
--manifest writes version, sha256 and bytes into tools/sounds/pack.json (the url is kept).
"""

import argparse
import hashlib
import json
import os
import tarfile

HERE = os.path.dirname(os.path.abspath(__file__))
SKIP = {"build-log.json"}


def entries(root):
    out = []
    for d, dirs, files in os.walk(root):
        dirs.sort()
        rel = os.path.relpath(d, root)
        if rel != ".":
            out.append(rel)
        for f in sorted(files):
            if f not in SKIP:
                out.append(os.path.normpath(os.path.join(rel, f)))
    return out


def clean(info):
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    info.mtime = 0
    info.mode = 0o755 if info.isdir() else 0o644
    return info


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pack_dir")
    ap.add_argument("--out", default=".")
    ap.add_argument("--manifest", action="store_true")
    a = ap.parse_args()
    root = os.path.abspath(a.pack_dir)
    with open(os.path.join(root, "pack.json"), encoding="utf-8") as f:
        version = json.load(f)["pack"]
    top = f"meloet-sounds-{version}"
    os.makedirs(a.out, exist_ok=True)
    path = os.path.join(os.path.abspath(a.out), top + ".tar")
    with tarfile.open(path, "w", format=tarfile.PAX_FORMAT) as tar:
        tar.add(root, arcname=top, recursive=False, filter=clean)
        for rel in entries(root):
            if os.path.islink(os.path.join(root, rel)):
                raise SystemExit(f"symbolic link in the pack: {rel}")
            tar.add(os.path.join(root, rel), arcname=f"{top}/{rel}", recursive=False, filter=clean)
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    size = os.path.getsize(path)
    print(f"{h.hexdigest()}  {path}  ({size} bytes)")
    if a.manifest:
        mpath = os.path.join(HERE, "pack.json")
        with open(mpath, encoding="utf-8") as f:
            m = json.load(f)
        m.update(version=version, file=os.path.basename(path), sha256=h.hexdigest(), bytes=size)
        with open(mpath, "w", encoding="utf-8") as f:
            json.dump(m, f, ensure_ascii=False, indent=1)
            f.write("\n")


if __name__ == "__main__":
    main()
