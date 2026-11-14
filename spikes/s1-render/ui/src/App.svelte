<script lang="ts">
  import { onMount, flushSync } from "svelte";
  import { makeNotes, KEYS, LOW, KEYS_W, ROW, RULER_H, VEL_H, isBlack, type Roll } from "./bench/model";
  import { makeRoll2d } from "./bench/roll2d";
  import { makeRollGl } from "./bench/rollgl";
  import { run } from "./bench/harness";
  import { report } from "./bridge";

  const q = new URLSearchParams((window as any).__BENCH ?? location.search);
  const renderer = q.get("renderer") ?? "2d";
  const meters = q.get("meters") === "1";
  const durMs = Number(q.get("dur") ?? 10000);
  const warmMs = Number(q.get("warm") ?? 1500);
  const displayMs = Number(q.get("display") ?? 1000 / 60);

  // Spec 3: input meter is 52 when the window is 960+ tall, 24 when a lower band is open.
  const tall = innerHeight >= 960;
  const inputH = tall ? 52 : 24;
  const fixedH = 48 + inputH + 40 + 32 + 24 + 24 + 24;
  const mixerH = meters ? Math.max(0, Math.min(432, innerHeight - fixedH - 268)) : 0;

  const STRIPS = 9; // 8 channels + master
  let levels = $state(Array.from({ length: STRIPS * 2 }, () => 0));
  let peaks = $state(Array.from({ length: STRIPS }, () => "−∞"));
  let lit = $state(Array.from({ length: KEYS }, () => false));
  let pos = $state("1. 1. 000");
  let vel = $state(0), bend = $state(0), mod = $state(0), sus = $state(false);
  let master = $state([0, 0]);

  let main: HTMLCanvasElement, ruler: HTMLCanvasElement, keys: HTMLCanvasElement, area: HTMLDivElement;
  let status = $state("running");

  function drawKeys(h: number) {
    const dpr = devicePixelRatio;
    keys.width = KEYS_W * dpr; keys.height = h * dpr;
    const c = keys.getContext("2d")!; c.setTransform(dpr, 0, 0, dpr, 0, 0);
    c.fillStyle = "#1A1A1A"; c.fillRect(0, 0, KEYS_W, h);
    const rows = Math.floor((h - RULER_H - VEL_H) / ROW);
    const top = LOW + KEYS - 1 - Math.max(0, Math.floor((KEYS - rows) / 2));
    for (let r = 0; r < rows; r++) {
      const p = top - r;
      c.fillStyle = isBlack(p) ? "#0A0A0A" : "#D6D6D6";
      c.fillRect(64, RULER_H + r * ROW, 40, ROW - 1);
      if (p % 12 === 0) { c.fillStyle = "#9A9A9A"; c.font = "12px monospace"; c.fillText(`C${p / 12 - 1}`, 8, RULER_H + r * ROW + 10); }
    }
  }

  function timerRes() {
    let best = Infinity;
    for (let i = 0; i < 50; i++) { const a = performance.now(); let b = a; while (b === a) b = performance.now(); best = Math.min(best, b - a); }
    return best;
  }
  function glInfo() {
    const gl = document.createElement("canvas").getContext("webgl2");
    if (!gl) return null;
    const ext = gl.getExtension("WEBGL_debug_renderer_info");
    return { renderer: ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER), version: gl.getParameter(gl.VERSION) };
  }

  let seed = 1;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  function perFrame(t: number) {
    if (meters) {
      for (let i = 0; i < levels.length; i++) levels[i] = Math.max(0, Math.min(1, 0.55 + 0.4 * Math.sin(t * 3 + i) + 0.1 * rnd()));
      for (let i = 0; i < STRIPS; i++) peaks[i] = (20 * Math.log10(levels[i * 2] + 1e-6)).toFixed(1);
      master = [levels[0], levels[1]];
      const chord = Math.floor(t * 8);
      for (let k = 0; k < KEYS; k++) lit[k] = (k * 7 + chord) % 13 === 0;
      vel = 40 + (chord * 37) % 87; bend = Math.round(8191 * Math.sin(t)); mod = Math.round(63 + 63 * Math.sin(t / 3)); sus = chord % 4 < 2;
    }
    const ticks = Math.floor(t * 1920 * 0.8);
    pos = `${Math.floor(ticks / 1920) + 1}. ${Math.floor((ticks % 1920) / 480) + 1}. ${String(ticks % 480).padStart(3, "0")}`;
    flushSync();
  }

  onMount(async () => {
    await document.fonts.ready;
    const notes = makeNotes();
    let roll: Roll;
    try { roll = renderer === "gl" ? makeRollGl(main, ruler, notes) : makeRoll2d(main, notes); }
    catch (e) { await report({ error: String(e) }); return; }
    let size = { w: 0, h: 0 };
    const fit = () => {
      size = { w: area.clientWidth, h: area.clientHeight };
      roll.resize(size.w, size.h, devicePixelRatio);
      drawKeys(size.h);
    };
    fit();
    new ResizeObserver(fit).observe(area);
    const r = await run({ roll, size: () => size, durMs, warmMs, displayMs, perFrame });
    status = "done";
    await report({
      ...r, renderer, meters, inner: [innerWidth, innerHeight], dpr: devicePixelRatio, rollSize: size, mixerH,
      ua: navigator.userAgent, isolated: crossOriginIsolated, timerResMs: timerRes(), glRenderer: glInfo(),
    });
  });
