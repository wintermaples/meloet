#!/usr/bin/env python3
"""Shoot the real app (release build) in each state, and put each shot next to its mock board.

usage: python3 -I tools/design/shoot_app.py [--no-build] [--compare-small] [STATE ...]

Needs a screen, a MIDI keyboard plugged in, and an audio output (on the agent PC:
`source ~/.config/meloet-agent/env.sh` and `PIPEWIRE_RUNTIME_DIR=/run/user/1000`).
Builds ui/ and the release app (CARGO_TARGET_DIR is honoured), then starts the app once per
state, window size and scale, each time with a new config dir. It drives the app with the
development variables (README) and with meloet-vkeys, and lets MELOET_SNAPSHOT write the PNG.
States that start from a remembered output copy settings.json from one first run that
confirmed the chooser.
Out: docs/screenshots/app/<state>-<w>x<h>-<scale>.png and docs/screenshots/compare/ (app on
the left, mock on the right, laid out as in shoot_ui.py). --compare-small makes compare sheets
only for 100 % (to keep the PNGs small).
"""
import json, os, shutil, subprocess, sys, tempfile, time

from PIL import Image, ImageDraw

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(root, "target")
target = os.path.join(root, target) if not os.path.isabs(target) else target
app_bin = os.path.join(target, "release", "meloet")
vkeys_bin = os.path.join(target, "release", "meloet-vkeys")
out = os.path.join(root, "docs", "screenshots")

SIZES = [(1280, 800), (1920, 1080)]
SCALES = [100, 150]
CHORD = ["60", "64", "67"]
QUIET = "25"


def band(name):
    return [(400, {"type": "toggleBand", "band": name})]


def played(lead_ms, count, interval_ms=250):
    return ["play", "--notes", "60,64,67,72", "--count", str(count), "--interval-ms", str(interval_ms),
            "--hold-ms", "150", "--velocity", QUIET, "--lead-ms", str(lead_ms)]


def held_chord(lead_ms, hold_ms):
    # One vkeys per note: `play` lets go of a note before it presses the next one.
    return [["play", "--notes", n, "--count", "1", "--hold-ms", str(hold_ms),
             "--velocity", QUIET, "--lead-ms", str(lead_ms)] for n in CHORD]


# board: mock to compare with. remembered: start with the output already chosen.
# only_virtual / accept_virtual: MELOET_ONLY_VIRTUAL / MELOET_ACCEPT_VIRTUAL.
# keys: meloet-vkeys runs started together with the app. snap: MELOET_SNAPSHOT_AFTER_MS.
STATES = {
    "flow1": dict(board="F-Connect1", only_virtual=True),
    "flow2": dict(board="F-Connect2", accept_virtual=True, keys=held_chord(1200, 3000), snap=2500),
    "flow3": dict(board="F-Connect3", accept_virtual=True,
                  commands=[(1500, {"type": "chooserConfirm"})],
                  keys=held_chord(2300, 2500), snap=3300),
    "empty": dict(board="F-Empty", remembered=True),
    "settings": dict(board="F-Settings", remembered=True,
                     commands=[(400, {"type": "openSettings", "advanced": False})]),
    "settingsAdv": dict(board="F-SettingsAdvFixed", remembered=True,
                        commands=[(400, {"type": "openSettings", "advanced": True})]),
    "pianoRoll": dict(board="A-PianoRoll", remembered=True, commands=band("pianoRoll")),
    "mixer": dict(board="A-Mixer", remembered=True, commands=band("mixer")),
    "history": dict(board="F-History", remembered=True, commands=band("history")),
    "shelf": dict(board="F-ShelfArrange", remembered=True, commands=band("shelf")),
    "params": dict(board="F-ParamFew", remembered=True, commands=band("params")),
    # Round 2. Times of commands and of the snapshot count from the page load (about 1.3 s after
    # the start); --lead-ms counts from the start.
    "flow4": dict(board="F-Connect4", remembered=True,
                  commands=[(200, {"type": "toggleMetronome"}), (400, {"type": "record"})], snap=1800),
    "recording": dict(board="F-ArrangeRec", remembered=True, only_virtual=True,
                      commands=[(400, {"type": "record"})], keys=[played(4500, 18)], snap=7000),
    "main": dict(board="Main", remembered=True, only_virtual=True,
                 commands=[(400, {"type": "record"}), (7500, {"type": "record"})],
                 keys=[played(4500, 10)], snap=9000),
    "takes": dict(board="F-Takes", remembered=True, only_virtual=True,
                  commands=[(400, {"type": "record"}), (6500, {"type": "record"}),
                            (7000, {"type": "goToStart"}), (7500, {"type": "record"}),
                            (14500, {"type": "record"})],
                  keys=[played(4500, 8), played(12000, 8, 230)], snap=16000),
    "plugIn": dict(remembered=True, only_virtual=True, keys=[["appear", "--seconds", "3"]], snap=1500),
    "plugOut": dict(remembered=True, only_virtual=True, keys=[["appear", "--seconds", "2"]], snap=4000),
}


def run(cmd, **kw):
    subprocess.run(cmd, check=True, **kw)


def build():
    env = dict(os.environ)
    env["PATH"] = os.path.expanduser("~/.local/node/bin") + os.pathsep + env.get("PATH", "")
    run(["npm", "run", "build"], cwd=os.path.join(root, "ui"), env=env, stdout=subprocess.DEVNULL)
    run(["cargo", "build", "--release", "-p", "meloet-app", "-p", "meloet-midi"], cwd=root, env=env)


