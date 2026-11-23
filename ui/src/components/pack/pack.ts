// How the place to fetch the sound pack is shown (spec 2 "Hint band (sound pack)", spec 3 43-B, 44-A, 45-A;
// mock F8-PackAsk, F8-PackGet, F8-PackFail, Opt8-PackShow, R12-Fit). Pure functions that do not know the DOM.

import type { AppState, PackFailure, PackFetchView, SoundsView } from "../../bridge";
import { STATUS_JOIN, t } from "../../text";

const MB = 1_000_000;
/** MB is 1000 × 1000 bytes (the same as mb in the status line). */
export const mb = (bytes: number): number => Math.round(bytes / MB);
/** The size of the whole pack (「約 240 MB」「ぜんぶあります ・ 240 MB」) is rounded to 10 MB (240 MB in the spec). */
export const packMb = (bytes: number): number => Math.round(bytes / (10 * MB)) * 10;
/** The % fetched. Not 100 until fetching is done. */
export const percentOf = (done: number, total: number): number =>
  total <= 0 ? 0 : done >= total ? 100 : Math.min(99, Math.floor((done / total) * 100));
/** Minutes of 「のこり 約 2 分」 (1 even under 1 minute). */
export const minutesOf = (seconds: number): number => Math.max(1, Math.round(seconds / 60));

/**
 * Whether a hint of flows 1–4 is shown: the hints right of the input meter (no keyboard, unplugged, stopped, no output yet),
 * and above the arrange, the output chooser and hints like 「音が出ました」.
 */
export function flowHintUp(app: AppState): boolean {
  return lastingHintUp(app) || passingHintUp(app);
}

/** Hints that stay (no keyboard, unplugged, input stopped). They do not go away until a keyboard is plugged in. */
export function lastingHintUp(app: AppState): boolean {
  if (app.keyboards.inputStopped) return true;
  return !app.keyboards.list.some((k) => k.id === app.keyboards.active);
}

/**
 * Hints that end if you wait (the output chooser and no output yet). The output chooser shows only with a keyboard, so no output yet without a keyboard is not counted.
 * 「音が出ました」 and the empty arrange show inside the arrange and stay until a clip is made, so do not wait for them (docs/history/design-notes.ja.md B-95-12).
 */
function passingHintUp(app: AppState): boolean {
  if (app.workspace.chooser !== null) return true;
  return app.outputs.chosen === null && !lastingHintUp(app);
}

export type PackBand =
  | { kind: "ask" }
  | { kind: "fetching"; fetch: Extract<PackFetchView, { state: "fetching" }> }
  | { kind: "failed"; fetch: Extract<PackFetchView, { state: "failed" }> };

/** The hint shown in the input meter band (44-A). Not shown while a flow hint is shown (asking waits; while fetching it goes to the status line). */
export function packBand(app: AppState): PackBand | null {
  if (flowHintUp(app)) return null;
  const f = app.sounds.fetch;
  if (f.state === "fetching") return { kind: "fetching", fetch: f };
  if (f.state === "failed") return { kind: "failed", fetch: f };
  return app.sounds.ask ? { kind: "ask" } : null;
}

/** Progress moved to the status line (the 44-B form; Opt8-PackShow): while fetching and a flow hint is shown. */
export function packInStatus(app: AppState): Extract<PackFetchView, { state: "fetching" }> | null {
  const f = app.sounds.fetch;
  return f.state === "fetching" && flowHintUp(app) ? f : null;
}

/**
 * Whether to ask in the status line (following the 44-B form): while asking, only hints that stay are shown.
 * Nothing is fetched until answered, so the way to ask must not be lost. While hints that end if you wait are shown, wait, then show it in the band.
 */
export function askInStatus(app: AppState): boolean {
  return app.sounds.ask && app.sounds.fetch.state === "idle" && lastingHintUp(app) && !passingHintUp(app);
}

/** The 2 lines of the asking band. short: the small text when it does not fit (mock:R12-Fit). */
export function askLines(sounds: SoundsView, short: boolean): [string, string] {
  return [
    t("pack.ask", { mb: packMb(sounds.packBytes) }),
    short ? t("pack.askDetailShort") : t("pack.askDetail", { count: sounds.total }),
  ];
}

/** The 2 lines of the fetching band (the spec 6 text split in two; the time left is not shown until known). */
export function fetchingLines(f: { doneBytes: number; totalBytes: number; remainingS: number | null }): [string, string] {
  const bytes = t("pack.fetchingBytes", { percent: percentOf(f.doneBytes, f.totalBytes), done: mb(f.doneBytes), total: mb(f.totalBytes) });
  const left = f.remainingS === null ? "" : `${STATUS_JOIN}${t("pack.fetchingLeft", { minutes: minutesOf(f.remainingS) })}`;
  return [t("pack.fetching"), bytes + left];
}

/**
 * The 2 lines of the failed band. Line 1 is the text of mock F8-PackFail. Line 2 is the mock text when not connected,
 * otherwise the spec 6 text for each reason (the ones shown in 「状態行 ・ 取りにいく所」).
 */
export function failedLines(failure: PackFailure, doneBytes: number, totalBytes: number): [string, string] {
  const detail = (() => {
    switch (failure.kind) {
      case "offline":
        return t("pack.failedOfflineDetail", { done: mb(doneBytes), total: mb(totalBytes) });
      case "cutOff":
        return t("pack.cutOff", { done: mb(doneBytes), total: mb(totalBytes) });
      case "diskFull":
        return t("pack.diskFull", { mb: mb(Math.max(0, failure.neededBytes - failure.freeBytes)) });
      case "corrupt":
        return t("pack.corrupt");
    }
  })();
  return [t("pack.failedTitle"), detail];
}

/** On failure, the button in the chosen form (「続きから」 when cut off on the way). */
export const againLabel = (failure: PackFailure): string => t(failure.kind === "cutOff" ? "button.resumeFetch" : "button.again");
