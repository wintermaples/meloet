# s1-render summary

display period 6.951 ms (144 Hz). 60 Hz cadence = 16.667 ms.

| env | size | renderer | meters | rAF p50 | interval mean / p95 / max (ms) | work mean / p95 / max (ms) | ≤1 refresh @144Hz | p95 ≤ 1 frame @60Hz | note |
|---|---|---|---|---|---|---|---|---|---|
| all3 | 1280x800 | 2d | off | 16 | 16.733 / 22 / 27 | 5.554 / 19 / 24 | 0 % | NO | wayland |
| all3 | 1280x800 | 2d | on | 16 | 16.166 / 17 / 20 | 4.102 / 13 / 15 | 0 % | yes | wayland |
| all3 | 1280x800 | gl | off | 16 | 16.063 / 17 / 17 | 0.3 / 1 / 1 | 0 % | yes | wayland |
| all3 | 1280x800 | gl | on | 16 | 16.063 / 17 / 17 | 0.496 / 1 / 2 | 0 % | yes | wayland |
| all3 | 1920x1080 | 2d | off | 16 | 17.64 / 27 / 32 | 6.12 / 22 / 27 | 0 % | NO | wayland |
| all3 | 1920x1080 | 2d | on | 16 | 16.954 / 23 / 25 | 4.703 / 16 / 18 | 0 % | NO | wayland |
| all3 | 1920x1080 | gl | off | 16 | 16.063 / 17 / 17 | 0.422 / 1 / 2 | 0 % | yes | wayland |
| all3 | 1920x1080 | gl | on | 16 | 16.063 / 17 / 17 | 0.604 / 1 / 3 | 0 % | yes | wayland |
| base | 1280x800 | 2d | off | 16 | 16.773 / 23 / 26 | 5.657 / 20 / 23 | 0 % | NO | wayland |
| base | 1280x800 | 2d | on | 16 | 16.168 / 17 / 20 | 4.111 / 13 / 15 | 0 % | yes | wayland |
| base | 1280x800 | gl | off | 16 | 16.061 / 17 / 17 | 0.337 / 1 / 1 | 0 % | yes | wayland |
| base | 1280x800 | gl | on | 16 | 16.063 / 17 / 17 | 0.518 / 1 / 3 | 0 % | yes | wayland |
| base | 1920x1080 | 2d | off | 16 | 17.561 / 27 / 33 | 6.074 / 22 / 26 | 0 % | NO | wayland |
| base | 1920x1080 | 2d | on | 16 | 16.997 / 24 / 26 | 4.761 / 16 / 19 | 0 % | NO | wayland |
| base | 1920x1080 | gl | off | 16 | 16.064 / 17 / 17 | 0.431 / 1 / 2 | 0 % | yes | wayland |
| base | 1920x1080 | gl | on | 16 | 16.063 / 17 / 17 | 0.588 / 1 / 2 | 0 % | yes | wayland |
| dmabufoff+nocomp | 1280x800 | 2d | off | 16 | 16.771 / 22 / 26 | 5.658 / 19 / 24 | 0 % | NO | wayland |
| dmabufoff+nocomp | 1280x800 | 2d | on | 16 | 16.192 / 18 / 19 | 4.145 / 13 / 14 | 0 % | yes | wayland |
| dmabufoff+nocomp | 1280x800 | gl | off | 16 | 16.069 / 17 / 17 | 0.326 / 1 / 1 | 0 % | yes | wayland |
| dmabufoff+nocomp | 1280x800 | gl | on | 16 | 16.064 / 17 / 17 | 0.51 / 1 / 3 | 0 % | yes | wayland |
| dmabufoff+nocomp | 1920x1080 | 2d | off | 16 | 17.641 / 28 / 41 | 6.06 / 22 / 33 | 0 % | NO | wayland |
| dmabufoff+nocomp | 1920x1080 | 2d | on | 16 | 17.078 / 24 / 28 | 4.823 / 17 / 20 | 0 % | NO | wayland |
| dmabufoff+nocomp | 1920x1080 | gl | off | 16 | 16.063 / 17 / 17 | 0.434 / 1 / 3 | 0 % | yes | wayland |
| dmabufoff+nocomp | 1920x1080 | gl | on | 16 | 16.069 / 17 / 17 | 0.621 / 1 / 3 | 0 % | yes | wayland |
| dmabufoff+nvsync | 1280x800 | 2d | off | 16 | 16.756 / 23 / 26 | 5.564 / 20 / 23 | 0 % | NO | wayland |
| dmabufoff+nvsync | 1280x800 | 2d | on | 16 | 16.193 / 17 / 21 | 4.118 / 13 / 16 | 0 % | yes | wayland |
| dmabufoff+nvsync | 1280x800 | gl | off | 16 | 16.064 / 17 / 17 | 0.342 / 1 / 1 | 0 % | yes | wayland |
| dmabufoff+nvsync | 1280x800 | gl | on | 16 | 16.063 / 17 / 17 | 0.539 / 1 / 2 | 0 % | yes | wayland |
| dmabufoff+nvsync | 1920x1080 | 2d | off | 16 | 17.619 / 28 / 34 | 6.167 / 22 / 28 | 0 % | NO | wayland |
| dmabufoff+nvsync | 1920x1080 | 2d | on | 16 | 17.066 / 24 / 27 | 4.775 / 17 / 20 | 0 % | NO | wayland |
| dmabufoff+nvsync | 1920x1080 | gl | off | 16 | 16.064 / 17 / 17 | 0.404 / 1 / 2 | 0 % | yes | wayland |
| dmabufoff+nvsync | 1920x1080 | gl | on | 16 | 16.064 / 17 / 17 | 0.593 / 1 / 3 | 0 % | yes | wayland |
| dmabufoff | 1280x800 | 2d | off | 16 | 16.772 / 23 / 25 | 5.66 / 20 / 22 | 0 % | NO | wayland |
| dmabufoff | 1280x800 | 2d | on | 16 | 16.166 / 17 / 19 | 4.124 / 13 / 14 | 0 % | yes | wayland |
| dmabufoff | 1280x800 | gl | off | 16 | 16.064 / 17 / 17 | 0.329 / 1 / 1 | 0 % | yes | wayland |
| dmabufoff | 1280x800 | gl | on | 16 | 16.064 / 17 / 17 | 0.531 / 1 / 2 | 0 % | yes | wayland |
| dmabufoff | 1920x1080 | 2d | off | 16 | 17.611 / 29 / 34 | 6.136 / 23 / 28 | 0 % | NO | wayland |
| dmabufoff | 1920x1080 | 2d | on | 16 | 17.036 / 24 / 27 | 4.809 / 17 / 19 | 0 % | NO | wayland |
| dmabufoff | 1920x1080 | gl | off | 16 | 16.064 / 17 / 17 | 0.421 / 1 / 2 | 0 % | yes | wayland |
| dmabufoff | 1920x1080 | gl | on | 16 | 16.064 / 17 / 17 | 0.639 / 1 / 3 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1280x800 | 2d | off | 17 | 16.667 / 18 / 26 | 1.659 / 5 / 6 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1280x800 | 2d | on | 17 | 16.667 / 17 / 27 | 1.303 / 4 / 5 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1280x800 | gl | off | 17 | 16.667 / 17 / 25 | 0.276 / 1 / 2 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1280x800 | gl | on | 17 | 16.669 / 17 / 27 | 0.441 / 1 / 3 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1920x1080 | 2d | off | 17 | 16.666 / 17 / 20 | 1.85 / 6 / 7 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1920x1080 | 2d | on | 17 | 16.667 / 18 / 24 | 1.574 / 5 / 6 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1920x1080 | gl | off | 17 | 16.669 / 18 / 27 | 0.341 / 1 / 2 | 0 % | yes | wayland |
| forcedmabuf+nvsync | 1920x1080 | gl | on | 17 | 16.669 / 18 / 31 | 0.522 / 1 / 3 | 0 % | yes | wayland |
| forcedmabuf | 1280x800 | 2d | off | — | — | — | — | — | no result (exit 1) |
| forcedmabuf | 1280x800 | 2d | on | — | — | — | — | — | no result (exit 1) |
| forcedmabuf | 1280x800 | gl | off | — | — | — | — | — | no result (exit 1) |
| forcedmabuf | 1280x800 | gl | on | — | — | — | — | — | no result (exit 1) |
| forcedmabuf | 1920x1080 | 2d | off | — | — | — | — | — | no result (exit 1) |
| forcedmabuf | 1920x1080 | 2d | on | — | — | — | — | — | no result (exit 1) |
| forcedmabuf | 1920x1080 | gl | off | — | — | — | — | — | no result (exit 1) |
| forcedmabuf | 1920x1080 | gl | on | — | — | — | — | — | no result (exit 1) |
| nocomp | 1280x800 | 2d | off | 16 | 16.782 / 23 / 27 | 5.643 / 20 / 25 | 0 % | NO | wayland |
| nocomp | 1280x800 | 2d | on | 16 | 16.218 / 18 / 20 | 4.162 / 14 / 16 | 0 % | yes | wayland |
| nocomp | 1280x800 | gl | off | 16 | 16.067 / 17 / 17 | 0.329 / 1 / 1 | 0 % | yes | wayland |
| nocomp | 1280x800 | gl | on | 16 | 16.061 / 17 / 17 | 0.53 / 1 / 2 | 0 % | yes | wayland |
| nocomp | 1920x1080 | 2d | off | 16 | 17.646 / 28 / 34 | 6.139 / 22 / 28 | 0 % | NO | wayland |
| nocomp | 1920x1080 | 2d | on | 16 | 17.041 / 23 / 26 | 4.733 / 16 / 19 | 0 % | NO | wayland |
| nocomp | 1920x1080 | gl | off | 16 | 16.063 / 17 / 17 | 0.404 / 1 / 2 | 0 % | yes | wayland |
| nocomp | 1920x1080 | gl | on | 16 | 16.069 / 17 / 17 | 0.631 / 1 / 3 | 0 % | yes | wayland |
| nvsync+nocomp | 1280x800 | 2d | off | 16 | 16.738 / 22 / 25 | 5.604 / 20 / 22 | 0 % | NO | wayland |
| nvsync+nocomp | 1280x800 | 2d | on | 16 | 16.192 / 18 / 20 | 4.145 / 13 / 15 | 0 % | yes | wayland |
| nvsync+nocomp | 1280x800 | gl | off | 16 | 16.061 / 17 / 17 | 0.323 / 1 / 1 | 0 % | yes | wayland |
| nvsync+nocomp | 1280x800 | gl | on | 16 | 16.063 / 17 / 17 | 0.543 / 1 / 3 | 0 % | yes | wayland |
| nvsync+nocomp | 1920x1080 | 2d | off | 16 | 17.52 / 28 / 30 | 6.042 / 22 / 25 | 0 % | NO | wayland |
| nvsync+nocomp | 1920x1080 | 2d | on | 16 | 17.051 / 24 / 28 | 4.75 / 16 / 20 | 0 % | NO | wayland |
| nvsync+nocomp | 1920x1080 | gl | off | 16 | 16.063 / 17 / 17 | 0.4 / 1 / 2 | 0 % | yes | wayland |
| nvsync+nocomp | 1920x1080 | gl | on | 16 | 16.064 / 17 / 17 | 0.62 / 1 / 3 | 0 % | yes | wayland |
| nvsync | 1280x800 | 2d | off | 16 | 16.741 / 22 / 25 | 5.624 / 19 / 22 | 0 % | NO | wayland |
| nvsync | 1280x800 | 2d | on | 16 | 16.166 / 17 / 19 | 4.144 / 13 / 15 | 0 % | yes | wayland |
| nvsync | 1280x800 | gl | off | 16 | 16.063 / 17 / 17 | 0.319 / 1 / 1 | 0 % | yes | wayland |
| nvsync | 1280x800 | gl | on | 16 | 16.064 / 17 / 17 | 0.533 / 1 / 2 | 0 % | yes | wayland |
| nvsync | 1920x1080 | 2d | off | 16 | 17.546 / 28 / 32 | 6.019 / 22 / 26 | 0 % | NO | wayland |
| nvsync | 1920x1080 | 2d | on | 16 | 16.98 / 23 / 29 | 4.727 / 16 / 22 | 0 % | NO | wayland |
| nvsync | 1920x1080 | gl | off | 16 | 16.064 / 17 / 17 | 0.415 / 1 / 2 | 0 % | yes | wayland |
| nvsync | 1920x1080 | gl | on | 16 | 16.067 / 17 / 17 | 0.613 / 1 / 3 | 0 % | yes | wayland |
| x11 | 1280x800 | 2d | off | 16 | 16.744 / 22 / 26 | 5.579 / 20 / 23 | 0 % | NO | x11 |
| x11 | 1280x800 | 2d | on | 16 | 16.217 / 18 / 22 | 4.168 / 13 / 17 | 0 % | yes | x11 |
| x11 | 1280x800 | gl | off | 16 | 16.063 / 17 / 17 | 0.319 / 1 / 1 | 0 % | yes | x11 |
| x11 | 1280x800 | gl | on | 16 | 16.061 / 17 / 17 | 0.482 / 1 / 2 | 0 % | yes | x11 |
| x11 | 1920x1080 | 2d | off | 16 | 17.483 / 27 / 32 | 5.99 / 22 / 26 | 0 % | NO | x11 |
| x11 | 1920x1080 | 2d | on | 16 | 16.932 / 23 / 26 | 4.628 / 16 / 18 | 0 % | NO | x11 |
| x11 | 1920x1080 | gl | off | 16 | 16.064 / 17 / 17 | 0.392 / 1 / 2 | 0 % | yes | x11 |
| x11 | 1920x1080 | gl | on | 16 | 16.064 / 17 / 17 | 0.589 / 1 / 3 | 0 % | yes | x11 |

## Resize stress (8 s of set_size every 120 ms, GL + meters, 1920x1080)

| env | survived | note |
|---|---|---|
| all3 | yes |  |
| base | yes |  |
| dmabufoff+nocomp | yes |  |
| dmabufoff+nvsync | yes |  |
| dmabufoff | yes |  |
| forcedmabuf+nvsync | yes |  |
| forcedmabuf | NO | exit 1 |
| nocomp | yes |  |
| nvsync+nocomp | yes |  |
| nvsync | yes |  |
| x11 | yes |  |
