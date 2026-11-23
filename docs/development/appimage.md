# The AppImage (a trial)

The deb is meloet's main form. The AppImage is a **trial**: one file that runs without
installing. It was refused before (architecture 23-5) because an AppImage carries its own
WebKitGTK, GTK and Wayland libraries, which then have to work with the PC's own graphics
driver (Mesa or NVIDIA). That worry is still true; see "Known limits" below.

Checked only on Ubuntu 26.04 (amd64, GNOME on Wayland, NVIDIA). Other Linux systems (Fedora,
Arch, older Ubuntu …) are **not checked**.

## Building

```bash
MELOET_DOCKER_SG=1 scripts/build-appimage.sh
```

It builds in the same Ubuntu 24.04 container as the deb (`packaging/Dockerfile`) and writes
`target/appimage/meloet_0.3.0_amd64.AppImage` (about 95 MB). `meloet-plughost` and
`meloet-mcp` go next to `meloet` in `usr/bin/`, as in the deb (`bundle.externalBin`). The
oldest glibc it runs on is 2.39 (Ubuntu 24.04's).

The tools tauri-cli's AppImage bundler would download (tauri-bundler 2.10.6, inside
@tauri-apps/cli 2.12.6) are downloaded by the script instead, each pinned by version and
sha256, and put in the bundler's cache (`~/.cache/tauri` of the container):

| Tool | Version | License | Where it ends up |
|---|---|---|---|
| `AppRun` (AppImageKit) | tauri-apps/binary-releases `apprun-old` | MIT | inside, as `AppRun.wrapped` |
| linuxdeploy | tauri-apps/binary-releases `linuxdeploy-07333c6` | MIT | build only |
| linuxdeploy-plugin-appimage (appimagetool, mksquashfs) | `1-alpha-20250213-1` | MIT (the tools in it: GPL, build only) | build only |
| AppImage type 2 runtime | AppImage/type2-runtime `20251108` | MIT, with libfuse 3.15.0 (LGPL-2.1), squashfuse 0.5.2 (BSD-2-Clause), zstd, zlib, mimalloc, musl linked in | the head of the AppImage file |

`libjbig` (GPL-2.0-or-later) is left out (`LINUXDEPLOY_EXCLUDED_LIBRARIES`); only GTK's TIFF
image loader needs it, and it takes the PC's copy. The script stops if it is found inside.

## What is inside

- `usr/bin/meloet`, `meloet-plughost`, `meloet-mcp` (MIT OR Apache-2.0), each with
  `RUNPATH=$ORIGIN/../lib`.
- `usr/lib/`: WebKitGTK 4.1, GTK 3, GLib, GStreamer's core libraries, liblilv and about 170
  other libraries from the Ubuntu 24.04 packages, with each package's copyright file in
  `usr/share/doc/<package>/copyright` (put there by linuxdeploy). Their licenses are mostly
  LGPL-2.1-or-later, MIT, BSD and similar. One is changed: in `libwebkit2gtk-4.1.so.0`
  linuxdeploy-plugin-gtk replaces the text `/usr` with `././`, so that WebKit finds its helper
  programs inside the AppImage (that is why `AppRun` moves into `usr/`).
