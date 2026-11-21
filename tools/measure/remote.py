"""Drives the real app (release build) from outside, one step at a time.

The other tools here give the app a timetable (MELOET_COMMANDS). This one decides its next step
from what it sees: it writes lines into the app's MELOET_PIPE (an intent for the core, or
JavaScript for the page), and the page answers through the development command
`measure_report`, which writes a file into MELOET_MEASURE_DIR.

    from remote import App
    with App(out, song="x.meloet") as app:
        app.send({"type": "toggleBand", "band": "mixer"})
        snap = app.snapshot()                      # the whole state, as the screen gets it
        n = app.js("return document.querySelectorAll('button').length")
        app.wait(lambda s: s["windows"]["open"], 5)

Every start has its own config and state folders, a virtual keyboard of its own (so that the
output chooser comes up and is confirmed), and XDG_CACHE_HOME = <target>/shoot/cache (the
record of one whole plugin scan; the first start makes it and takes about 20 s).
Also here: reading /proc (RSS, PSS, CPU), and `X`, the tool's own look at the X server (find a
plugin's window, give it the keyboard, made-up pointer events for the window alone).
Changes nothing on the PC.
"""
import ctypes, ctypes.util, json, os, shutil, signal, subprocess, sys, tempfile, time

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(root, "target")
target = os.path.join(root, target) if not os.path.isabs(target) else target
app_bin = os.path.join(target, "release", "meloet")
vkeys_bin = os.path.join(target, "release", "meloet-vkeys")
plughost_bin = os.path.join(target, "release", "meloet-plughost")
scan_cache = os.path.join(target, "shoot", "cache")
scan_json = os.path.join(scan_cache, "meloet", "plugins", "scan.json")
SF2_SOURCE = "/usr/share/sounds/sf2/FluidR3_GM.sf2"

TICK = os.sysconf("SC_CLK_TCK")
PAGE = os.sysconf("SC_PAGE_SIZE")


def now_ms():
    return time.time() * 1000


# ---- /proc -------------------------------------------------------------------------------

def children(pid):
    out = []
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            stat = open(f"/proc/{p}/stat").read()
        except OSError:
            continue
        if int(stat[stat.rindex(")") + 2:].split()[1]) == pid:
            out.append(int(p))
    return out


def comm(pid):
    try:
        return open(f"/proc/{pid}/comm").read().strip()
    except OSError:
        return None


def descendants(pid):
    out, todo = [], [pid]
    while todo:
        for c in children(todo.pop()):
            out.append(c)
            todo.append(c)
    return out


def mem(pid):
    """{"rssMb", "pssMb", "cpuS", "threads"} of one process, None when it is gone."""
    try:
        stat = open(f"/proc/{pid}/stat").read()
        rest = stat[stat.rindex(")") + 2:].split()
        rss = int(open(f"/proc/{pid}/statm").read().split()[1]) * PAGE / 1048576
    except (OSError, ValueError, IndexError):
        return None
    pss = None
    try:
        for line in open(f"/proc/{pid}/smaps_rollup"):
            if line.startswith("Pss:"):
                pss = round(int(line.split()[1]) / 1024, 1)
    except OSError:
        pass
    return {"rssMb": round(rss, 1), "pssMb": pss, "cpuS": round((int(rest[11]) + int(rest[12])) / TICK, 2),
            "threads": int(rest[17])}


def plughosts(parent=None):
    """pids of `meloet-plughost host ...` (those under `parent` when given), with their args."""
    found = {}
    under = set(descendants(parent)) if parent else None
    for p in os.listdir("/proc"):
        if not p.isdigit() or (under is not None and int(p) not in under):
            continue
        try:
            args = open(f"/proc/{p}/cmdline", "rb").read().split(b"\0")
        except OSError:
            continue
        if len(args) >= 2 and args[0].decode(errors="replace") == plughost_bin and args[1] == b"host":
            found[int(p)] = [a.decode(errors="replace") for a in args[2:] if a]
    return found


# ---- the app -----------------------------------------------------------------------------

