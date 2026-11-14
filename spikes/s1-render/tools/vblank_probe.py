#!/usr/bin/env python3
"""Calls DRM_IOCTL_WAIT_VBLANK the same way WebKitGTK's DisplayVBlankMonitorDRM does, on every primary node."""
import errno, fcntl, glob, os, struct, time

DRM_IOCTL_WAIT_VBLANK = 0xC018643A  # _IOWR('d', 0x3a, union drm_wait_vblank) (24 bytes)
DRM_VBLANK_RELATIVE = 0x1
for node in sorted(glob.glob("/dev/dri/card*")):
    try:
        fd = os.open(node, os.O_RDWR | os.O_CLOEXEC)
    except OSError as e:
        print(f"{node}: open failed: {e}"); continue
    for crtc in range(2):
        req = struct.pack("IIQ", DRM_VBLANK_RELATIVE | (0x2 if crtc == 1 else 0), 1, 0) + b"\0" * 8
        t = time.monotonic()
        try:
            fcntl.ioctl(fd, DRM_IOCTL_WAIT_VBLANK, bytearray(req))
            print(f"{node} crtc{crtc}: OK (waited {1000 * (time.monotonic() - t):.2f} ms)")
        except OSError as e:
            print(f"{node} crtc{crtc}: {errno.errorcode.get(e.errno, e.errno)} {e.strerror}")
    os.close(fd)
