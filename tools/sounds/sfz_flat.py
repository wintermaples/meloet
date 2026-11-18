"""Flatten an SFZ file into one JSON line per region.

python3 -I tools/sounds/sfz_flat.py <file.sfz> [--define NAME=VALUE ...] > regions.jsonl

Follows #include and #define (in file order, so a later #define changes the
lines after it), and merges <control>/<global>/<master>/<group>/<region>
opcodes the way SFZ v2 inherits them. Each line holds the region's opcodes
plus "_sample" (the sample path made absolute with default_path).
Only reads files; it understands the parts of SFZ that the sources in
tools/sounds/sources.json use, not all of SFZ.
"""

import json
import os
import re
import sys

LEVEL = {"global": 0, "master": 1, "group": 2, "region": 3}


def substitute(line, defines):
    if "$" not in line:
        return line
    names = sorted(defines, key=len, reverse=True)
    out = []
    i = 0
    while i < len(line):
        if line[i] == "$":
            for n in names:
                if line.startswith(n, i):
                    out.append(defines[n])
                    i += len(n)
                    break
            else:
                out.append("$")
                i += 1
        else:
            out.append(line[i])
            i += 1
    return "".join(out)


def strip_comment(line):
    i = line.find("//")
    return line if i < 0 else line[:i]


def expand(path, defines, out, base=None):
    # SFZ resolves every #include against the folder of the top .sfz file
    base = base or os.path.dirname(path)
    with open(path, encoding="utf-8", errors="replace") as f:
        text = f.read()
    for raw in text.splitlines():
        line = strip_comment(raw)
        # one physical line may carry several #define
        parts = re.split(r"(?=#define\s)|(?=#include\s)", line)
        for part in parts:
            p = part.strip()
            if not p:
                continue
            m = re.match(r'#define\s+(\$\w+)\s+(.*)$', p)
            if m:
                defines[m.group(1)] = substitute(m.group(2).strip(), defines)
                continue
            m = re.match(r'#include\s+"([^"]+)"', p)
            if m:
                inc = substitute(m.group(1), defines)
                expand(os.path.normpath(os.path.join(base, inc)), defines, out, base)
                continue
            out.append(substitute(p, defines))


OPCODE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)=")


def parse_opcodes(text):
    ops = {}
    ms = list(OPCODE.finditer(text))
    for i, m in enumerate(ms):
        end = ms[i + 1].start() if i + 1 < len(ms) else len(text)
        ops[m.group(1)] = text[m.end():end].strip()
    return ops


def regions(lines, sfz_dir):
    text = "\n".join(lines)
    tokens = re.split(r"<(\w+)>", text)
    scope = {"control": {}, "global": {}, "master": {}, "group": {}}
    region_ops = None
    result = []

    def flush():
        if region_ops is not None:
            r = {}
            for h in ("global", "master", "group"):
                r.update(scope[h])
            r.update(region_ops)
            dp = scope["control"].get("default_path", "").replace("\\", "/")
            if "sample" in r:
                r["_sample"] = os.path.normpath(os.path.join(sfz_dir, dp, r["sample"].replace("\\", "/")))
            result.append(r)

    for i in range(1, len(tokens), 2):
        header, body = tokens[i], tokens[i + 1]
        ops = parse_opcodes(body)
        flush()
        region_ops = None
        if header == "control":
            scope["control"].update(ops)
        elif header in LEVEL and header != "region":
            scope[header] = dict(ops)
            for lower in ("global", "master", "group"):
                if LEVEL[lower] > LEVEL[header]:
                    scope[lower] = {}
        elif header == "region":
            region_ops = ops
    flush()
    return result


def main():
    path = sys.argv[1]
    defines = {}
    args = sys.argv[2:]
    while args:
        if args[0] == "--define":
            k, v = args[1].split("=", 1)
            defines[k] = v
            args = args[2:]
        else:
            raise SystemExit(f"unknown argument {args[0]}")
    lines = []
    expand(path, defines, lines)
    for r in regions(lines, os.path.dirname(os.path.abspath(path))):
        print(json.dumps(r, ensure_ascii=False))


if __name__ == "__main__":
    main()