def agent_env():
    """This PC only: the agent user reaches the screen and the sound through env.sh."""
    env = dict(os.environ)
    path = os.path.expanduser("~/.config/meloet-agent/env.sh")
    if os.path.exists(path):
        out = subprocess.run(["bash", "-c", f"source {path}; env -0"], capture_output=True).stdout
        for item in out.split(b"\0"):
            if b"=" in item:
                k, v = item.decode(errors="replace").split("=", 1)
                env[k] = v
        env.setdefault("PIPEWIRE_RUNTIME_DIR", "/run/user/1000")
        env.setdefault("DISPLAY", ":0")
    return env


def sf2_dir():
    folder = os.path.join(target, "shoot", "sf2")
    if not os.path.exists(os.path.join(folder, os.path.basename(SF2_SOURCE))):
        os.makedirs(folder, exist_ok=True)
        shutil.copy(SF2_SOURCE, folder)
    return folder


def make_song(path, *args):
    subprocess.run([sys.executable, "-I", os.path.join(root, "tools", "measure", "make_song.py"), path,
                    "--scan", scan_json, *[a.replace("{sf2}", SF2_SOURCE) for a in args]],
                   check=True, stdout=subprocess.DEVNULL)
    return path


class RemoteError(Exception):
    pass


class App:
    def __init__(self, out, song=None, window="1920x1080", env=None, keyboard=True, confirm=True,
                 sf2=False, settle_s=1.0, song_in_place=False):
        self.out = out
        self.raw = os.path.join(out, "raw")
        os.makedirs(self.raw, exist_ok=True)
        self.work = tempfile.mkdtemp(prefix="meloet-remote-")
        self.measure = os.path.join(self.work, "measure")
        self.fifo = os.path.join(self.work, "pipe")
        for d in ("config", "state", "measure", "song", "sf2"):
            os.makedirs(os.path.join(self.work, d))
        os.mkfifo(self.fifo)
        self.song = None
        if song and song_in_place:
            # A song with audio is its folder: opened where it is, and a save changes it there.
            self.song = os.path.abspath(song)
        elif song:
            self.song = os.path.join(self.work, "song", os.path.basename(song))
            shutil.copy(song, self.song)
        self.vkeys_name = f"meloet-vkeys-remote-{os.getpid()}"
        self.env = agent_env()
        self.env.update(MELOET_CONFIG_DIR=os.path.join(self.work, "config"),
                        MELOET_STATE_DIR=os.path.join(self.work, "state"),
                        MELOET_MEASURE_DIR=self.measure, MELOET_PIPE=self.fifo, MELOET_WINDOW=window,
                        MELOET_ONLY_VIRTUAL="1", MELOET_VKEYS_NAME=self.vkeys_name,
                        MELOET_SONGS_DIR=os.path.join(self.work, "song"),
                        MELOET_SF2_DIR=sf2_dir() if sf2 else os.path.join(self.work, "sf2"),
                        XDG_CACHE_HOME=scan_cache)
        if self.song:
            self.env["MELOET_OPEN_SONG"] = self.song
        self.env.update(env or {})
        self.keyboard, self.confirm, self.settle_s = keyboard, confirm, settle_s
        self.ids = 0
        self.keys = self.launcher = self.pipe = self.pid = None
        self.stderr = open(os.path.join(self.raw, "meloet.stderr"), "wb")

    def __enter__(self):
        try:
            self.start()
        except BaseException:
            self.close()
            raise
        return self

    def __exit__(self, *exc):
        self.close()

    def start(self):
        if self.keyboard:
            self.keys = subprocess.Popen([vkeys_bin, "appear", "--name", self.vkeys_name, "--seconds", "86400"],
                                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=self.env)
            time.sleep(1)
        self.launched_ms = now_ms()
        self.launcher = subprocess.Popen(["dbus-run-session", "--", app_bin], env=self.env,
                                         stdout=self.stderr, stderr=subprocess.STDOUT)
        until = time.time() + 20
        while time.time() < until and not self.pid:
            self.pid = next((c for c in children(self.launcher.pid) if comm(c) == "meloet"), None)
            time.sleep(0.05)
        if not self.pid:
            raise RemoteError("meloet did not start")
        # The app opens the pipe once the page has loaded; opening for writing waits for that.
        until = time.time() + 60
        while True:
            try:
                fd = os.open(self.fifo, os.O_WRONLY | os.O_NONBLOCK)
                break
            except OSError:
                if time.time() > until or self.launcher.poll() is not None:
                    raise RemoteError("the app did not open the pipe")
                time.sleep(0.05)
        os.set_blocking(fd, True)
        self.pipe = os.fdopen(fd, "w", encoding="utf-8")
        self.wait(lambda s: True, 30)
        with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "remote_page.js"), encoding="utf-8") as f:
            self.run_js(f.read())
        if self.keyboard and self.confirm:
            state = self.wait(lambda s: s["workspace"]["chooser"] or s["outputs"]["chosen"], 15,
                              "the output chooser")
            if state["workspace"]["chooser"]:
                self.send({"type": "chooserConfirm"})
                self.wait(lambda s: s["outputs"]["chosen"] and s["audio"]["running"], 10, "the output")
        if self.song:
            self.wait(lambda s: s["song"]["file"] != "neverSaved", 60, "the song to open")
        time.sleep(self.settle_s)
        return self

    # -- the two ways in

    def send(self, command):
        self.pipe.write(json.dumps({"command": command}, ensure_ascii=False) + "\n")
        self.pipe.flush()

    def run_js(self, code):
        """Runs JavaScript in the page and does not wait."""
        self.pipe.write(json.dumps({"js": code}, ensure_ascii=False) + "\n")
        self.pipe.flush()

    def js(self, code, timeout=20):
        """Runs the body of an async function (`invoke` is Tauri's) and returns what it returns."""
        self.ids += 1
        name = f"remote-{self.ids}.json"
        path = os.path.join(self.measure, name)
        self.run_js(
            "(async()=>{const inv=window.__TAURI_INTERNALS__.invoke;let r;"
            "try{r=await (async(invoke,k)=>{" + code + "\n})(inv,window.__remote)}"
            "catch(e){r={__error:String(e&&e.stack||e)}}"
            f"await inv('measure_report',{{name:'{name}',json:JSON.stringify(r===undefined?null:r)}})}})()")
        until = time.time() + timeout
        while not os.path.exists(path):
            if time.time() > until:
                raise RemoteError(f"the page did not answer in {timeout} s: {code[:200]}")
            if self.launcher.poll() is not None:
                raise RemoteError("the app has ended")
            time.sleep(0.004)
        with open(path, encoding="utf-8") as f:
            result = json.load(f)
        os.remove(path)
        if isinstance(result, dict) and "__error" in result:
            raise RemoteError(result["__error"])
        return result

    def snapshot(self):
        return self.js("return (await invoke('snapshot')).state")

    def wait(self, test, timeout=10, what="it", every=0.03):
        """The first state for which `test` holds (a test that raises counts as not yet)."""
        until = time.time() + timeout
        while True:
            state = self.snapshot()
            try:
                if test(state):
                    return state
            except (KeyError, IndexError, TypeError):
                pass
            if time.time() > until:
                raise RemoteError(f"waited {timeout} s for {what}")
            time.sleep(every)

    def status(self):
        """The text of the status line, as the screen shows it."""
        return self.js("return k.status()")

    def shot(self, png, timeout=30):
        """Writes what the WebView shows to the PNG (the stderr line tells when it is whole)."""
        if os.path.exists(png):
            os.remove(png)
        seen = os.path.getsize(self.stderr.name)
        self.pipe.write(json.dumps({"shot": png}) + "\n")
        self.pipe.flush()
        until = time.time() + timeout
        while time.time() < until:
            with open(self.stderr.name, "rb") as f:
                f.seek(seen)
                if b"MELOET_SNAPSHOT:" in f.read():
                    return png
            time.sleep(0.05)
        raise RemoteError(f"no shot in {timeout} s")

    # -- processes

    def web_pid(self):
        return next((c for c in descendants(self.pid) if comm(c) == "WebKitWebProces"), None)

    def hosts(self):
        return plughosts(self.pid)

    def memory(self):
        hosts = {str(p): mem(p) for p in self.hosts()}
        return {"wallMs": round(now_ms()), "meloet": mem(self.pid), "web": mem(self.web_pid()),
                "hosts": hosts}

    def alive(self):
        return self.launcher.poll() is None

    def play_keys(self, *args, wait=True, log=None):
        cmd = [vkeys_bin, "play", "--name", self.vkeys_name, *[str(a) for a in args]]
        if log:
            cmd += ["--log", log]
        p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=self.env)
        if wait:
            p.wait()
        return p

    def close(self, save_measure=True):
        """Ends the app the usual way (SIGTERM: the core stops and writes its measurements)."""
        status = None
        if self.launcher is not None:
            if self.pid and self.launcher.poll() is None:
                try:
                    os.kill(self.pid, signal.SIGTERM)
                except OSError:
                    pass
            try:
                status = self.launcher.wait(timeout=30)
            except subprocess.TimeoutExpired:
                for p in [self.pid, *descendants(self.pid)]:
                    try:
                        os.kill(p, signal.SIGKILL)
                    except OSError:
                        pass
                status = self.launcher.wait(timeout=10)
            self.launcher = None
        if self.pipe:
            try:
                self.pipe.close()
            except OSError:
                pass
            self.pipe = None
        if self.keys:
            self.keys.terminate()
            self.keys.wait()
            self.keys = None
        self.stderr.close()
        if save_measure and os.path.isdir(self.measure):
            for name in os.listdir(self.measure):
                if not name.startswith(("remote-", ".")):
                    shutil.copy(os.path.join(self.measure, name), os.path.join(self.raw, name))
        if self.song and os.path.exists(self.song):
            shutil.copy(self.song, os.path.join(self.raw, "saved.meloet"))
        shutil.rmtree(self.work, ignore_errors=True)
        self.exit_status = status
        return status


