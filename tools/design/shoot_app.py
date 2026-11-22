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
Every start gets its own XDG_CACHE_HOME, so that the plugin scan is the same each time: a
state with scan="first" an empty one (the scan runs from the beginning, with its band); the
others share <target>/shoot/cache, which holds the record of one whole scan (made by a run of
its own when it is not there yet), so nothing of the scan shows.
Every start also gets its own MELOET_SF2_DIR: empty, or for a state with sf2=True
<target>/shoot/sf2 with a copy of this PC's FluidR3_GM.sf2. The states of round 5 step 2 need
Surge XT and ZamComp (VST3) among the plugins of this PC; those of step 3 also Dragonfly Hall
Reverb and abGate (LV2), and DISPLAY (an X screen) for the plugin windows, which come up for real.
Out: docs/screenshots/app/<state>-<w>x<h>-<scale>.png and docs/screenshots/compare/ (app on
the left, mock on the right, laid out as in shoot_ui.py). --compare-small makes compare sheets
only for 100 % (to keep the PNGs small). --only 1280x800-100 shoots that one size and scale.
"""
import hashlib, json, math, os, random, shutil, signal, struct, subprocess, sys, tempfile, time, wave

from PIL import Image, ImageDraw

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(root, "target")
target = os.path.join(root, target) if not os.path.isabs(target) else target
app_bin = os.path.join(target, "release", "meloet")
vkeys_bin = os.path.join(target, "release", "meloet-vkeys")
out = os.path.join(root, "docs", "screenshots")

SIZES = [(1280, 800), (1920, 1080)]
SCALES = [100, 150]
BIG, SMALLEST = ((1280, 800), 100), ((1280, 672), 150)
CHORD = ["60", "64", "67"]
QUIET = "25"
TICKS_PER_QUARTER = 1920


def q(quarters):
    """Ticks of that many quarter notes."""
    return quarters * TICKS_PER_QUARTER


def band(name):
    return [(400, {"type": "toggleBand", "band": name})]


def played(lead_ms, count, interval_ms=250):
    return ["play", "--notes", "60,64,67,72", "--count", str(count), "--interval-ms", str(interval_ms),
            "--hold-ms", "150", "--velocity", QUIET, "--lead-ms", str(lead_ms)]


def held_chord(lead_ms, hold_ms):
    # One vkeys per note: `play` lets go of a note before it presses the next one.
    return [["play", "--notes", n, "--count", "1", "--hold-ms", str(hold_ms),
             "--velocity", QUIET, "--lead-ms", str(lead_ms)] for n in CHORD]


def click(selector):
    return f"document.querySelector('{selector}').click()"


def typed(text, field="shelf-search"):
    return (f"const input = document.querySelector('[data-test=\"{field}\"]'); "
            f"input.value = '{text}'; input.dispatchEvent(new Event('input', {{ bubbles: true }}));")


def context(selector):
    """A right click on the element."""
    return (f"const e = document.querySelector('{selector}'), r = e.getBoundingClientRect(); "
            "e.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, "
            "clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 2 }));")


WINDOWS_SONG = ["--tracks", "2", "--notes", "60", "--bars", "8", "--instruments", "plugin,builtin",
                "--plugin", "Surge XT", "--effect-plugin", "Dragonfly Hall Reverb", "--effect-format", "lv2"]
REVERB_SLOT = {"kind": "insert", "insert": {"strip": {"kind": "track", "id": 1}, "slot": 2}}
ROUND6_SONG = ["--tracks", "5", "--notes", "40", "--instruments", "builtin,builtin,builtin,builtin,plugin",
               "--plugin", "Surge XT"]
# Surge XT (VST3) parameter ids, read from the real app's parameter table.
SURGE_CUTOFF, SURGE_PITCH = 1945359057, 643940465


def lanes_song(tracks, points=6):
    """ROUND6_SONG with markers and a volume lane on the tracks at those places."""
    return ROUND6_SONG + ["--markers", f"0:イントロ,{q(16)}:Aメロ,{q(48)}:サビ", "--lanes", "volume",
                          "--lane-tracks", tracks, "--lane-points", str(points)]


def gain(track, db, commit):
    return {"type": "setGain", "strip": {"kind": "track", "id": track}, "db": db, "commit": commit}


def core_js(body):
    """Page JavaScript that reads the state and sends intents the way the screen does: for ids only
    the state holds (a lane's id, the take a clip shows now). lane(track, kind), take(track)."""
    return ("(async () => { const i = window.__TAURI_INTERNALS__; "
            "const s = (await i.invoke('snapshot')).state; "
            "const track = (id) => s.song.tracks.find((t) => t.id === id); "
            "const lane = (id, kind) => track(id).lanes.find((l) => l.target.kind === kind).laneId; "
            "const take = (id) => track(id).clips[0].takeId; "
            "const send = (command) => i.invoke('send', { command }); "
            + body + " })()")


# With no microphone, the monitor of this PC's output is the input (the vocal track hears
# the metronome). No state turns 「モニター」 on: the input would be played back into itself.
SINK_MONITOR = {"MELOET_INPUT_SINK_MONITOR": "alsa_output.pci-0000_0c_00.6.analog-stereo"}
SONG7 = ROUND6_SONG + ["--markers", f"0:イントロ,{q(16)}:Aメロ,{q(48)}:サビ"]
VOCAL = 100
RATE = 48000
# 96 bars at 96 a minute (240 s) sung at 76.80: making it takes some seconds (about 100 times
# faster than real time), so that 「音を作っています」 can be in the shot.
LONG_BARS, LONG_SECONDS = 96, 300.0


def vclip(name, start, end, file, seconds, offset=0, sync=96.0, missing=False, **shape):
    """A clip of the vocal track: its own take of the whole file, whose tick 0 is at start + offset.
    sync: the tempo it was sung at. missing: the file is not written. shape: as in the song file."""
    return dict(name=name, start=start, end=end, file=file, seconds=seconds, offset=offset, sync=sync,
                missing=missing, shape=shape)


_voice = None


def voice_wav(path, frames):
    """A made-up voice, mono, 24 bits: 10 s of sung-like notes and breaths, repeated."""
    global _voice
    if _voice is None:
        rng, block, t = random.Random(7), bytearray(), 0
        while t < 10 * RATE:
            n = int(RATE * rng.choice([0.25, 0.5, 0.75]))
            hz, level = rng.choice([196.0, 220.0, 261.6, 293.7, 329.6, 392.0]), rng.uniform(0.2, 0.6)
            rest = rng.random() < 0.2
            for j in range(n):
                env = min(1.0, j / 1400, (n - j) / 2400) * (0.65 + 0.35 * math.sin(2 * math.pi * 5 * j / RATE))
                x = 0.0 if rest else level * env * (math.sin(2 * math.pi * hz * j / RATE)
                                                    + 0.4 * math.sin(4 * math.pi * hz * j / RATE)) / 1.4
                block += struct.pack("<i", int(x * 8388607))[:3]
            t += n
        _voice = bytes(block[:10 * RATE * 3])
    whole, rest = divmod(frames * 3, len(_voice))
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(3)
        w.setframerate(RATE)
        w.writeframes(_voice * whole + _voice[:rest])


def dress_song(made, s, folder):
    """The song make_song.py made, as 「はじめての曲」 in a folder of its own: with s["loop"] on,
    and s["vocal"] as the clips of a vocal track 「歌」 (VOCAL; clips VOCAL + 1, + 2, …) whose
    files are written into 音/. Returns the path of the song file."""
    with open(made, encoding="utf-8") as f:
        data = json.load(f)
    song = data["song"]
    song["name"] = "はじめての曲"
    if s.get("loop"):
        a, b = s["loop"]
        song["loop"] = {"start": q(4 * (a - 1)), "end": q(4 * (b - 1))}
        data["session"]["loopOn"] = True
    if s.get("vocal"):
        os.makedirs(os.path.join(folder, "音"))
        audio, takes, clips = {}, [], []
        for n, c in enumerate(s["vocal"]):
            frames = round(c["seconds"] * RATE)
            if c["file"] not in audio:
                audio[c["file"]] = {"id": 200 + len(audio), "uid": hashlib.md5(c["file"].encode()).hexdigest(),
                                    "file": c["file"], "frames": frames, "rate": RATE, "channels": 1, "bits": 24}
                if not c["missing"]:
                    voice_wav(os.path.join(folder, "音", c["file"]), frames)
            take = {"id": 300 + n, "audio": audio[c["file"]]["id"], "firstFrame": 0, "frames": frames}
            if c["sync"]:
                take["sync"] = {"tempo": round(c["sync"] * 100)}
            takes.append(take)
            shape = {"fadeIn": 0, "fadeOut": 0, "gainDb": 0.0, "follow": False, "semitones": 0, "cents": 0,
                     "keepFormant": True, **c["shape"]}
            clips.append({"id": VOCAL + 1 + n, "name": c["name"], "start": c["start"], "end": c["end"],
                          "active": 0, "takes": [{"take": 300 + n, "offset": c["offset"]}], "shape": shape})
        song["tracks"].append({"id": VOCAL, "name": "歌", "color": 1, "kind": "vocal", "input": "mono1",
                               "instrument": {"kind": "builtin", "name": "sine"}, "gainDb": 0.0, "pan": 0,
                               "sends": {"a": None, "b": None}, "inserts": [None] * 4, "clips": clips,
                               "plugins": [], "automation": []})
        song["audio"] = list(audio.values())
        data["audioTakes"] = takes
        song["nextId"] = 400
    path = os.path.join(folder, "はじめての曲.meloet")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False)
    return path