</script>

<div class="app">
  <div class="transport">
    <span class="w s6">はじめての曲</span>
    <div class="readout"><span class="l">小節 . 拍 . ティック</span><span class="m t16 w">{pos}</span></div>
    <span class="x1"></span>
    <div class="mm"><div class="hbar"><div style="width:{master[0] * 100}%"></div></div><div class="hbar"><div style="width:{master[1] * 100}%"></div></div></div>
  </div>
  <div class="input" style="height:{inputH}px">
    <span class="w">鍵盤 49</span>
    <div class="kb" style="height:{tall ? 32 : 12}px">
      {#each lit as on, k}<div class="key" class:black={isBlack(LOW + k)} class:on></div>{/each}
    </div>
    <span class="m w">{vel}</span><span class="m">{sus ? "ON" : "OFF"}</span><span class="m">{bend}</span><span class="m">{mod}</span>
  </div>
  <div class="band" style="height:40px">アレンジ 全体</div>
  <div class="band" style="height:32px">▾ ピアノロール ・ ノート 10000</div>
  <div class="roll">
    <canvas bind:this={keys} style="width:{KEYS_W}px"></canvas>
    <div class="area" bind:this={area}>
      <canvas bind:this={main} class="fill"></canvas>
      {#if renderer === "gl"}<canvas bind:this={ruler} class="ruler"></canvas>{/if}
    </div>
  </div>
  <div class="band" style="height:24px">▸ サステイン ▸ ピッチベンド ▸ モジュレーション</div>
  {#if meters}
    <div class="mixer" style="height:{mixerH}px">
      {#each Array(STRIPS) as _, i}
        <div class="strip">
          <span class="w">{i < 8 ? `${i + 1} トラック` : "マスター"}</span>
          <div class="meters">
            {#each [0, 1] as ch}
              <div class="vbar">
                <div class="ok" style="height:{Math.min(levels[i * 2 + ch], 0.72) * 100}%"></div>
                <div class="hot" style="bottom:72%;height:{Math.max(0, levels[i * 2 + ch] - 0.72) * 100}%"></div>
              </div>
            {/each}
          </div>
          <span class="m">{peaks[i]} dB</span>
        </div>
      {/each}
    </div>
  {/if}
  <div class="band" style="height:24px">▸ アレンジ ▾ ピアノロール ▸ ミキサー ▸ 取り消しの履歴</div>
  <div class="status">{status} ・ {renderer} ・ meters {meters ? "on" : "off"}</div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: #121212; overflow: hidden; }
  .app { display: flex; flex-direction: column; height: 100vh; color: #D6D6D6; font: 12px/16px 'IBM Plex Sans JP', 'Noto Sans JP', sans-serif; }
  .m { font-family: 'IBM Plex Mono', monospace; font-variant-numeric: tabular-nums; }
  .w { color: #F2F2F2; } .l { color: #9A9A9A; } .s6 { font-weight: 600; } .t16 { font-size: 16px; line-height: 20px; } .x1 { flex: 1; }
  .transport { height: 48px; flex-shrink: 0; display: flex; align-items: center; gap: 12px; padding: 0 12px; background: #1A1A1A; border-bottom: 1px solid #2C2C2C; box-sizing: border-box; }
  .readout { display: flex; flex-direction: column; background: #0A0A0A; padding: 0 12px; }
  .mm { width: 132px; display: flex; flex-direction: column; gap: 4px; }
  .hbar { height: 4px; background: #0A0A0A; } .hbar div { height: 100%; background: #4CD964; }
  .input { flex-shrink: 0; display: flex; align-items: center; gap: 20px; padding: 0 12px; background: #0A0A0A; border-bottom: 1px solid #3A3A3A; box-sizing: border-box; }
  .kb { display: flex; gap: 1px; }
  .key { width: 7px; height: 100%; background: #1A1A1A; } .key.black { background: #0A0A0A; } .key.on { background: #FFB000; }
  .band { flex-shrink: 0; display: flex; align-items: center; padding: 0 12px; background: #222222; border-top: 1px solid #3A3A3A; box-sizing: border-box; }
  .roll { flex: 1; min-height: 0; display: flex; }
  .area { flex: 1; min-width: 0; position: relative; }
  .fill { position: absolute; inset: 0; width: 100%; height: 100%; }
  .ruler { position: absolute; left: 0; top: 0; width: 100%; height: 20px; }
  .mixer { flex-shrink: 0; display: flex; gap: 0; background: #1A1A1A; border-top: 1px solid #3A3A3A; }
  .strip { width: 108px; display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 8px 0; border-right: 1px solid #2C2C2C; }
  .meters { flex: 1; display: flex; gap: 2px; }
  .vbar { width: 6px; height: 100%; background: #0A0A0A; position: relative; }
  .ok, .hot { position: absolute; left: 0; right: 0; }
  .ok { bottom: 0; background: #4CD964; } .hot { background: #FFD60A; }
  .status { height: 24px; flex-shrink: 0; display: flex; align-items: center; padding: 0 12px; background: #0A0A0A; border-top: 1px solid #2C2C2C; }
</style>