def start_app(config, size, extra, log):
    env = dict(os.environ)
    env.pop("MELOET_ACCEPT_VIRTUAL", None)
    env.pop("MELOET_ONLY_VIRTUAL", None)
    env.pop("MELOET_VKEYS_NAME", None)
    env.update(MELOET_CONFIG_DIR=config, MELOET_WINDOW=f"{size[0]}x{size[1]}", **extra)
    return subprocess.Popen(["dbus-run-session", "--", app_bin], env=env,
                            stdout=log, stderr=subprocess.STDOUT)


def remembered_settings(work):
    """settings.json after the app itself confirmed the chooser (real keyboard, first output)."""
    config = os.path.join(work, "seed")
    os.makedirs(config)
    with open(os.path.join(work, "seed.log"), "wb") as log:
        start_app(config, SIZES[0], dict(
            MELOET_COMMANDS=json.dumps([{"afterMs": 1200, "command": {"type": "chooserConfirm"}}]),
            MELOET_EXIT_AFTER_MS="2500"), log).wait(timeout=60)
    path = os.path.join(config, "settings.json")
    if not os.path.exists(path):
        sys.exit(f"the first run wrote no settings.json (log: {work}/seed.log)")
    return path


def shoot(state, size, scale, settings, work, png):
    s = STATES[state]
    config = tempfile.mkdtemp(prefix="cfg-", dir=work)
    if s.get("remembered"):
        shutil.copy(settings, os.path.join(config, "settings.json"))
    commands = ([(100, {"type": "setScale", "percent": scale})] if scale != 100 else []) + s.get("commands", [])
    extra = dict(MELOET_SNAPSHOT=png, MELOET_SNAPSHOT_AFTER_MS=str(s.get("snap", 1500)),
                 MELOET_EXIT_AFTER_MS="20000")
    if commands:
        extra["MELOET_COMMANDS"] = json.dumps([{"afterMs": ms, "command": c} for ms, c in commands])
    if s.get("only_virtual"):
        extra["MELOET_ONLY_VIRTUAL"] = "1"
    if s.get("accept_virtual"):
        extra["MELOET_ACCEPT_VIRTUAL"] = "1"
    # Only these virtual keyboards count, so that other runs' virtual keyboards stay out.
    names = [f"meloet-vkeys-shoot-{os.getpid()}-{i}" for i in range(len(s.get("keys", [])))]
    if names:
        extra["MELOET_VKEYS_NAME"] = ",".join(names)
    if os.path.exists(png):
        os.remove(png)
    with open(png + ".log", "wb") as log:
        app = start_app(config, size, extra, log)
        keys = [subprocess.Popen([vkeys_bin, *k, "--name", n], stdout=log, stderr=subprocess.STDOUT)
                for k, n in zip(s.get("keys", []), names)]
        app.wait(timeout=60)
        for k in keys:
            k.wait(timeout=30)
    if not os.path.exists(png):
        raise RuntimeError(f"no PNG for {state} (log: {png}.log)")
    os.remove(png + ".log")
    time.sleep(0.3)


def sheet(shot, board_png, board, stem, dest):
    # The same layout as shoot_ui.py.
    left = Image.open(shot).convert("RGB")
    w, h = left.size
    right = Image.new("RGB", (w, h), (40, 0, 40))
    m = Image.open(board_png).convert("RGB")
    right.paste(m.crop((0, 0, min(w, m.width), min(h, m.height))), (0, 0))
    page = Image.new("RGB", (w * 2 + 8, h + 24), (255, 0, 255))
    page.paste(left, (0, 24))
    page.paste(right, (w + 8, 24))
    d = ImageDraw.Draw(page)
    d.text((4, 6), f"app  {stem}", fill=(0, 0, 0))
    d.text((w + 12, 6), f"mock  {board} (100 %)", fill=(0, 0, 0))
    page.save(dest, optimize=True)


def main():
    args = sys.argv[1:]
    no_build = "--no-build" in args
    small = "--compare-small" in args
    states = [a for a in args if not a.startswith("--")] or list(STATES)
    unknown = [s for s in states if s not in STATES]
    if unknown:
        sys.exit(f"unknown state: {' '.join(unknown)} (known: {' '.join(STATES)})")
    if not no_build:
        build()
    os.makedirs(os.path.join(out, "app"), exist_ok=True)
    os.makedirs(os.path.join(out, "compare"), exist_ok=True)
    work = tempfile.mkdtemp(prefix="meloet-shoot-")
    try:
        mocks = os.path.join(work, "mock")
        boards = sorted({STATES[s]["board"] for s in states if STATES[s].get("board")})
        if boards:
            run([sys.executable, "-I", os.path.join(root, "tools", "design", "render_mocks.py"), mocks, *boards],
                stdout=subprocess.DEVNULL)
        settings = remembered_settings(work) if any(STATES[s].get("remembered") for s in states) else None
        for state in states:
            for size in SIZES:
                for scale in SCALES:
                    stem = f"{state}-{size[0]}x{size[1]}-{scale}"
                    shot = os.path.join(out, "app", stem + ".png")
                    shoot(state, size, scale, settings, work, shot)
                    print(shot)
                    board = STATES[state].get("board")
                    compare = os.path.join(out, "compare", stem + ".png")
                    if board and not (small and scale != 100):
                        sheet(shot, os.path.join(mocks, board + ".png"), board, stem, compare)
                        print(compare)
                    elif os.path.exists(compare):
                        os.remove(compare)
    finally:
        shutil.rmtree(work, ignore_errors=True)


main()