TAKE1 = "歌 テイク 1.wav"
# F7-AudioEdit: 「歌 テイク 1」 cut at bar 7; the right half fades in and is at −3.0 dB.
EDIT_CLIPS = [vclip("歌 テイク 1", q(16), q(24), TAKE1, 10.0, fadeOut=q(1), follow=True),
              vclip("歌 テイク 1", q(24), q(32), TAKE1, 10.0, offset=-q(8), fadeIn=q(1) // 4, gainDb=-3.0, follow=True)]
ONE_CLIP = [vclip("歌 テイク 1", q(16), q(24), TAKE1, 5.0)]


# board: mock to compare with. remembered: start with the output already chosen.
# only_virtual / accept_virtual: MELOET_ONLY_VIRTUAL / MELOET_ACCEPT_VIRTUAL.
# keys: meloet-vkeys runs started together with the app. snap: MELOET_SNAPSHOT_AFTER_MS.
# With song:
# loop: (from bar, to bar), the song's loop, on. The song is then 「はじめての曲」 in a folder of its own.
# vocal: vclip()s of a vocal track 「歌」 added to the song (dress_song); its files are made in the
#   song's 音/ (none for missing=True), the song is opened where it is, and the start gets an
#   XDG_CACHE_HOME of its own (the scan record copied in) so that no made audio is kept between
#   shots. The song folder and that cache are removed after the shot.
# env: more environment variables for the app (SINK_MONITOR).
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
    # Times of commands and of the snapshot count from the page load (about 1.3 s after
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
    # song: notes of the one clip of a song made by tools/measure/make_song.py, opened
    # with MELOET_OPEN_SONG; its clip (id 2) is selected and shown in the piano roll.
    "pianoRollSong": dict(board="A-PianoRoll", remembered=True, song=200,
                          commands=[(600, {"type": "selectClip", "clip": 2}),
                                    (800, {"type": "toggleBand", "band": "pianoRoll"})], snap=2500),
    # Two tracks added through the shelf, one renamed, one recolored and moved to the top.
    "tracks": dict(board="R7-Arrange6", remembered=True, snap=3600, commands=[
        (400, {"type": "openShelf", "mode": {"kind": "newTrack"}}),
        (700, {"type": "shelfSelect", "instrument": "sine"}),
        (1000, {"type": "shelfConfirm"}),
        (1300, {"type": "shelfSelect", "instrument": "sine"}),
        (1600, {"type": "shelfConfirm"}),
        (1900, {"type": "renameTrack", "track": 2, "name": "リード"}),
        (2200, {"type": "setTrackColor", "track": 3, "color": 4}),
        (2500, {"type": "moveTrack", "track": 3, "to": 0}),
        (2800, {"type": "back"}),
    ]),
    # The plugin scan of the real app, over the plugins of this PC. scan="first": no
    # record yet. The others start with the record of a whole scan. scanAppDone waits for the
    # scan to end (19 s on the agent PC).
    "scanApp": dict(board="R9-Scan", remembered=True, scan="first", commands=band("shelf"), snap=3000),
    "scanAppDone": dict(board="R9-Scan", remembered=True, scan="first", commands=band("shelf"), snap=24000),
    "scanAppPlaces": dict(board="R9-Scan", remembered=True,
                          commands=[(400, {"type": "openPluginPlaces"})]),
    "scanAppStopped": dict(board="F5-ScanStopped", remembered=True,
                           commands=[(400, {"type": "toggleBand", "band": "shelf"}),
                                     (700, {"type": "openScanStopped"})]),
    # The shelf's 「あとから入れたもの」, plugin and SF2 tracks, a plugin host
    # stopped from outside, and what a song names that is not there. click / typed: what only
    # the screen knows (MELOET_EVAL). song: arguments for make_song.py. stop_host: (seconds
    # after the start, signal) for the first plugin host. gone: the song's plugin is renamed to
    # one this PC does not have.
    "sf2ShelfApp": dict(board="F5-SF2Shelf", remembered=True, sf2=True, snap=2500,
                        commands=band("shelf"),
                        evals=[(1200, click('[data-group="f:FluidR3_GM.sf2"]')),
                               (1600, click('[data-group="ff:guitar:FluidR3_GM.sf2"]'))]),
    "shelfPluginsApp": dict(board="Opt5-Plugins", remembered=True, sf2=True, snap=2500,
                            commands=band("shelf"),
                            evals=[(1200, click('[data-test="shelf-effects"]')),
                                   (1600, click('[data-group="c:effect:reverb"]'))]),
    "shelfSearchApp": dict(board="F5-ShelfSearch", remembered=True, sf2=True, snap=2500,
                           commands=band("shelf"),
                           evals=[(1200, click('[data-test="shelf-effects"]')), (1600, typed("Dragonfly"))]),
    "pluginAuditionApp": dict(board="F5-PluginAudition", remembered=True, sf2=True, only_virtual=True,
                              snap=5000, commands=band("shelf"), select_plugin=(1800, "Surge XT"),
                              evals=[(1200, click('[data-group="c:instrument:synth"]'))],
                              keys=held_chord(4300, 2500)),
    "pluginTrackApp": dict(board="F5-PluginTrack", remembered=True, sf2=True, snap=5000,
                           song=["--tracks", "3", "--notes", "60", "--bars", "8", "--instruments", "plugin,sf2,builtin",
                                 "--plugin", "Surge XT", "--effect-plugin", "ZamComp", "--sf2", "{sf2}:0:4:Rhodes EP"],
                           commands=[(1500, {"type": "toggleBand", "band": "mixer"}),
                                     (2500, {"type": "playStop"})]),
    "pluginStoppedApp": dict(remembered=True, sf2=True, snap=7000, stop_host=(5.5, "KILL"),
                             song=["--tracks", "3", "--notes", "60", "--bars", "8", "--instruments", "plugin,sf2,builtin",
                                   "--plugin", "Surge XT", "--sf2", "{sf2}:0:4:Rhodes EP"],
                             commands=[(1500, {"type": "toggleBand", "band": "shelf"}),
                                       (1700, {"type": "toggleBand", "band": "mixer"}),
                                       (2500, {"type": "playStop"})]),
    "missingSf2App": dict(board="R8-Missing", remembered=True, snap=3500,
                          song=["--tracks", "3", "--notes", "60", "--bars", "8", "--instruments", "plugin,sf2,builtin",
                                "--plugin", "Surge XT", "--sf2", "{sf2}:0:4:Rhodes EP"],
                          commands=band("shelf")),
    "missingPluginApp": dict(remembered=True, sf2=True, snap=3500, gone=True,
                             song=["--tracks", "3", "--notes", "60", "--bars", "8", "--instruments", "plugin,sf2,builtin",
                                   "--plugin", "Surge XT", "--effect-plugin", "ZamComp", "--sf2", "{sf2}:0:4:Rhodes EP"],
                             commands=[(400, {"type": "toggleBand", "band": "shelf"}),
                                       (600, {"type": "toggleBand", "band": "mixer"})]),
    # Neither the plugins nor the SF2 file are there, and no band is opened: the status line
    # still holds what meloet said when the song was opened.
    "missingStatusApp": dict(remembered=True, snap=3500, gone=True,
                             song=["--tracks", "3", "--notes", "60", "--bars", "8", "--instruments", "plugin,sf2,builtin",
                                   "--plugin", "Surge XT", "--effect-plugin", "ZamComp", "--sf2", "{sf2}:0:4:Rhodes EP"]),
    # Plugin windows, the parameters panel, the mixer's inserts. The windows of
    # Surge XT and Dragonfly open on the X screen for real (they are not in the shot).
    "windowOpenApp": dict(board="F-WindowOpen", remembered=True, snap=4800, song=WINDOWS_SONG,
                       commands=[(1500, {"type": "toggleBand", "band": "mixer"}),
                                 (2000, {"type": "openPluginWindow", "slot": {"kind": "instrument", "track": 1}}),
                                 (2700, {"type": "openPluginWindow", "slot": REVERB_SLOT})],
                       evals=[(3800, click('[data-test="status-windows"]'))]),
    "paramManyApp": dict(board="F-ParamMany", remembered=True, snap=4500, song=WINDOWS_SONG,
                      commands=[(1500, {"type": "selectInstrument", "track": 1}),
                                (2200, {"type": "toggleBand", "band": "params"})],
                      evals=[(3300, "[...document.querySelectorAll('[data-test=\"params-group\"]')]"
                                    ".find((e) => e.innerText.includes('Macros')).click()")]),
    "paramSearchApp": dict(board="F-ParamSearch", remembered=True, snap=4500, song=WINDOWS_SONG,
                        commands=[(1500, {"type": "selectInstrument", "track": 1}),
                                  (2200, {"type": "toggleBand", "band": "params"})],
                        evals=[(3300, typed("cutoff", "params-search"))]),
    "paramNoWindowApp": dict(board="F-ParamNoWindow", remembered=True, snap=4000,
                          song=["--tracks", "2", "--notes", "60", "--bars", "8", "--instruments", "plugin,builtin",
                                "--plugin", "Surge XT", "--effect-plugin", "abGate", "--effect-format", "lv2"],
                          commands=[(1500, {"type": "toggleBand", "band": "mixer"}),
                                    # Selecting a plugin that has no window meloet can open
                                    # unfolds the parameters panel by itself.
                                    (2000, {"type": "selectInsert", "insert": REVERB_SLOT["insert"]})]),
    "mixerBypassApp": dict(board="F5-MixerBypass", remembered=True, snap=3500, song=WINDOWS_SONG,
                        commands=[(1500, {"type": "toggleBypass", "insert": REVERB_SLOT["insert"]}),
                                  (2200, {"type": "toggleBand", "band": "mixer"})]),
    "insertReplaceApp": dict(board="R9-Inserts", remembered=True, snap=4000, song=WINDOWS_SONG,
                          commands=[(1500, {"type": "toggleBand", "band": "mixer"})],
                          evals=[(2600, context('[data-strip="track-1"] [data-test="mixer-insert"][data-slot="2"]'))]),
    # No keyboard and no output chosen: the master's frame says 「出口なし ▸」.
    "noOutputApp": dict(board="R9-Params", only_virtual=True, snap=2500,
                     commands=[(800, {"type": "toggleBand", "band": "mixer"})]),
    # Tempo, markers, automation lanes, the history of many steps. The songs are 5 tracks
    # as in the mocks; ROUND6_SONG's 5th is Surge XT. core_js: what needs ids only the state knows.
    "tempoApp": dict(board="F6-Tempo", remembered=True, snap=3500,
                     song=ROUND6_SONG + ["--tempo-points", f"{q(16)}:104:ramp,{q(32)}:120,{q(44)}:96",
                                         "--meter-changes", "8:3/4,12:4/4",
                                         "--markers", f"0:イントロ,{q(16)}:Aメロ,{q(44)}:サビ"],
                     commands=[(1500, {"type": "locate", "tick": q(24)}),
                               (1700, {"type": "setTimelineOpen", "open": True}),
                               (1900, {"type": "selectTimeline", "target": {"kind": "tempoPoint", "tick": q(16)}})]),
    "markersApp": dict(board="R10-Markers", remembered=True, snap=3000,
                       song=ROUND6_SONG + ["--markers", f"0:イントロ,{q(16)}:Aメロ,{q(36)}:サビ"],
                       commands=[(1500, {"type": "locate", "tick": q(24)}),
                                 (1800, {"type": "addMarker", "name": "マーカー 4"})]),
    # Played from bar 5; a hand on the fader makes the lane rest (the mock is playing, too).
    "autoRestApp": dict(board="F6-AutoRest", remembered=True, snap=9400, song=lanes_song("1"),
                        commands=[(1500, {"type": "locate", "tick": q(16)}), (1700, {"type": "playStop"}),
                                  (5000, gain(1, -10.0, False)), (5200, gain(1, -12.0, True))]),
    # 「● 書く」 on, played from bar 5, and the fader held and moved from about bar 7.
    "autoRecApp": dict(board="F6-AutoRec", remembered=True, snap=9400, song=lanes_song("1"),
                       commands=[(1500, {"type": "locate", "tick": q(16)}),
                                 (1600, {"type": "setLaneWrite", "track": 1, "target": {"kind": "volume"}, "on": True}),
                                 (1700, {"type": "playStop"})]
                       + [(6400 + 250 * n, gain(1, db, False))
                          for n, db in enumerate([-4.8, -5.5, -6.5, -8.0, -9.5, -10.5, -11.0, -11.5, -12.0, -12.0, -12.0])]),
    "autoEditApp": dict(board="R10-AutoEdit", remembered=True, snap=3000, song=lanes_song("1"),
                        commands=[(1500, {"type": "locate", "tick": q(28)})],
                        evals=[(1800, core_js("await send({type: 'selectAutoPoints', lane: lane(1, 'volume'), "
                                              "points: [1, 2, 3]});"))]),
    # The mixer opened while playing: the status line names the faders lanes move.
    "mixerAutoApp": dict(board="F6-MixerAuto", remembered=True, snap=8000, song=lanes_song("1,4", 8),
                         commands=[(1500, {"type": "locate", "tick": q(24)}),
                                   (1600, {"type": "playStop"}), (1800, {"type": "toggleBand", "band": "mixer"})]),
    "mixerRestApp": dict(board="F6-MixerRest", remembered=True, snap=8000, song=lanes_song("1,4", 8),
                         commands=[(1500, {"type": "locate", "tick": q(24)}),
                                   (1600, {"type": "playStop"}), (1800, {"type": "toggleBand", "band": "mixer"}),
                                   (4500, gain(10, -10.0, False)), (4700, gain(10, -12.0, True))]),
    "paramManyAutoApp": dict(board="F6-ParamMany", remembered=True, snap=4800,
                             song=["--tracks", "5", "--notes", "40", "--instruments", "plugin,builtin",
                                   "--plugin", "Surge XT", "--markers", f"0:イントロ,{q(16)}:Aメロ",
                                   "--lanes", f"volume,plugin:{SURGE_CUTOFF},plugin:{SURGE_PITCH}",
                                   "--lane-tracks", "1", "--lane-points", "5"],
                             commands=[(1500, {"type": "locate", "tick": q(16)}),
                                       (1700, {"type": "selectInstrument", "track": 1}),
                                       (2200, {"type": "toggleBand", "band": "params"})],
                             evals=[(3500, "[...document.querySelectorAll('[data-test=\"params-group\"]')]"
                                           ".find((e) => e.innerText.includes('Macros')).click()")]),
    # 16 steps, 3 of them undone (「いまここ」 is 13), and row 11 chosen with ↓ in the band.
    "historyStepsApp": dict(board="R10-History", remembered=True, snap=7500,
                            song=["--tracks", "5", "--notes", "40", "--markers", f"0:イントロ,{q(16)}:Aメロ"],
                            commands=[(1500 + 200 * n, c) for n, c in enumerate([
                                {"type": "renameTrack", "track": 1, "name": "ピアノ"},
                                {"type": "renameTrack", "track": 4, "name": "ベース"},
                                {"type": "renameTrack", "track": 7, "name": "ドラム"},
                                {"type": "renameTrack", "track": 10, "name": "パッド"},
                                gain(1, -4.8, True), gain(4, -7.5, True), gain(7, -6.0, True), gain(10, -10.5, True),
                                {"type": "setPan", "strip": {"kind": "track", "id": 10}, "pan": -20, "commit": True},
                                {"type": "addTempoPoint", "tick": q(16), "bpm": 120},
                                {"type": "locate", "tick": q(48)},
                                {"type": "addMarker", "name": "サビ"},
                                {"type": "closeMarkerName"},
                                {"type": "toggleBypass", "insert": {"strip": {"kind": "track", "id": 10}, "slot": 0}},
                                gain(10, -12.0, True)])]
                            # The notes of the piano roll's clip only.
                            + [(4350, {"type": "selectClip", "clip": 2}), (4400, {"type": "toggleBand", "band": "pianoRoll"}),
                               (5200, {"type": "undo"}), (5400, {"type": "undo"}), (5600, {"type": "undo"}),
                               (5700, {"type": "toggleBand", "band": "pianoRoll"}),
                               (5800, {"type": "toggleBand", "band": "history"})],
                            evals=[(4500, core_js("await send({type: 'quantize', take: take(1), "
                                                  "notes: [...Array(12).keys()]});")),
                                   (4700, core_js("await send({type: 'resizeNotes', take: take(1), notes: [0, 1], "
                                                  f"edge: 'end', ticks: {q(1) // 4}}});")),
                                   (4900, core_js("await send({type: 'moveNotes', take: take(1), notes: [0, 1, 2], "
                                                  f"ticks: {q(1) // 4}, keys: 0, copy: false}});")),
                                   (6500, "const l = document.querySelector('[data-test=\"history-list\"]'); l.focus(); "
                                          "for (const n of [0, 1]) l.dispatchEvent(new KeyboardEvent('keydown', "
                                          "{ key: 'ArrowDown', bubbles: true, cancelable: true }));")]),
    # Vocal tracks, vocal clips, the latency, writing the song out. The songs are SONG7
    # (5 keys tracks, the 5th Surge XT) with 「歌」 added by a command or by dress_song.
    "vocalTrackApp": dict(board="F7-VocalTrack", remembered=True, snap=3200, song=SONG7, loop=(5, 9),
                          env=SINK_MONITOR, commands=[(1500, {"type": "addVocalTrack"})],
                          evals=[(2300, click('[data-test="track-input"]'))]),
    # Recorded from bar 5 after a bar of count-in; the play line at about 7. 2. 0000.
    "vocalRecApp": dict(board="F7-VocalRec", remembered=True, snap=10600, song=SONG7, env=SINK_MONITOR,
                        commands=[(1500, {"type": "toggleMetronome"}), (1700, {"type": "addVocalTrack"}),
                                  (1900, {"type": "locate", "tick": q(16)}), (2500, {"type": "record"})]),
    # 「1 ピアノ」 selected again, the ● of 「歌」 stays on: keys and voice together.
    "twoRecApp": dict(board="F7-TwoRec", remembered=True, only_virtual=True, snap=10600, song=SONG7,
                      env=SINK_MONITOR,
                      commands=[(1500, {"type": "toggleMetronome"}), (1700, {"type": "addVocalTrack"}),
                                (1900, {"type": "selectTrack", "track": 1, "fromArm": False}),
                                (2100, {"type": "locate", "tick": q(16)}), (2500, {"type": "record"})],
                      keys=[played(6800, 12, 300)] + held_chord(10800, 3000)),
    "audioEditApp": dict(board="F7-AudioEdit", remembered=True, snap=3000, song=SONG7, vocal=EDIT_CLIPS,
                         env=SINK_MONITOR,
                         commands=[(1500, {"type": "selectTrack", "track": VOCAL, "fromArm": False}),
                                   (1600, {"type": "locate", "tick": q(24)}),
                                   (1700, {"type": "selectClip", "clip": VOCAL + 2}),
                                   (1900, {"type": "toggleBand", "band": "params"})]),
    # 「曲のテンポについていく」 and +2 semitones at 2000: the audio is being made at the shot.
    "stretchApp": dict(board="F7-Stretch", remembered=True, snap=2900, song=SONG7, env=SINK_MONITOR,
                       vocal=[vclip("歌 テイク 1", q(16), q(16 + 4 * LONG_BARS), TAKE1, LONG_SECONDS, sync=76.8,
                                    fadeIn=q(1) // 4)],
                       commands=[(1500, {"type": "selectTrack", "track": VOCAL, "fromArm": False}),
                                 (1600, {"type": "locate", "tick": q(16)}),
                                 (1700, {"type": "selectClip", "clip": VOCAL + 1}),
                                 (1900, {"type": "toggleBand", "band": "params"}),
                                 (2000, {"type": "setClipStretch", "clip": VOCAL + 1, "change": {"kind": "follow", "on": True}}),
                                 (2050, {"type": "setClipStretch", "clip": VOCAL + 1,
                                         "change": {"kind": "semitones", "semitones": 2}})]),
    "audioMissingApp": dict(board="F7-AudioMissing", remembered=True, snap=3000, song=SONG7, env=SINK_MONITOR,
                            vocal=[vclip("歌 テイク 1", q(16), q(24), TAKE1, 5.0, missing=True),
                                   vclip("ハモり", q(32), q(40), "ハモり.wav", 5.0, missing=True)],
                            commands=[(1500, {"type": "selectClip", "clip": VOCAL + 1}),
                                      (1700, {"type": "toggleBand", "band": "params"})]),
    # Three of the five clicks: each waits up to 300 ms for its answer.
    "latencyApp": dict(board="F7-Latency", remembered=True, snap=3500, song=SONG7, loop=(5, 9), env=SINK_MONITOR,
                       commands=[(1500, {"type": "openSettings", "advanced": True}),
                                 (1800, {"type": "openLatencyMeasure"}), (2600, {"type": "startLatencyMeasure"})]),
    # Asks for 128 frames; the PC runs what it is set to (256 on the agent PC).
    "settingsAdv7App": dict(board="F7-SettingsAdv", remembered=True, snap=3000, song=SONG7, loop=(5, 9),
                            commands=[(1500, {"type": "openSettings", "advanced": True}),
                                      (1800, {"type": "settingsSetBuffer", "frames": 128})]),
    # The check (the whole song written to a work file, Surge XT loaded for it) is done at the shot.
    "exportApp": dict(board="F7-Export", remembered=True, snap=9000, song=SONG7, loop=(5, 9),
                      commands=[(1500, {"type": "openExport"})]),
    # 600 bars (25 minutes; written about 100 times faster than real time): still being written at the shot.
    "exportingApp": dict(board="F7-Exporting", remembered=True, snap=6000, song=SONG7 + ["--bars", "600"],
                         loop=(5, 9), vocal=ONE_CLIP, commands=[(1500, {"type": "openExport"})],
                         evals=[(1700, core_js("for (let n = 0; n < 300; n++) { "
                                               "const d = (await i.invoke('snapshot')).state.export.dialog; "
                                               "if (d && d.canExport) break; "
                                               "await new Promise((r) => setTimeout(r, 50)); } "
                                               "await send({type: 'startExport'});"))]),
    "answers92App": dict(board="F7-Answers92", remembered=True, snap=3300, song=lanes_song("1", 5), loop=(5, 9),
                         commands=[(1500, {"type": "locate", "tick": q(16)}),
                                   # Unfolds the parameters panel by itself.
                                   (1700, {"type": "selectInstrument", "track": 1})],
                         evals=[(2600, click('[data-test="lane-add"]'))]),
    # The kinds of vocal clip side by side (plain, fades and volume, missing, being made).
    "r11AudioApp": dict(board="R11-Audio", remembered=True, snap=2900, song=SONG7, env=SINK_MONITOR,
                        vocal=[vclip("歌 テイク 1", 0, q(8), TAKE1, 10.0),
                               vclip("歌 テイク 1", q(8), q(16), TAKE1, 10.0, offset=-q(8), fadeIn=q(1), fadeOut=q(2),
                                     gainDb=-3.0),
                               vclip("ハモり", q(16), q(24), "ハモり.wav", 5.0, missing=True),
                               vclip("歌 テイク 2", q(24), q(24 + 4 * LONG_BARS), "歌 テイク 2.wav", LONG_SECONDS,
                                     sync=76.8)],
                        commands=[(1500, {"type": "selectTrack", "track": VOCAL, "fromArm": False}),
                                  (1600, {"type": "selectClip", "clip": VOCAL + 2}),
                                  (1800, {"type": "toggleBand", "band": "params"}),
                                  (2000, {"type": "setClipStretch", "clip": VOCAL + 4, "change": {"kind": "follow", "on": True}}),
                                  (2050, {"type": "setClipStretch", "clip": VOCAL + 4,
                                          "change": {"kind": "semitones", "semitones": 2}})]),
    # 1280x800 at 150 % is the smallest window (853 wide): the input meter in two halves and the
    # export dialog scrolling inside, at once.
    "r11FitApp": dict(board="R11-Fit", remembered=True, snap=9000, song=SONG7, loop=(5, 9), vocal=ONE_CLIP,
                      env=SINK_MONITOR,
                      commands=[(1500, {"type": "selectTrack", "track": VOCAL, "fromArm": False}),
                                (1700, {"type": "openExport"})]),
    # A marker at bar 2 in the piano roll's ruler, the history band with no step, the head's tempo selected.
    "r11Answers92App": dict(board="R11-Answers92", remembered=True, snap=3000,
                            song=ROUND6_SONG + ["--markers", f"0:イントロ,{q(4)}:サビ"],
                            commands=[(1500, {"type": "selectClip", "clip": 2}),
                                      (1600, {"type": "toggleBand", "band": "pianoRoll"}),
                                      (1800, {"type": "toggleBand", "band": "history"}),
                                      (2000, {"type": "setTimelineOpen", "open": True}),
                                      (2200, {"type": "selectTimeline", "target": {"kind": "tempoPoint", "tick": 0}})]),
    # shots: the (size, scale) pairs taken instead of every SIZES x SCALES; SMALLEST is the
    # smallest window (853x448 at 150 %).
    "firstLaneApp": dict(board="F8-FirstLane", remembered=True, snap=3000, song=SONG7, loop=(5, 9), shots=[BIG],
                         evals=[(2300, "document.querySelectorAll('[data-test=\"track-lanes\"]')[1].click()")]),
    "multiClipApp": dict(board="F8-MultiClip", remembered=True, snap=3200, song=SONG7, loop=(5, 9), env=SINK_MONITOR,
                         vocal=[vclip("歌 テイク 1", q(16), q(24), TAKE1, 5.0),
                                vclip("歌 テイク 2", q(32), q(40), "歌 テイク 2.wav", 5.0),
                                vclip("ハモり", q(48), q(56), "ハモり.wav", 5.0)],
                         shots=[BIG, SMALLEST],
                         commands=[(1500, {"type": "selectClips", "clips": [VOCAL + 1, VOCAL + 2, VOCAL + 3],
                                           "mode": "replace"}),
                                   (1800, {"type": "toggleBand", "band": "params"})]),
    # 600 bars with 「2 ベース」 soloed: the check is still going at the shot.
    "exportCheckApp": dict(board="F8-ExportCheck", remembered=True, snap=3600, song=SONG7 + ["--bars", "600"],
                           loop=(5, 9), shots=[BIG, SMALLEST],
                           commands=[(1300, {"type": "toggleSolo", "track": 4}), (1500, {"type": "openExport"})]),
    # The master's limiter bypassed, every fader at +6 dB, more notes, hard pans: the check finds
    # places over 0 dB.
    "exportClipApp": dict(board="F8-ExportClip", remembered=True, snap=9000, song=SONG7 + ["--notes", "400", "--hard-pan"],
                          loop=(5, 9), shots=[BIG],
                          commands=[(1150, {"type": "toggleBypass", "insert": {"strip": {"kind": "master"}, "slot": 0}}),
                                    (1200, {"type": "setGain", "strip": {"kind": "master"}, "db": 6.0, "commit": True})]
                          + [(1250 + 10 * n, gain(t, 6.0, True)) for n, t in enumerate([1, 4, 7, 10, 13])]
                          + [(1500, {"type": "openExport"})]),
    # The vocal track selected in the smallest window: the input meter in two halves, the
    # microphone's meter hearing the metronome (the input is the monitor of the output).
    "r12MetersApp": dict(board="R12-Meters", remembered=True, snap=3600, song=SONG7, env=SINK_MONITOR,
                         shots=[BIG, SMALLEST],
                         commands=[(1500, {"type": "toggleMetronome"}), (1700, {"type": "addVocalTrack"}),
                                   (1900, {"type": "playStop"})]),
    # The same with the mixer open below: the slim input meter.
    "r12FitApp": dict(board="R12-Fit", remembered=True, snap=3600, song=SONG7, env=SINK_MONITOR,
                      shots=[BIG, SMALLEST],
                      commands=[(1500, {"type": "toggleMetronome"}), (1700, {"type": "addVocalTrack"}),
                                (1800, {"type": "toggleBand", "band": "mixer"}), (1900, {"type": "playStop"})]),
    "packSettingsApp": dict(board="F8-PackSettings", remembered=True, snap=3000, song=SONG7, loop=(5, 9),
                            shots=[BIG, SMALLEST], commands=[(1500, {"type": "openSettings", "advanced": True})]),
    "plugIn": dict(remembered=True, only_virtual=True, keys=[["appear", "--seconds", "3"]], snap=1500),
    "plugOut": dict(remembered=True, only_virtual=True, keys=[["appear", "--seconds", "2"]], snap=4000),
}


def run(cmd, **kw):
    subprocess.run(cmd, check=True, **kw)


def build():
    env = dict(os.environ)
    env["PATH"] = os.path.expanduser("~/.local/node/bin") + os.pathsep + env.get("PATH", "")
    # The development hooks (MELOET_WINDOW, MELOET_SNAPSHOT, ...) are only in this build.
    env["MELOET_DEV_HOOKS"] = "1"
    run(["npm", "run", "build"], cwd=os.path.join(root, "ui"), env=env, stdout=subprocess.DEVNULL)
    run(["cargo", "build", "--release", "-p", "meloet-app", "-p", "meloet-midi", "-p", "meloet-plughost",
         "--features", "dev-hooks"], cwd=root, env=env)


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


def scanned_cache(work):
    """An XDG_CACHE_HOME that holds the record of one whole plugin scan."""
    cache = os.path.join(target, "shoot", "cache")
    if os.path.exists(os.path.join(cache, "meloet", "plugins", "scan.json")):
        return cache
    shutil.rmtree(cache, ignore_errors=True)
    os.makedirs(cache)
    config = os.path.join(work, "scan-seed")
    os.makedirs(config)
    log_path = os.path.join(work, "scan-seed.log")
    with open(log_path, "wb") as log:
        app = start_app(config, SIZES[0], dict(XDG_CACHE_HOME=cache, MELOET_EXIT_AFTER_MS="300000"), log)
        # The scanner says so on stderr when a scan that had bundles to read has ended.
        for _ in range(600):
            if app.poll() is not None or b"plugin bundle(s) in" in open(log_path, "rb").read():
                break
            time.sleep(0.5)
        # The app is the child of dbus-run-session; it ends the usual way at SIGTERM.
        for pid in subprocess.run(["pgrep", "-P", str(app.pid), "-x", "meloet"],
                                  capture_output=True, text=True).stdout.split():
            os.kill(int(pid), signal.SIGTERM)
        app.wait(timeout=60)
    return cache


SF2_SOURCE = "/usr/share/sounds/sf2/FluidR3_GM.sf2"


def sf2_dir():
    """A MELOET_SF2_DIR that holds FluidR3_GM.sf2, as if the person had added it."""
    folder = os.path.join(target, "shoot", "sf2")
    if not os.path.exists(os.path.join(folder, os.path.basename(SF2_SOURCE))):
        if not os.path.exists(SF2_SOURCE):
            sys.exit(f"no {SF2_SOURCE} on this PC")
        os.makedirs(folder, exist_ok=True)
        shutil.copy(SF2_SOURCE, folder)
    return folder


def plugin_key(scan, name):
    """The VST3 key of the plugin with that name in the scan record."""
    with open(scan, encoding="utf-8") as f:
        record = json.load(f)
    for bundle in record["bundles"]:
        for plugin in bundle["plugins"]:
            if plugin["name"] == name and plugin["key"].startswith("vst3:"):
                return plugin["key"]
    sys.exit(f"no VST3 plugin named {name!r} in {scan}")


def rename_plugins(song):
    """Every plugin of the song becomes one this PC does not have (its settings stay)."""
    with open(song, encoding="utf-8") as f:
        data = json.load(f)

    def gone(plugin, name):
        plugin.update(key="vst3:00000000000000000000000000000000", name=name, vendor="Nobody",
                      path=f"/usr/lib/vst3/{name}.vst3")

    for track in data["song"]["tracks"]:
        if track["instrument"].get("kind") == "plugin":
            gone(track["instrument"], "Lost Synth")
        for slot in track["inserts"]:
            if slot and "plugin" in slot:
                gone(slot["plugin"], "Lost Comp")
                slot["plugin"]["key"] = "vst3:11111111111111111111111111111111"
    with open(song, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False)


def shoot(state, size, scale, settings, work, png):
    s = STATES[state]
    config = tempfile.mkdtemp(prefix="cfg-", dir=work)
    if s.get("remembered"):
        shutil.copy(settings, os.path.join(config, "settings.json"))
    commands = ([(100, {"type": "setScale", "percent": scale})] if scale != 100 else []) + s.get("commands", [])
    extra = dict(MELOET_SNAPSHOT=png, MELOET_SNAPSHOT_AFTER_MS=str(s.get("snap", 1500)),
                 MELOET_EXIT_AFTER_MS=str(max(20000, s.get("snap", 0) + 5000)))
    extra["XDG_CACHE_HOME"] = (tempfile.mkdtemp(prefix="cache-", dir=work) if s.get("scan") == "first"
                               else scanned_cache(work))
    removed = []
    if s.get("vocal"):
        own = tempfile.mkdtemp(prefix="cache-", dir=work)
        shutil.copytree(os.path.join(extra["XDG_CACHE_HOME"], "meloet", "plugins"), os.path.join(own, "meloet", "plugins"))
        extra["XDG_CACHE_HOME"] = own
        removed.append(own)
    if commands:
        extra["MELOET_COMMANDS"] = json.dumps([{"afterMs": ms, "command": c} for ms, c in commands])
    scan = os.path.join(extra["XDG_CACHE_HOME"], "meloet", "plugins", "scan.json")
    extra["MELOET_SF2_DIR"] = sf2_dir() if s.get("sf2") else tempfile.mkdtemp(prefix="sf2-", dir=work)
    if s.get("select_plugin"):
        ms, name = s["select_plugin"]
        commands = commands + [(ms, {"type": "shelfSelectSound", "sound": {"kind": "plugin", "key": plugin_key(scan, name)}})]
        extra["MELOET_COMMANDS"] = json.dumps([{"afterMs": ms, "command": c} for ms, c in commands])
    if s.get("evals"):
        extra["MELOET_EVAL"] = json.dumps([{"afterMs": ms, "js": js} for ms, js in s["evals"]])
    if s.get("song"):
        song = os.path.join(work, f"song-{state}.meloet")
        make_song = [sys.executable, "-I", os.path.join(root, "tools", "measure", "make_song.py"), song]
        if isinstance(s["song"], int):
            run(make_song + ["--notes", str(s["song"])], stdout=subprocess.DEVNULL)
        else:
            run(make_song + ["--scan", scan] + [a.replace("{sf2}", SF2_SOURCE) for a in s["song"]],
                stdout=subprocess.DEVNULL)
        if s.get("gone"):
            rename_plugins(song)
        if s.get("loop") or s.get("vocal"):
            folder = os.path.join(tempfile.mkdtemp(prefix="song-", dir=work), "はじめての曲")
            os.makedirs(folder)
            song = dress_song(song, s, folder)
            removed.append(os.path.dirname(folder))
        extra.update(MELOET_OPEN_SONG=song, MELOET_STATE_DIR=tempfile.mkdtemp(prefix="state-", dir=work))
    if s.get("stop_host"):
        # The host that is stopped on purpose writes no crash report.
        extra["MELOET_PLUGHOST_NO_DUMP"] = "1"
    if s.get("only_virtual"):
        extra["MELOET_ONLY_VIRTUAL"] = "1"
    if s.get("accept_virtual"):
        extra["MELOET_ACCEPT_VIRTUAL"] = "1"
    extra.update(s.get("env", {}))
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
        if s.get("stop_host"):
            at, sig = s["stop_host"]
            keys.append(subprocess.Popen(
                [sys.executable, "-I", os.path.join(root, "tools", "measure", "plug_watch.py"), os.devnull,
                 os.path.join(target, "release", "meloet-plughost"), "--seconds", str(at + 2),
                 "--signal", sig, "--at", str(at)], stdout=subprocess.DEVNULL))
        app.wait(timeout=60)
        for k in keys:
            k.wait(timeout=30)
    for path in removed:
        shutil.rmtree(path, ignore_errors=True)
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
    only = args[args.index("--only") + 1] if "--only" in args else None
    if only:
        args.remove(only)
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
            pairs = STATES[state].get("shots") or [(size, scale) for size in SIZES for scale in SCALES]
            for size, scale in pairs:
                stem = f"{state}-{size[0]}x{size[1]}-{scale}"
                if only and only != f"{size[0]}x{size[1]}-{scale}":
                    continue
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
