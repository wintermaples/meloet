# The AppImage and the LGPL: does meloet's licence change? (2026-11-24)

Asked by the owner about B-98-2: "It is fine as long as meloet itself does not become GPL / LGPL. If it would, what should we do?" Researched by meloet-research on 2026-11-24. Confidence: A = primary source, B = secondary source or own observation, C = personal posts or our own inference. We are not lawyers; the points marked "lawyer" need one.

## Short answer

- **meloet's own code stays MIT OR Apache-2.0 (A).** Shipping unchanged LGPL libraries next to it, linked dynamically, does not put meloet under the LGPL or the GPL, as long as meloet's terms allow changes for the person's own use and reverse engineering to debug them (MIT and Apache-2.0 do). LGPL-2.1 sections 5 and 6, LGPL-3 section 4, the FSF's FAQ.
- **The deb has no such duty (A).** It uses the libraries that are already on the PC (FSF FAQ, "LGPLStaticVsDynamic").
- **The AppImage does.** Whoever ships the libraries must ship them the LGPL's way: the licence texts, the libraries' source (with it, by a written offer of three years, or from the same place), and a way for the person to replace the library.
- **The changed `libwebkit2gtk` is a modified LGPL library (A, a reading of LGPL-2.1 section 2).** It stays LGPL; a notice of the change and its date is added to the duties, and the source of the changed form (the original source plus the scripts that make the change).
- **A library under the plain GPL would reach meloet (A, FSF FAQ "IfLibraryIsGPL")** if a meloet process links to it. The list of the about 170 bundled libraries is in no document and the AppImage could not be unpacked during the research, so this is **not checked**. `libjbig` (GPL-2+) is already left out by the build script.
- **The runtime's libfuse does not reach meloet (A leaning to B).** The type-2 runtime links libfuse (LGPL-2.1, with a small patch) statically and starts the payload as another process. The one who ships the runtime still owes the LGPL duties for it.

## The patch

tauri-bundler 2.10.6 holds `linuxdeploy-plugin-gtk.sh`; it runs `sed -i` over every `libwebkit*` file under `usr/lib*` of the AppDir and replaces the text `/usr` by `././` (four bytes by four bytes, every path starting with `/usr`), which is why `AppRun` changes into `usr/` (A, the script itself).

`WEBKIT_EXEC_PATH` is no way around it: WebKit reads it only in `ENABLE(DEVELOPER_MODE)` builds (A, `ProcessExecutablePathGLib.cpp` at tag webkitgtk-2.52.0), and the library of this PC holds no such text (B, our own search; Ubuntu 26.04, not the 24.04 of the build container). Other ways (a mount namespace, an `LD_PRELOAD` shim, building WebKitGTK ourselves) are not tried (C), and avoiding the patch would only remove the notice of change, not the duty to give the source.

## What a release of the AppImage would need (from the licence texts, A; the list is our reading)

1. The licence texts: LGPL-2.1, LGPL-3, LGPL-2, GPL-2, GPL-3 and the others (MPL, AFL …). Some libraries are "LGPL-3+ or GPL-2+" (libgmp, nettle, libidn2's library); WebKit holds files under LGPL-2 only.
2. A table made by the build: package, exact version, licence, where the source is.
3. The sources themselves in the same place as the AppImage, and a written offer.
4. The notice for `libwebkit2gtk`: that it was changed, the date, the `sed` expression, the script.
5. The source of libfuse 3.15.0 with the runtime's patch, and of the type-2 runtime.
6. A plain notice that the libraries are used and are under the LGPL (the about dialog, the README), and how to replace one (unpack with `--appimage-extract`, swap the file, run `AppRun` or pack again).
7. Before any of that: unpack the AppImage, read every `usr/share/doc/*/copyright`, and make the build stop on a library under the plain GPL or AGPL (as it does for `libjbig`); check that no GStreamer "bad" / "ugly" plugin and no `libreadline` (GPL-3+) is inside.

Open points for a lawyer: whether "can be unpacked and the file swapped" meets the relink rule (no primary source found, C); whether the source may sit at another place than the binary under LGPL-2.1 6(d).

## Choices

| Choice | Cost |
|---|---|
| (a) No AppImage: deb / apt only | None. Loses the one-file form; architecture 23-5's reasons against the AppImage still hold. |
| (b) AppImage with the pack above | Work at every build, a large release (WebKitGTK's source alone is large), the open points above. |
| (c) Avoid the patch | Little gain: only the notice of change goes. |
| (d) Flatpak with the GNOME runtime | The libraries come from the runtime (A, Tauri's and Flathub's documents), but plugins of the PC (`/usr/lib/vst3`, `/usr/lib/lv2`) are not seen from the sandbox (C, users' reports), which is the core of a DAW. |

The lead's advice to the owner: (a) for now.

## Sources

A:
- LGPL-2.1: https://www.gnu.org/licenses/old-licenses/lgpl-2.1.txt (sections 0, 1, 2, 4, 5, 6)
- LGPL-3: https://www.gnu.org/licenses/lgpl-3.0.en.html (section 4)
- FSF GPL FAQ: https://www.gnu.org/licenses/gpl-faq.html (LGPLStaticVsDynamic, IfLibraryIsGPL, MereAggregation, SourceAndBinaryOnDifferentSites)
- WebKit's licence page: https://webkit.org/licensing-webkit/ ; `/usr/share/doc/libwebkit2gtk-4.1-0/copyright` of this PC (Ubuntu 26.04). They differ in words (Debian: "BSD 2-clause by default", WebKit: "LGPL and BSD parts"); both list LGPL parts.
- The patch: https://raw.githubusercontent.com/tauri-apps/tauri/tauri-bundler-v2.10.6/crates/tauri-bundler/src/bundle/linux/appimage/linuxdeploy-plugin-gtk.sh
- WebKit: https://raw.githubusercontent.com/WebKit/WebKit/webkitgtk-2.52.0/Source/WebKit/Shared/glib/ProcessExecutablePathGLib.cpp
- The runtime: https://github.com/AppImage/type2-runtime (LICENSE, patches/libfuse) ; https://raw.githubusercontent.com/libfuse/libfuse/fuse-3.15.0/LICENSE ; https://github.com/AppImage/AppImageSpec/blob/master/draft.md ; https://docs.appimage.org/user-guide/run-appimages.html
- Flatpak: https://v2.tauri.app/distribute/flatpak/ ; https://docs.flathub.org/blog/enhanced-license-compliance-tools
- The copyright files of libidn2, libgmp, nettle, libdbus, libmount, libreadline on this PC (26.04, not 24.04).

B: Ubuntu's `debian/rules` of webkit2gtk (no DEVELOPER_MODE); our search of this PC's `libwebkit2gtk-4.1.so.0`; Ubuntu Discourse on AppArmor and user namespaces.

C: forum posts on AppImage and the LGPL (discourse.appimage.org t/1629, forum.palemoon.org t=33451); AppImageKit issue 267; reports on Flatpak DAWs and host plugins; our own inferences.

No primary source was found for: "extract and swap" meeting the relink rule; "naming the tool and the exact change" being enough as the source of the patch.
