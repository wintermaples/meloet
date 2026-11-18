"""Pick the sample files that some regions of an SFZ use.

python3 -I tools/sounds/sfz_select.py <file.sfz> --root <dir> [--where OP=V1,V2 ...] [--has-any OP1,OP2 ...]

Regions come from sfz_flat.py. --where keeps regions whose opcode OP is one
of the values; --has-any keeps regions that carry at least one of the opcodes.
Every flag must match. Prints each sample path once, relative to --root, so
the list can go to `git sparse-checkout add --stdin` (with a leading "/").
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sfz_flat  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("sfz")
    ap.add_argument("--root", required=True)
    ap.add_argument("--where", action="append", default=[])
    ap.add_argument("--has-any", action="append", default=[])
    a = ap.parse_args()
    where = []
    for w in a.where:
        k, v = w.split("=", 1)
        where.append((k, set(v.split(","))))
    has_any = [set(h.split(",")) for h in a.has_any]
    lines = []
    sfz_flat.expand(a.sfz, {}, lines)
    regions = sfz_flat.regions(lines, os.path.dirname(os.path.abspath(a.sfz)))
    root = os.path.abspath(a.root)
    seen = []
    for r in regions:
        path = r.get("_sample")
        if not path or os.path.basename(path).startswith("*"):
            continue
        if any(r.get(k) not in vs for k, vs in where):
            continue
        if any(not (hs & r.keys()) for hs in has_any):
            continue
        rel = os.path.relpath(path, root)
        if rel not in seen:
            seen.append(rel)
    for rel in seen:
        print(rel)


if __name__ == "__main__":
    main()
