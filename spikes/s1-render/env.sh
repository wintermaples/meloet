#!/usr/bin/env bash
# Records the machine state the numbers depend on.
export DISPLAY=${DISPLAY:-:0}
echo "date: $(date -Iseconds)"
echo "os: $(. /etc/os-release; echo "$PRETTY_NAME")"
echo "kernel: $(uname -r)"
echo "gpu: $(lspci | grep -E 'VGA|3D' | sed 's/^[^:]*: //' | paste -sd ';')"
echo "nvidia: $(head -1 /proc/driver/nvidia/version 2>/dev/null)"
echo "gl (Xwayland): $(glxinfo -B 2>/dev/null | grep -E 'OpenGL renderer|OpenGL version' | sed 's/.*: //' | paste -sd ';')"
echo "session: GNOME $(gnome-shell --version 2>/dev/null | awk '{print $3}'), Wayland (Xwayland present: $(pgrep -x Xwayland >/dev/null && echo yes || echo no))"
echo "monitors (xrandr via Xwayland):"; xrandr --current 2>/dev/null | grep -E " connected|\*" | sed 's/^/  /'
echo "webkit2gtk-4.1: $(dpkg-query -W -f='${Version}' libwebkit2gtk-4.1-0)"
echo "pipewire: $(dpkg-query -W -f='${Version}' pipewire)"
echo "mesa: $(dpkg-query -W -f='${Version}' libegl-mesa0:amd64 2>/dev/null)"
echo "vblank probe:"; python3 -I "$(dirname "$0")/tools/vblank_probe.py" | sed 's/^/  /'
