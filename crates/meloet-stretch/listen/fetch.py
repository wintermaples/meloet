#!/usr/bin/env python3
"""Gets the voices for the listening set and cuts the parts we use.

    python3 -I crates/meloet-stretch/listen/fetch.py [--cache DIR] [--out DIR]

- Downloads every file of sources.json into the cache (default ~/.cache/meloet/listen-src) and
  checks its sha256. The voices never go into the repository.
- Cuts the part named there with ffmpeg: mono, 48 kHz, 24 bit, into <cache>/wav/<id>.wav.
- Writes README.md (where every voice comes from, license, credit) and copies the note for the
  owner into the output folder (default target/listen/round7-stretch).

Then: cargo run -p meloet-stretch --release --example listen -- <cache>/wav <out>
"""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
AGENT = "audio-test-fetch/1.0"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def readme(sources):
    lines = [
        "# round7-stretch: 聞きくらべの音",
        "",
        "のび縮みと音の高さの部品（Signalsmith Stretch 1.4.0）で作った音です。",
        "何を聞いてほしいかは `聞いてほしい所.md` に書きました。",
        "",
        "作りなおす:",
        "",
        "```bash",
        "python3 -I crates/meloet-stretch/listen/fetch.py",
        "cargo run -p meloet-stretch --release --example listen -- ~/.cache/meloet/listen-src/wav target/listen/round7-stretch",
        "```",
        "",
        "## 声の素材（リポジトリには入れていません）",
        "",
        "どれも CC0 ・ パブリックドメイン ・ CC BY です（CC BY-SA は使っていません）。",
        "meloet がした事: 下の秒の範囲を切りだし、モノラル ・ 48 kHz にして、のび縮みと高さを変えました。",
        "",
    ]
    for s in sources:
        lines += [
            f"### {s['id']}（{s['kind']}）",
            "",
            f"- 題: {s['title']}",
            f"- 作者 ・ 帰属: {s['credit']}",
            f"- ページ: {s['page']}",
            f"- ファイル: {s['url']}",
            f"- sha256: `{s['sha256']}`" + (f"（はじめの {s['bytes']} バイト）" if s.get("bytes") else ""),
            f"- ライセンス: {s['license']}（{s['license_uri']}）",
            f"- 使った所: {s['start']} 秒から {s['seconds']} 秒ぶん",
        ]
        if s.get("note"):
            lines.append(f"- メモ: {s['note']}")
        lines.append("")
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache", default=str(pathlib.Path.home() / ".cache" / "meloet" / "listen-src"))
    ap.add_argument("--out", default=str(ROOT / "target" / "listen" / "round7-stretch"))
    args = ap.parse_args()
    cache, out = pathlib.Path(args.cache), pathlib.Path(args.out)
    (cache / "wav").mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    sources = json.loads((HERE / "sources.json").read_text(encoding="utf-8"))["sources"]
    for s in sources:
        raw = cache / s["file"]
        if not raw.exists():
            print(f"get {s['url']}")
            headers = {"User-Agent": AGENT}
            if s.get("bytes"):
                # The big WAV files come slowly; the first part is enough.
                headers["Range"] = f"bytes=0-{s['bytes'] - 1}"
            req = urllib.request.Request(s["url"], headers=headers)
            with urllib.request.urlopen(req) as r, open(raw, "wb") as f:
                shutil.copyfileobj(r, f)
        got = sha256(raw)
        if got != s["sha256"]:
            sys.exit(f"{raw}: sha256 is {got}, sources.json says {s['sha256']}")
        wav = cache / "wav" / f"{s['id']}.wav"
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-ss", str(s["start"]), "-t", str(s["seconds"]), "-i", str(raw),
             "-ac", "1", "-ar", "48000", "-c:a", "pcm_s24le", str(wav)],
            check=True,
        )
        print(f"ok  {wav}")
    (out / "README.md").write_text(readme(sources), encoding="utf-8")
    shutil.copyfile(HERE / "聞いてほしい所.md", out / "聞いてほしい所.md")
    print(f"wrote {out / 'README.md'}")


if __name__ == "__main__":
    main()
