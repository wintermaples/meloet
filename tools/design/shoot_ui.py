#!/usr/bin/env python3
"""Shoot the screen (fake scenes) and put each shot next to its mock board in one PNG.

usage: python3 -I tools/design/shoot_ui.py OUT_DIR [SCENE ...]

Runs `npm run build` in ui/, serves ui/dist with `vite preview`, and shoots each scene with
headless google-chrome at 1280x800 and 1920x1080, scale 100 and 150. Next to each shot it puts
the mock board for that scene (rendered by render_mocks.py), cropped or padded to the same size.
Out: OUT_DIR/<scene>-<w>x<h>-<scale>.png (side by side) and OUT_DIR/shots/ (the shots alone).
"""
import os, socket, subprocess, sys, time, urllib.request

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
    "longName": "F-MeterLongName",
    "pianoRoll": "A-PianoRoll",
    "mixer": "A-Mixer",
    "history": "F-History",
    "shelf": "F-ShelfArrange",
    "params": "F-ParamFew",
}
SIZES = [(1280, 800), (1920, 1080)]
SCALES = [100, 150]
scenes = sys.argv[2:] or list(BOARDS)


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
        for w, h in SIZES:
            for scale in SCALES:
                stem = f"{scene}-{w}x{h}-{scale}"
                shot = os.path.join(shots, stem + ".png")
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
