#!/usr/bin/env python3
"""Shoot the screen (fake scenes) and put each shot next to its mock board in one PNG.

usage: python3 -I tools/design/shoot_ui.py OUT_DIR [SCENE ...]

Runs `npm run build` in ui/, serves ui/dist with `vite preview`, and shoots each scene with
headless google-chrome at 1280x800 and 1920x1080, scale 100 and 150. Next to each shot it puts
the mock board for that scene (rendered by render_mocks.py), cropped or padded to the same size.
Out: OUT_DIR/<scene>-<w>x<h>-<scale>.png (side by side) and OUT_DIR/shots/ (the shots alone).
Names in CLICKS shoot a scene after clicking something (a menu open), through the DevTools protocol.
"""
import json, os, socket, subprocess, sys, time, urllib.request

from PIL import Image, ImageDraw

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ui = os.path.join(root, "ui")
out = os.path.abspath(sys.argv[1])
shots = os.path.join(out, "shots")
mocks = os.path.join(out, "mock")

# scene -> mock board (None: no board to compare with)
BOARDS = {
    "connect1": "F-Connect1",
    "connect2": "F-Connect2",
    "connect3": "F-Connect3",
    "empty": "F-Empty",
    "bufferFixed": "F-BufferFixed",
    "settings": "F-Settings",
    "settingsAdvFixed": "F-SettingsAdvFixed",
    "settingsEnd": None,
    "settingsAdvFixedEnd": None,
    "longName": "F-MeterLongName",
    "pianoRoll": "A-PianoRoll",
    "pianoRoll720": "F-PR720",
    "pianoRoll150": "F-PR150",
    "pianoRollWide": "F-WidePR",
    "pianoRollPlay": "F-PRPlay",
    "pianoRollRec": "F-PRRec",
    "mixer": "A-Mixer",
    "history": "F-History",
    "shelf": "F-ShelfArrange",
    "params": "F-ParamFew",
    "recording": "F-ArrangeRec",
    "countIn": "F-Connect4",
    "takesArrange": "F-Takes",
    "loop": "Main",
    "muted": "Main",
    "menuFile": "R5-Menus",
    "menuZoom": "R5-Menus",
    "menuClip": "R5-Menus",
    "recover1": "F-Recover1",
    "recover2": "F-Recover2",
    "recover3": "F-Recover3",
    "arrangeZoomIn": "R3-Arrange",
    "arrangeZoomBeats": "R3-Arrange",
    "scaleMenu": "A-PianoRoll",
    "noteListTyping": "R3-Lanes",
    "noteListBroken": "R3-Lanes",
    "ccLanes": "R3-Lanes",
    "ccOtherMenu": "A-PianoRoll",
    "arrangeTrimmed": "R3-Arrange",
    "arrangeTrimmedRoll": None,
    "arrangeMoveDrag": "R3-Arrange",
    "arrangeCopyDrag": "R3-Arrange",
    "arrangeTrimDrag": "R3-Arrange",
}
# Double-clicks the Vel cell of a row of the note list and types into the field (R3-Lanes: "96|" and "200").
TYPE_IN_LIST = """
const cell = document.querySelectorAll('[data-testid="pr-note-list"] .row')[%d].querySelector('[data-field="velocity"]');
cell.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
await new Promise((r) => setTimeout(r, 100));
const input = document.querySelector('[data-test="note-list-field"]');
input.value = "%s";
input.dispatchEvent(new Event("input", { bubbles: true }));
"""

# Presses "ベース A" (track 2, bars 5–8) of the scene arrangeTrimmed and drags it without letting go (R3-Arrange).
# Arguments: x pressed, x moved to (CSS px from the left of the lanes, 1 bar = 64 at 100 %), and the PointerEvent options.
DRAG_CLIP = """
const lanes = document.querySelector(".lanes");
const r = lanes.getBoundingClientRect();
const ev = (type, x, o) => lanes.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: r.left + x, clientY: r.top + 94, ...o }));
ev("pointerdown", %d, %s);
ev("pointermove", (%d + %d) / 2, %s);
ev("pointermove", %d, %s);
"""


# Right-clicks "Aメロ" of the piano (bars 5–8) to open the clip's list (R5-Menus).
CONTEXT_CLIP = """
const lanes = document.querySelector(".lanes");
const r = lanes.getBoundingClientRect();
lanes.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: r.left + 300, clientY: r.top + 30 }));
"""


def drag_clip(x0, x1, opts="{}"):
    return "js:" + DRAG_CLIP % (x0, opts, x0, x1, opts, x1, opts)


