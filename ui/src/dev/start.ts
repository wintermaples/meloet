// Development only: the piano roll as the target of an input script (script.ts). Loaded with import() only
// when Rust hands over MELOET_UI_SCRIPT while measuring, so it is not in the page's usual code path.
// It only reads where things are; the input itself goes through the note area's own event handlers.

import { metrics } from "../components/metrics.svelte";
import * as G from "../components/pianoroll/geom";
import { shown } from "../components/pianoroll/shown.svelte";
import { shownFor } from "../components/pianoroll/shownNotes";
import { dragReading, prView, pxPerTick, rowHeight } from "../components/pianoroll/view.svelte";
import { current } from "../state/app.svelte";
import { browserClock, parseUiScript, SCRIPT_REPORT, UiScript, type Grab, type Target } from "./script";

const AREA = ".roll .notes > .area";
/** A drag moves the note at least this far (CSS px), whole grid steps. */
const DRAG_MIN_PX = 12;

function geom(): G.RollGeom {
  return {
    scrollTick: prView.scrollTick,
    ppt: pxPerTick(metrics.scale),
    topKey: prView.topKey,
    rowH: rowHeight(metrics.scale),
    width: prView.areaWidth,
    height: prView.areaHeight,
  };
}

/** The visible note nearest the middle whose body (not the resize edge) can be grabbed. */
function grab(area: HTMLElement): Grab | null {
  const n = shownFor(shown.take, shown.clip);
  const state = current.state;
  if (!n || !state) return null;
  const g = geom();
  const edge = metrics.px("part.note.resize");
  const step = G.gridTicks(state.pianoRoll.grid, { num: state.song.meterNum, den: state.song.meterDen });
  const stepPx = step * g.ppt;
  const dx = stepPx * Math.max(1, Math.ceil(DRAG_MIN_PX / stepPx));
  const r = area.getBoundingClientRect();
  const midX = g.width / 2;
  const midY = g.height / 2;
  let best: { x: number; y: number; d: number } | null = null;
  G.eachNoteIn(n, G.xToTick(g, dx), G.xToTick(g, g.width - dx), (i) => {
    const y = G.keyToY(g, n.take.keys[i]!) + g.rowH / 2;
    if (y < g.rowH || y > g.height - g.rowH) return;
    const x0 = G.tickToX(g, G.noteStart(n, i));
    const x1 = G.tickToX(g, G.noteEnd(n, i));
    const x = Math.max(x0, 0) + 1;
    if (x < dx || x > g.width - 2 * dx || x >= x1 - Math.min(edge, (x1 - x0) / 2)) return;
    const hit = G.hitNote(n, g, x, y, edge);
    if (!hit || hit.edge) return;
    const d = Math.hypot(x - midX, y - midY);
    if (!best || d < best.d) best = { x, y, d };
  });
  const found = best as { x: number; y: number } | null;
  return found ? { x: r.left + found.x, y: r.top + found.y, dx } : null;
}

function pianoRoll(): Target {
  const area = () => document.querySelector<HTMLElement>(AREA);
  return {
    area,
    ready: () => {
      const a = area();
      return a !== null && prView.areaWidth > 0 && prView.areaHeight > 0 && G.noteCount(shownFor(shown.take, shown.clip)) > 0;
    },
    grab: () => {
      const a = area();
      return a ? grab(a) : null;
    },
    scrollTick: () => prView.scrollTick,
    zoom: () => prView.quarterPx,
    rowPx: () => prView.rowPx,
    shownTake: () => shown.take?.id ?? null,
    reading: () => dragReading.text,
  };
}

/** Starts the script; returns the function that stops it. A script that does not read is reported and does nothing. */
export function startUiScript(text: string, report: (name: string, json: string) => Promise<void>): () => void {
  const parts = parseUiScript(text);
  if (!parts) {
    void report(SCRIPT_REPORT, JSON.stringify({ script: text, done: false, error: "the script did not read", parts: [] })).catch(() => {});
    return () => {};
  }
  const script = new UiScript(text, parts, pianoRoll(), browserClock, report);
  void script.run();
  return () => script.stop();
}