# ---- X -----------------------------------------------------------------------------------

class _XEvent(ctypes.Union):
    # XButtonEvent and XMotionEvent share this layout up to `same_screen`.
    class Button(ctypes.Structure):
        _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
                    ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("root", ctypes.c_ulong),
                    ("subwindow", ctypes.c_ulong), ("time", ctypes.c_ulong), ("x", ctypes.c_int),
                    ("y", ctypes.c_int), ("x_root", ctypes.c_int), ("y_root", ctypes.c_int),
                    ("state", ctypes.c_uint), ("button", ctypes.c_uint), ("same_screen", ctypes.c_int)]

    class Client(ctypes.Structure):
        _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
                    ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong),
                    ("format", ctypes.c_int), ("data", ctypes.c_long * 5)]

    _fields_ = [("button", Button), ("client", Client), ("pad", ctypes.c_long * 24)]


class X:
    """What `crates/meloet-plughost/tests/common/x11.rs` does, for a tool outside the tests."""

    def __init__(self, display=None):
        lib = ctypes.CDLL(ctypes.util.find_library("X11"))
        self.lib = lib
        lib.XOpenDisplay.restype = ctypes.c_void_p
        lib.XOpenDisplay.argtypes = [ctypes.c_char_p]
        self.d = lib.XOpenDisplay((display or os.environ.get("DISPLAY") or ":0").encode())
        if not self.d:
            raise RemoteError("no X display")
        lib.XDefaultRootWindow.restype = ctypes.c_ulong
        lib.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        lib.XInternAtom.restype = ctypes.c_ulong
        lib.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        lib.XQueryTree.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong),
                                   ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)),
                                   ctypes.POINTER(ctypes.c_uint)]
        lib.XGetWindowProperty.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_long,
                                           ctypes.c_long, ctypes.c_int, ctypes.c_ulong,
                                           ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_int),
                                           ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_ulong),
                                           ctypes.POINTER(ctypes.POINTER(ctypes.c_ubyte))]
        lib.XFree.argtypes = [ctypes.c_void_p]
        lib.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
        lib.XTranslateCoordinates.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int,
                                              ctypes.c_int, ctypes.POINTER(ctypes.c_int),
                                              ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_ulong)]
        lib.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long,
                                   ctypes.POINTER(_XEvent)]
        lib.XGetGeometry.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong),
                                     ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int),
                                     ctypes.POINTER(ctypes.c_uint), ctypes.POINTER(ctypes.c_uint),
                                     ctypes.POINTER(ctypes.c_uint), ctypes.POINTER(ctypes.c_uint)]
        lib.XFlush.argtypes = [ctypes.c_void_p]
        lib.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
        # A window that goes away between two requests must not end the tool (Xlib's default
        # error handler exits).
        self._handler = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)(lambda d, e: 0)
        lib.XSetErrorHandler(self._handler)
        self.root = lib.XDefaultRootWindow(self.d)
        atom = lambda name: lib.XInternAtom(self.d, name, 0)
        self.a_pid, self.a_name = atom(b"_NET_WM_PID"), atom(b"_NET_WM_NAME")
        self.a_protocols, self.a_delete = atom(b"WM_PROTOCOLS"), atom(b"WM_DELETE_WINDOW")

    def children(self, window):
        root, parent, n = ctypes.c_ulong(), ctypes.c_ulong(), ctypes.c_uint()
        kids = ctypes.POINTER(ctypes.c_ulong)()
        if not self.lib.XQueryTree(self.d, window, root, parent, kids, n):
            return []
        out = [kids[i] for i in range(n.value)]
        if kids:
            self.lib.XFree(kids)
        return out

    def _property(self, window, atom):
        kind, fmt = ctypes.c_ulong(), ctypes.c_int()
        n, after = ctypes.c_ulong(), ctypes.c_ulong()
        data = ctypes.POINTER(ctypes.c_ubyte)()
        if self.lib.XGetWindowProperty(self.d, window, atom, 0, 1024, 0, 0, kind, fmt, n, after, data) != 0:
            return None
        if not data:
            return None
        if fmt.value == 32:
            value = [ctypes.cast(data, ctypes.POINTER(ctypes.c_ulong))[i] for i in range(n.value)]
        else:
            value = bytes(data[i] for i in range(n.value))
        self.lib.XFree(data)
        return value

    def title(self, window):
        v = self._property(window, self.a_name)
        return v.decode(errors="replace") if isinstance(v, bytes) else None

    def windows_of(self, pid):
        """The titled windows process `pid` made: [(window, title)]."""
        out, level = [], [self.root]
        for _ in range(4):
            nxt = []
            for w in level:
                for c in self.children(w):
                    if self._property(c, self.a_pid) == [pid] and self.title(c):
                        out.append((c, self.title(c)))
                    nxt.append(c)
            level = nxt
        return out

    def find(self, pid, title=None):
        for w, t in self.windows_of(pid):
            if title is None or t == title:
                return w
        return None

    def size(self, window):
        root, x, y = ctypes.c_ulong(), ctypes.c_int(), ctypes.c_int()
        w, h, b, depth = ctypes.c_uint(), ctypes.c_uint(), ctypes.c_uint(), ctypes.c_uint()
        self.lib.XGetGeometry(self.d, window, root, x, y, w, h, b, depth)
        return w.value, h.value

    def focus(self, window):
        """Gives the keyboard to the window, as a click on it does."""
        self.lib.XSetInputFocus(self.d, window, 1, 0)  # RevertToPointerRoot, CurrentTime
        self.lib.XSync(self.d, 0)

    def unfocus(self):
        """Takes the keyboard away from every X window, as a click on a Wayland window does."""
        self.lib.XSetInputFocus(self.d, 0, 0, 0)
        self.lib.XSync(self.d, 0)

    def _translate(self, src, dst, x, y):
        dx, dy, child = ctypes.c_int(), ctypes.c_int(), ctypes.c_ulong()
        ok = self.lib.XTranslateCoordinates(self.d, src, dst, x, y, dx, dy, child)
        return (dx.value, dy.value, child.value) if ok else None

    def drag(self, window, x, y, dy, steps=10, pause=0.015, dx=0):
        """Tells the plugin's own window inside `window` that the left button went down at
        (x, y), moved by (dx, dy) in `steps` steps and came up. The real pointer is not touched."""
        target, at = window, (x, y)
        while True:
            r = self._translate(window, target, x, y)
            if r is None:
                break
            at = (r[0], r[1])
            if not r[2]:
                break
            target = r[2]
        if target == window:
            return False
        r = self._translate(window, self.root, x, y)
        root_at = (r[0], r[1]) if r else (0, 0)

        def send(kind, mask, mx, my, state):
            e = _XEvent()
            b = e.button
            b.type, b.display, b.window, b.root = kind, self.d, target, self.root
            b.x, b.y, b.x_root, b.y_root = at[0] + mx, at[1] + my, root_at[0] + mx, root_at[1] + my
            b.state, b.button, b.same_screen = state, (0 if kind == 6 else 1), 1
            self.lib.XSendEvent(self.d, target, 1, mask, ctypes.byref(e))
            self.lib.XFlush(self.d)
            time.sleep(pause)

        send(4, 1 << 2, 0, 0, 0)                                   # ButtonPress
        for step in range(1, steps + 1):
            send(6, (1 << 6) | (1 << 13), dx * step // steps, dy * step // steps, 1 << 8)  # MotionNotify
        send(5, 1 << 3, dx, dy, 1 << 8)                            # ButtonRelease
        return True

    def ask_to_close(self, window):
        """What the window's close button does."""
        e = _XEvent()
        c = e.client
        c.type, c.display, c.window, c.message_type, c.format = 33, self.d, window, self.a_protocols, 32
        c.data[0] = self.a_delete
        self.lib.XSendEvent(self.d, window, 0, 0, ctypes.byref(e))
        self.lib.XSync(self.d, 0)


# ---- the sound that went out ---------------------------------------------------------------

class Recorder:
    """Records the output's monitor (what went to the chosen output) into a WAV, as
    plugin-stop.sh does. `started_ms` is the wall clock just before pw-record was started; the
    first sample is some tens of ms after it.

    pw-record writes its file on its audio thread: with the file on a disk it lost three or
    four blocks every 5 s (the kernel's writeback). So it writes into memory (the runtime
    folder), and `stop` moves the file to `wav`."""

    def __init__(self, wav, env=None, fmt="s16"):
        """fmt "f32": the samples as the output got them (read with stretch_app.read_f32)."""
        env = env or agent_env()
        self.wav = wav
        runtime = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
        self.memory = tempfile.mkdtemp(prefix="meloet-rec-", dir=runtime) if os.access(runtime, os.W_OK) else None
        self.into = os.path.join(self.memory, "rec.wav") if self.memory else wav
        self.started_ms = now_ms()
        self.p = subprocess.Popen(["pw-record", "-P", "stream.capture.sink=true", "--format", fmt, "--rate", "48000",
                                   "--channels", "2", self.into], env=env,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def stop(self):
        self.p.send_signal(signal.SIGINT)
        try:
            self.p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.p.kill()
            self.p.wait()
        if self.memory:
            if os.path.exists(self.into):
                shutil.move(self.into, self.wav)
            shutil.rmtree(self.memory, ignore_errors=True)
            self.memory = None
        return self.wav


def read_wav(wav):
    """(rate, left, right) as float arrays in -1..1. pw-record leaves the lengths in the header
    unset when it is stopped by a signal, so the samples are read from after the header."""
    import numpy
    with open(wav, "rb") as f:
        raw = f.read()
    at = raw.find(b"data")
    data = numpy.frombuffer(raw[at + 8:len(raw) - ((len(raw) - at - 8) % 4)], dtype="<i2").astype(numpy.float64) / 32768
    return 48000, data[0::2], data[1::2]


def level_db(samples):
    import numpy
    if len(samples) == 0:
        return None
    rms = float(numpy.sqrt(numpy.mean(samples * samples)))
    return round(20 * numpy.log10(rms), 1) if rms > 0 else float("-inf")


def crossings_per_second(samples, rate=48000):
    """Upward zero crossings a second: a rough word for how high or bright the sound is."""
    import numpy
    if len(samples) < 2:
        return None
    up = numpy.count_nonzero((samples[:-1] < 0) & (samples[1:] >= 0))
    return round(up * rate / len(samples))