- Not inside (taken from the PC): glibc, libstdc++, libGL / libEGL and the graphics drivers,
  libX11, libxcb, libwayland-client, fontconfig, freetype, harfbuzz, libasound, libpipewire,
  libdrm, libgbm, libz (linuxdeploy's exclude list).
- `usr/share/doc/meloet/`: `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `AppImage-libraries.txt`
  (the paragraph above). `usr/share/doc/appimage-runtime/`: what the runtime is made of and the
  license texts (copies in `packaging/appimage/licenses/`).

Before an AppImage is handed out, the LGPL libraries inside need their source offered from the
same place (LGPL-2.1 section 4); the Ubuntu source packages are the source. This is not done
yet; the AppImage is not published.

## Running

```bash
chmod +x meloet_0.3.0_amd64.AppImage
./meloet_0.3.0_amd64.AppImage
./meloet_0.3.0_amd64.AppImage path/to/song.meloet   # opens the song
```

It needs FUSE (`fusermount3`, Ubuntu's `fuse3` package). Without it:
`./meloet_0.3.0_amd64.AppImage --appimage-extract-and-run`.

- The AppImage does not register itself: no menu entry and no `.meloet` file type. Open songs
  by giving the file as the argument (a relative path is from the folder you started it in).
- Sound packs: as with the deb, next to the program first, else `$XDG_DATA_HOME/meloet/sounds`
  (the AppImage is read-only, so it is always the second).
- Plugins: the PC's own (`/usr/lib/vst3`, `/usr/lib/lv2`, `~/.vst3`, `~/.lv2` …).
- Settings, songs and the recovery file are in the same places as with the deb.

## Children run on the PC's libraries

The AppImage's `AppRun` and the GTK hook of linuxdeploy put `LD_LIBRARY_PATH`, `PATH`,
`XDG_DATA_DIRS`, `GTK_*`, `GIO_MODULE_DIR`, `GSETTINGS_SCHEMA_DIR`, `PYTHONHOME`, `QT_PLUGIN_PATH`,
`GST_PLUGIN_SYSTEM_PATH*` and others into meloet's environment, all pointing inside the
AppImage. A plugin loaded with those would use the AppImage's GTK and GLib. So when `APPIMAGE`
is set, `meloet_plugin::launch::spawn` (plugin hosts and scans) and `xdg-open` take out every
entry that lies inside `$APPDIR`, and `APPIMAGE`, `APPDIR`, `OWD`, `ARGV0`, `GTK_THEME`,
`PYTHONDONTWRITEBYTECODE` (`launch::outside_appimage`). `meloet-plughost` itself still finds
liblilv, libdbus and their libraries inside the AppImage through its `RUNPATH`; the plugins
get the PC's libX11, freetype, GLib, libstdc++.

## The AI link

The guide for the AI link names `/usr/bin/meloet-mcp`. With the AppImage, give the AI app the
AppImage itself with `--mcp`:

```json
{ "command": "/home/you/Apps/meloet_0.3.0_amd64.AppImage", "args": ["--mcp"] }
```

`meloet --mcp` becomes the `meloet-mcp` next to it (an `exec`), so it is the same bridge. The
deb's `/usr/bin/meloet-mcp` stays, and `meloet --mcp` works with the deb too.

## Known limits

- **Graphics** (why 23-5 refused it): the AppImage carries WebKitGTK 2.52 of Ubuntu 24.04 and
  `libwayland-egl`, `libwayland-server`, `libxkbcommon`, `libepoxy` with it, while EGL and the
  driver are the PC's. Tauri issues #15976 (Fedora 44, Mesa 26) and #15665 (Ubuntu 26.04) report
  `EGL_BAD_PARAMETER` and blank windows from this mix. On this PC (NVIDIA 595, GNOME Wayland)
  it works, but a newer Mesa may not.
- The WebKitGTK inside is not the PC's: the drawing path and timing meloet is tuned for
  (chapter 7) may differ from the deb. Key → screen and key → sound have not been measured with
  the AppImage yet.
- Not checked: other distributions, X11-only desktops other than XWayland, AMD and Intel
  graphics, a PC without `liblilv` (it is inside, so it should not matter).

## What was checked (2026-11-24, Ubuntu 26.04)

With `meloet_0.3.0_amd64.AppImage` (sha256 c1ae120a…a7eb) and a build of the same source with
the development hooks:

- Starts on Wayland and on X11 (XWayland); the window shows the whole screen (pictures with
  `MELOET_SNAPSHOT`), WebGL regions draw.
- The output opens (PipeWire stream `meloet` running); the plugin scan finishes (306 bundles,
  12 s).
- A song given as a relative argument opens; Surge XT (VST3) and ZamComp (LV2) load in
  `meloet-plughost` without the AppImage's variables and play (master −18.5 dB); the Surge XT
  window opens; the song saves (version 7) and opens again.
- `--mcp` answers alone, and with the AppImage meloet running and 「AI とつなぐ」 on it lists
  94 tools; also with `--appimage-extract-and-run`.
- Closing the window and SIGTERM end with status 0.
- Not checked: the sound pack download, built-in sounds with the pack, the latency.
