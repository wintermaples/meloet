import { connect, cpuSample, devices, finish, onRust, rustClock, type Msg, type Transport } from "./bridge";

const q = new URLSearchParams((window as any).__S4 ?? location.search);
const mode = q.get("mode") ?? "latency";
const transport = (q.get("transport") ?? "chan-bin") as Transport;
const hz = Number(q.get("hz") ?? 60);
const durMs = Number(q.get("dur") ?? 15000);
const LOW = 36, KEYS = 49;

const $ = (id: string) => document.getElementById(id)!;
const keys: HTMLElement[] = [];
for (let k = 0; k < KEYS; k++) {
  const d = document.createElement("div");
  d.className = "k" + ([1, 3, 6, 8, 10].includes((LOW + k) % 12) ? " b" : "");
  $("kb").appendChild(d); keys.push(d);
}
const meters: HTMLElement[] = [];
for (let i = 0; i < 18; i++) { const m = document.createElement("div"); m.className = "m"; m.appendChild(document.createElement("i")); $("meters").appendChild(m); meters.push(m.firstChild as HTMLElement); }
$("mode").textContent = `${mode} ・ ${transport} ・ ${hz} Hz`;
const log = (s: string) => { $("log").textContent += s + "\n"; };

const jsNow = () => performance.timeOrigin + performance.now();
let offsetMs = 0; // rust_ms = js_ms + offsetMs
async function syncClock() {
  let best = Infinity;
  for (let i = 0; i < 60; i++) {
    const a = jsNow(); const r = await rustClock(); const b = jsNow();
    if (b - a < best) { best = b - a; offsetMs = r / 1e6 - (a + b) / 2; }
  }
  return best;
}
const toRustMs = (js: number) => js + offsetMs;

type KeyRec = { note: number; on: boolean; tNs: number; recvMs: number; rafMs?: number; handlerMs: number };
const keyRecs: KeyRec[] = [];
const frameLag: number[] = [];
let handlerTotal = 0, frames = 0;

function lightKey(on: boolean, note: number, vel: number, tNs: number) {
  const recv = toRustMs(jsNow());
  const t0 = performance.now();
  const el = keys[note - LOW];
  if (el) el.classList.toggle("on", on);
  $("vel").textContent = String(vel);
  const rec: KeyRec = { note, on, tNs, recvMs: recv, handlerMs: performance.now() - t0 };
  keyRecs.push(rec);
  requestAnimationFrame((ts) => { rec.rafMs = toRustMs(performance.timeOrigin + ts); });
}

function applyFrame(meterVals: ArrayLike<number>, pos: number, tNs: number) {
  const t0 = performance.now();
  for (let i = 0; i < 18; i++) meters[i].style.height = `${meterVals[i] * 100}%`;
  $("pos").textContent = pos.toFixed(3);
  handlerTotal += performance.now() - t0; frames++;
  frameLag.push(toRustMs(jsNow()) - tNs / 1e6);
}

function parseBin(buf: ArrayBuffer) {
  const v = new DataView(buf);
  let o = 0;
  while (o < buf.byteLength) {
    const type = v.getUint8(o);
    if (type === 1) { lightKey(v.getUint8(o + 1) === 1, v.getUint8(o + 2), v.getUint8(o + 3), Number(v.getBigUint64(o + 8, true))); o += 16; }
    else if (type === 2) {
      const tNs = Number(v.getBigUint64(o + 16, true));
      const pos = v.getFloat64(o + 40, true);
      const m = new Float32Array(buf.slice(o + 48, o + 48 + 72));
      applyFrame(m, pos, tNs);
      o += 120;
    } else break;
  }
}

function onMsg(m: Msg) {
  if (m.bin) return parseBin(m.bin);
  const j = m.json;
  if (j.kind === "note") lightKey(j.on, j.note, j.vel, j.t_ns);
  else if (j.meters) applyFrame(j.meters, j.position, j.t_ns);
}

async function main() {
  const devs = await devices();
  if (mode === "devices") { await finish({ mode, devs }); return; }
  const plug: any[] = [];
  onRust("device", (d) => { plug.push({ ...d, uiMs: toRustMs(jsNow()) }); $("dev").textContent = d.connected ? d.name : "鍵盤なし"; log(JSON.stringify(d)); });
  onRust("udev", (d) => { plug.push({ udev: d, uiMs: toRustMs(jsNow()) }); });
  const rtt = await syncClock();
  log(`clock sync rtt ${rtt} ms`);
  const pull = await connect(transport, mode === "load" || q.has("hz") ? hz : 0, onMsg);
  const rafIv: number[] = [];
  let last = 0, running = true;
  const loop = async (ts: number) => {
    if (last) rafIv.push(ts - last);
    last = ts;
    if (transport === "pull") parseBin(await pull());
    if (running) requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
  const c0 = await cpuSample();
  await new Promise((r) => setTimeout(r, durMs));
  const c1 = await cpuSample();
  running = false;
  await finish({ mode, transport, hz, durMs, clockRttMs: rtt, offsetMs, keys: keyRecs, frameLagMs: frameLag, frames, handlerTotalMs: handlerTotal, rafIntervals: rafIv, cpu: [c0, c1], plug, devs });
}
main().catch((e) => finish({ error: String(e) }));
