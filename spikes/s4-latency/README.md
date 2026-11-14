# s4-latency — bet 4, key response

Tauri app: ALSA sequencer input with hot-plug (System:Announce), a placeholder sine synth on cpal/PipeWire
(no allocation or locks in the callback), and four ways to send data to the page. `vkeys` is a stand-in keyboard.

```bash
(cd ui && npm ci && npm run build) && cargo build --release -p s4-latency
./run.sh devices          # output devices per cpal host
./run.sh latency [256]    # 120 virtual keys per transport: key -> screen, key -> sound
./run.sh latency-load     # same while meters and position stream at 60 Hz
./run.sh load             # per-frame streaming cost, 60 and 144 Hz
./run.sh plug 90          # plug and unplug the keyboard while it runs
./run.sh real 60          # press real keys while it runs
python3 summarize.py      # -> results/summary.md
```

Buffer 256 needs PipeWire's quantum lowered for the run (`pw-metadata -n settings 0 clock.force-quantum 256`, then back).
