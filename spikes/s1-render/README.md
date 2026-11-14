# s1-render — bet 1, screen speed

Tauri + Svelte page that draws a 10 000-note piano roll (Canvas 2D or WebGL) while scrolling and zooming by itself,
optionally with 9 mixer meters and the input meter updated every frame. It reports frame intervals and draw time.

```bash
(cd ui && npm ci && npm run build) && cargo build --release -p s1-render
./env.sh > results/env.txt                     # machine state
./run_matrix.sh && python3 summarize.py        # 11 env sets x 2 sizes x 2 renderers x meters + resize stress
python3 tools/vblank_probe.py                  # does the GPU answer DRM_IOCTL_WAIT_VBLANK?
# one run by hand:
../../target/release/s1-render --renderer gl --meters 1 --w 1920 --h 1080 --dur 10000 --display 6.951 --out /tmp/r.json
#   --driver tick --tickhz 143.87   draw on a clock sent from Rust instead of rAF
#   --resize-stress 8               resize the window every 120 ms for 8 s
#   --no60 1                        turn off WebKit's PreferPageRenderingUpdatesNear60FPS
```

Results: `results/summary.md`, raw JSON in `results/raw/`. `src/bridge.ts` is the only file that imports Tauri.
