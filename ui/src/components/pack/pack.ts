// 音の素材の包みを取りにいく所の見せかた（仕様書 2 章「案内の帯（音の素材）」、3 章 43-B ・ 44-A ・ 45-A、
// モック F8-PackAsk ・ F8-PackGet ・ F8-PackFail ・ Opt8-PackShow ・ R12-Fit）。DOM を知らない純粋な関数。

import type { AppState, PackFailure, PackFetchView, SoundsView } from "../../bridge";
import { STATUS_JOIN, t } from "../../text/ja";

const MB = 1_000_000;
/** MB は 1000 × 1000 バイト（状態行の mb と同じ）。 */
export const mb = (bytes: number): number => Math.round(bytes / MB);
/** 包みぜんぶの大きさ（「約 240 MB」「ぜんぶあります ・ 240 MB」）は 10 MB にまるめる（仕様書の 240 MB）。 */
export const packMb = (bytes: number): number => Math.round(bytes / (10 * MB)) * 10;
/** 取れた分の %。取りおえるまでは 100 にしない。 */
export const percentOf = (done: number, total: number): number =>
  total <= 0 ? 0 : done >= total ? 100 : Math.min(99, Math.floor((done / total) * 100));
/** 「のこり 約 2 分」の分（1 分より短くても 1）。 */
export const minutesOf = (seconds: number): number => Math.max(1, Math.round(seconds / 60));

/**
 * 流れ 1〜4 の案内が出ているか: 入力計の右の案内（鍵盤がない ・ 抜けた ・ 止まっている ・ 出口がまだ）、
 * アレンジの上の出口えらびと「音が出ました」などの案内。
 */
export function flowHintUp(app: AppState): boolean {
  return lastingHintUp(app) || passingHintUp(app);
}

/** ずっと出ている案内（鍵盤がない ・ 抜けた ・ 入力が止まっている）。鍵盤をつながないかぎり消えない。 */
export function lastingHintUp(app: AppState): boolean {
  if (app.keyboards.inputStopped) return true;
  return !app.keyboards.list.some((k) => k.id === app.keyboards.active);
}

/**
 * 待てば終わる案内（出口えらびと出口がまだ）。出口えらびは鍵盤があるときだけ出るので、鍵盤がなくて出口がまだのときは数えない。
 * 「音が出ました」「空のアレンジ」はアレンジの中に出て、クリップができるまで消えないので待たない（design-notes B-95-12）。
 */
function passingHintUp(app: AppState): boolean {
  if (app.workspace.chooser !== null) return true;
  return app.outputs.chosen === null && !lastingHintUp(app);
}

export type PackBand =
  | { kind: "ask" }
  | { kind: "fetching"; fetch: Extract<PackFetchView, { state: "fetching" }> }
  | { kind: "failed"; fetch: Extract<PackFetchView, { state: "failed" }> };

/** 入力計の帯に出す案内（44-A）。流れの案内が出ている間は出さない（聞くのは待ち、取っている間は状態行へ）。 */
export function packBand(app: AppState): PackBand | null {
  if (flowHintUp(app)) return null;
  const f = app.sounds.fetch;
  if (f.state === "fetching") return { kind: "fetching", fetch: f };
  if (f.state === "failed") return { kind: "failed", fetch: f };
  return app.sounds.ask ? { kind: "ask" } : null;
}

/** 状態行に移した進み（44-B の形。Opt8-PackShow）: 取っている間で、流れの案内が出ているとき。 */
export function packInStatus(app: AppState): Extract<PackFetchView, { state: "fetching" }> | null {
  const f = app.sounds.fetch;
  return f.state === "fetching" && flowHintUp(app) ? f : null;
}

/**
 * 状態行で聞くか（44-B の形にならう）: 聞いている間に、ずっと出ている案内だけが出ているとき。
 * 答えるまで取らないので、聞く入口をなくさない。待てば終わる案内の間は、待ってから帯に出す。
 */
export function askInStatus(app: AppState): boolean {
  return app.sounds.ask && app.sounds.fetch.state === "idle" && lastingHintUp(app) && !passingHintUp(app);
}

/** 聞いている帯の 2 行。short: 入りきらないときの小さい字（mock:R12-Fit）。 */
export function askLines(sounds: SoundsView, short: boolean): [string, string] {
  return [
    t("pack.ask", { mb: packMb(sounds.packBytes) }),
    short ? t("pack.askDetailShort") : t("pack.askDetail", { count: sounds.total }),
  ];
}

/** 取っている帯の 2 行（6 章の文を分けた物。のこりの時間はわかるまで出さない）。 */
export function fetchingLines(f: { doneBytes: number; totalBytes: number; remainingS: number | null }): [string, string] {
  const bytes = t("pack.fetchingBytes", { percent: percentOf(f.doneBytes, f.totalBytes), done: mb(f.doneBytes), total: mb(f.totalBytes) });
  const left = f.remainingS === null ? "" : `${STATUS_JOIN}${t("pack.fetchingLeft", { minutes: minutesOf(f.remainingS) })}`;
  return [t("pack.fetching"), bytes + left];
}

/**
 * 取れなかった帯の 2 行。1 行目はモック F8-PackFail の文。2 行目は、つながっていないときはモックの文、
 * ほかは 6 章のわけごとの文（出す所が「状態行 ・ 取りにいく所」の物）。
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

/** 取れなかったときの、えらんでいる形のボタン（とちゅうで切れたときは「続きから」）。 */
export const againLabel = (failure: PackFailure): string => t(failure.kind === "cutOff" ? "button.resumeFetch" : "button.again");