# Shots that need a click first (a menu is screen-only state): name -> (scene, CSS selector to click).
# A selector that starts with "js:" is run as a script instead (it may await).
# Several selectors (a tuple) are clicked one after another.
CLICKS = {
    "songMenu": ("saved", '[data-test="transport-song"]'),
    "countInMenu": ("empty", '[data-test="transport-count-in"]'),
    "arrangeZoomIn": ("loop", '[aria-label="横に拡大"]'),
    # Two steps in: 1 beat is as wide as the gap between bar numbers, so beat lines show (B-61).
    "arrangeZoomBeats": ("loop", ('[aria-label="横に拡大"]', '[aria-label="横に拡大"]')),
    "scaleMenu": ("pianoRoll", '[data-test="inspector-scale"]'),
    "noteListTyping": ("pianoRoll", "js:" + TYPE_IN_LIST % (2, "96")),
    "noteListBroken": ("pianoRoll", "js:" + TYPE_IN_LIST % (4, "200")),
    "ccLanes": ("pianoRollCc", ('[data-test="pr-cc-sustain"]', '[data-test="pr-cc-pitchBend"]', '[data-test="pr-cc-other"]',
                                '[data-test="pr-cc-other-menu"] [data-row="0"]')),
    "ccOtherMenu": ("pianoRollCc", '[data-test="pr-cc-other"]'),
    "arrangeMoveDrag": ("arrangeTrimmed", drag_clip(300, 364)),
    "arrangeCopyDrag": ("arrangeTrimmed", drag_clip(300, 556, "{ altKey: true }")),
    "arrangeTrimDrag": ("arrangeTrimmed", drag_clip(509, 413, "{ shiftKey: true }")),
    "menuFile": ("saved", '[data-menu="0"]'),
    "menuZoom": ("muted", ('[data-menu="2"]', '[data-test="menubar-list"] [data-row="10"]')),
    "menuClip": ("muted", "js:" + CONTEXT_CLIP),
}
SIZES = [(1280, 800), (1920, 1080)]
SCALES = [100, 150]
# Scenes that stand for one board at one window size and zoom: name -> [(w, h, scale)].
ONLY = {
    "pianoRoll": [(1280, 800, 100)],
    "pianoRoll720": [(1280, 720, 100)],
    "pianoRoll150": [(1280, 800, 150)],
    "pianoRollWide": [(1920, 1080, 100)],
    "pianoRollPlay": [(1280, 800, 100)],
    "pianoRollRec": [(1280, 800, 100)],
    "noteListTyping": [(1280, 800, 100)],
    "noteListBroken": [(1280, 800, 100)],
    "ccLanes": [(1280, 800, 100), (1920, 1080, 100), (1920, 1080, 150)],
    "ccOtherMenu": [(1920, 1080, 100)],
    "arrangeTrimmed": [(1280, 800, 100), (1280, 800, 150)],
    "arrangeZoomBeats": [(1280, 800, 100), (1280, 800, 150)],
    # At 1280x800 150 % the right panel is folded, so there is no scale button to press.
    "scaleMenu": [(1280, 800, 100), (1920, 1080, 150)],
    "arrangeTrimmedRoll": [(1280, 800, 100)],
    "arrangeMoveDrag": [(1280, 800, 100)],
    "arrangeCopyDrag": [(1280, 800, 100)],
    "arrangeTrimDrag": [(1280, 800, 100)],
    "menuFile": [(1280, 800, 100), (1280, 800, 150)],
    "menuZoom": [(1280, 800, 100)],
    "menuClip": [(1280, 800, 100)],
}
scenes = sys.argv[2:] or list(BOARDS)

