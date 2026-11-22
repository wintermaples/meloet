// 音のファイルを窓の上に持ってきている間の、ポインタの場所（約束 24 の fileDrag ・ fileDrop。docs/architecture.md 22-17「ドラッグの約束」）。
// 道は Rust が持つ。画面が決めるのは落とした所だけ: 場所からトラック ・ ティック ・ 30-C の左右を決め、fileDrop で DropAudio を 1 つ送る。
// 座標はページの CSS px。入る所の計算はアレンジの部品（DropFrame）が登録する（アレンジがないときは、新しい歌のトラックの 0）。

import type { DropMode, Event } from "../../bridge";
import { current, onEvent, send } from "../../state/app.svelte";
import { dropModeAt } from "./audioClipGeom";

/** 落とす所: 歌のトラック（null は新しい歌のトラック）と、拍に吸いつけたティック。 */
export interface DropPlace {
  track: number | null;
  tick: number;
}

export const fileDrag = $state<{ at: { x: number; y: number } | null }>({ at: null });

type Resolver = (pageX: number, pageY: number, mode: DropMode) => DropPlace;
let resolver: Resolver | null = null;

/** アレンジの部品が、ページの場所 → 落とす所を登録する。返り値の関数で外す。 */
export function setDropResolver(next: Resolver): () => void {
  resolver = next;
  return () => {
    if (resolver === next) resolver = null;
  };
}

/** ページのこの場所に落としたら、どこに入るか（アレンジがなければ null）。 */
export function dropPlaceAt(pageX: number, pageY: number, mode: DropMode): DropPlace | null {
  return resolver?.(pageX, pageY, mode) ?? null;
}

/** 音のファイルが 2 つ以上なら窓の左右で 30-C を分ける。1 つなら、落とした所のトラックへ（仕様書 3 章 29-A）。 */
export function dropModeFor(audio: number, pageX: number, windowWidth: number): DropMode {
  return audio >= 2 ? dropModeAt(pageX, windowWidth) : "inRow";
}

/** 最後に持ってきていた音のファイルの数（落とした時には、知らせの箱がもう閉じていることがある）。 */
let lastAudio = 1;

export function heardFileEvent(e: Event): void {
  if (e.type === "fileDrag") {
    fileDrag.at = { x: e.x, y: e.y };
    lastAudio = current.state?.workspace.fileDrop?.audio ?? lastAudio;
    return;
  }
  if (e.type !== "fileDrop") return;
  fileDrag.at = null;
  const audio = current.state?.workspace.fileDrop?.audio ?? lastAudio;
  const mode = dropModeFor(audio, e.x, window.innerWidth);
  const place = resolver?.(e.x, e.y, mode) ?? { track: null, tick: 0 };
  void send({ type: "dropAudio", drop: e.drop, track: place.track, tick: place.tick, mode });
}

onEvent(heardFileEvent);
