// Development only (MELOET_MEASURE_DIR and MELOET_UI_SCRIPT=watch): writes down the engine's load as the screen
// receives it and how many WebGL contexts the page has made and holds. It only reads.

import { contextsMade } from "../gl/pool";
import { onTelemetry } from "../state/app.svelte";

export const WATCH_REPORT = "app-watch.json";
const REPORT_EVERY_MS = 2000;

interface Second {
  /** Seconds since the watch started. */
  t: number;
  /** Telemetry messages in this second. */
  n: number;
  /** The load (thousandths of a block) over those messages. */
  loadMean: number;
  loadMax: number;
}

/** Starts the watch; returns the function that stops it. */
export function startWatch(report: (name: string, json: string) => Promise<void>): () => void {
  const began = performance.now();
  const seconds: Second[] = [];
  let at = 0;
  let n = 0;
  let sum = 0;
  let max = 0;
  const close = (): void => {
    if (n > 0) seconds.push({ t: at, n, loadMean: Math.round((sum / n) * 10) / 10, loadMax: max });
    n = 0;
    sum = 0;
    max = 0;
  };
  const stopTelemetry = onTelemetry((t) => {
    const now = Math.floor((performance.now() - began) / 1000);
    if (now !== at) {
      close();
      at = now;
    }
    n += 1;
    sum += t.loadPermille;
    max = Math.max(max, t.loadPermille);
  });
  const contexts = () => {
    const inPage = [...document.querySelectorAll("canvas")];
    // getContext on a canvas that has a context hands back the same one.
    const lost = inPage.filter((c) => c.getContext("webgl2")?.isContextLost() ?? false).length;
    return { made: contextsMade(), inPage: inPage.length, lostInPage: lost };
  };
  const send = (): void => {
    const json = JSON.stringify({
      // Wall clock (ms since the epoch) of the page's time 0, to place the frame profile's times after the launch.
      timeOriginMs: performance.timeOrigin,
      watchStartedMs: began,
      window: { width: window.innerWidth, height: window.innerHeight, dpr: window.devicePixelRatio },
      contexts: contexts(),
      seconds,
    });
    void report(WATCH_REPORT, json).catch(() => {});
  };
  const timer = window.setInterval(send, REPORT_EVERY_MS);
  return () => {
    clearInterval(timer);
    stopTelemetry();
  };
}