# Drives headless Chrome over the DevTools protocol (Node's own WebSocket): open, click, shoot.
CLICK_SHOT = r"""
const [port, url, selectors, out, w, h] = process.argv.slice(1);
const { spawn } = await import("node:child_process");
const { writeFileSync, mkdtempSync } = await import("node:fs");
const { tmpdir } = await import("node:os");
const { join } = await import("node:path");
const chrome = spawn("google-chrome", ["--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
  `--window-size=${w},${h}`, `--remote-debugging-port=${port}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "shoot-"))}`, url],
  { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
try {
  let page;
  for (let i = 0; i < 100 && !page; i++) {
    await sleep(100);
    try { page = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page"); } catch {}
  }
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => { const d = JSON.parse(m.data); waiting.get(d.id)?.(d.result); };
  const call = (method, params = {}) => new Promise((r) => { waiting.set(++id, r); ws.send(JSON.stringify({ id, method, params })); });
  await call("Emulation.setDeviceMetricsOverride", { width: Number(w), height: Number(h), deviceScaleFactor: 1, mobile: false });
  // The first tab can still be about:blank; load the scene in the tab we hold.
  await call("Page.enable");
  await call("Page.navigate", { url });
  const evaluate = async (expression) => (await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }))?.result?.value;
  // Waits (in the page, up to 60 s) until `test` is true, then until the page stops changing: fonts loaded, no change
  // to the DOM for 300 ms (at most 5 s more: a playing scene never stops), and two frames for WebGL to paint.
  const settle = async (test) => {
    const ok = await evaluate(`(async () => {
      const end = Date.now() + 60000;
      while (!(${test}) && Date.now() < end) await new Promise((r) => setTimeout(r, 50));
      if (!(${test})) return false;
      await document.fonts.ready;
      await new Promise((done) => {
        let t = setTimeout(stop, 300);
        const cap = setTimeout(stop, 5000);
        const mo = new MutationObserver(() => { clearTimeout(t); t = setTimeout(stop, 300); });
        mo.observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
        function stop() { mo.disconnect(); clearTimeout(t); clearTimeout(cap); done(); }
      });
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      return true;
    })()`);
    if (!ok) throw new Error(`the page did not get ready: ${test}`);
  };
  await settle(`location.href === ${JSON.stringify(url)} && document.readyState === "complete" && document.querySelector(".app .status") !== null`);
  for (const selector of JSON.parse(selectors)) {
    if (selector.startsWith("js:")) {
      await evaluate(`(async () => {${selector.slice(3)}})()`);
      await settle("true");
    } else {
      await settle(`document.querySelector(${JSON.stringify(selector)}) !== null`);
      await evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
      await settle("true");
    }
  }
  const shot = await call("Page.captureScreenshot", { format: "png" });
  writeFileSync(out, Buffer.from(shot.data, "base64"));
  ws.close();
} finally {
  chrome.kill();
}
"""


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def run(cmd, **kw):
    subprocess.run(cmd, check=True, **kw)


os.makedirs(shots, exist_ok=True)
env = dict(os.environ)
env["PATH"] = os.path.expanduser("~/.local/node/bin") + os.pathsep + env.get("PATH", "")
run(["npm", "run", "build"], cwd=ui, env=env, stdout=subprocess.DEVNULL)

boards = sorted({BOARDS[s] for s in scenes if BOARDS.get(s)})
if boards:
    run([sys.executable, "-I", os.path.join(root, "tools", "design", "render_mocks.py"), mocks, *boards],
        stdout=subprocess.DEVNULL)

port = free_port()
preview = subprocess.Popen(
    [os.path.join(ui, "node_modules", ".bin", "vite"), "preview", "--port", str(port), "--strictPort", "--host", "127.0.0.1"],
    cwd=ui, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(100):
        try:
            urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=1)
            break
        except OSError:
            time.sleep(0.1)
    else:
        sys.exit("vite preview did not start")

    for scene in scenes:
        for w, h, scale in ONLY.get(scene) or [(w, h, s) for w, h in SIZES for s in SCALES]:
            stem = f"{scene}-{w}x{h}-{scale}"
            shot = os.path.join(shots, stem + ".png")
            if scene in CLICKS:
                base, selector = CLICKS[scene]
                selectors = json.dumps([selector] if isinstance(selector, str) else list(selector))
                done = subprocess.run(["node", "--input-type=module", "-e", CLICK_SHOT, str(free_port()),
                                       f"http://127.0.0.1:{port}/?scene={base}&scale={scale}", selectors, shot, str(w), str(h)],
                                      env=env, capture_output=True, text=True, timeout=180)
                if done.returncode != 0:
                    sys.exit(f"{stem}: {done.stderr.strip() or done.returncode}")
            else:
                run(["google-chrome", "--headless=new", "--disable-gpu", "--hide-scrollbars",
                     "--force-device-scale-factor=1", "--virtual-time-budget=3000",
                     f"--window-size={w},{h}", f"--screenshot={shot}",
                     f"http://127.0.0.1:{port}/?scene={scene}&scale={scale}"],
                    capture_output=True, timeout=60)
            left = Image.open(shot).convert("RGB")
            board = BOARDS.get(scene)
            right = Image.new("RGB", (w, h), (40, 0, 40))
            if board and os.path.exists(os.path.join(mocks, board + ".png")):
                m = Image.open(os.path.join(mocks, board + ".png")).convert("RGB")
                right.paste(m.crop((0, 0, min(w, m.width), min(h, m.height))), (0, 0))
            sheet = Image.new("RGB", (w * 2 + 8, h + 24), (255, 0, 255))
            sheet.paste(left, (0, 24))
            sheet.paste(right, (w + 8, 24))
            d = ImageDraw.Draw(sheet)
            d.text((4, 6), f"screen  {stem}", fill=(0, 0, 0))
            d.text((w + 12, 6), f"mock  {board or '-'} (100 %)", fill=(0, 0, 0))
            sheet.save(os.path.join(out, stem + ".png"))
            print(os.path.join(out, stem + ".png"))
finally:
    preview.terminate()
    preview.wait()
