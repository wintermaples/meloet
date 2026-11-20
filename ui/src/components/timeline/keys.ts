// テンポ ・ 拍子の変更とマーカーのキー（仕様書 4 章）。DOM を知らない。
// K: 再生の線の所にマーカー（一文字なので、文字を打っている最中は効かない）。Alt → ・ Alt ←: 次 ・ 前のマーカーへ（効く）。
// F2: えらんだマーカーの名前（えらんでいるのがマーカーのときだけ。ほかはトラックの名前）。
// Delete: えらんだテンポの点 ・ 拍子の旗 ・ マーカーを消す（曲の頭は消せない）。キーがアレンジに向いていて、
// クリップより後にえらんだときだけ（pick.ts）。録音中とカウントイン中は、どれも送らない（core も受けない）。
// キーの係（ui/src/keys/attach.ts の handleKeydown）が、Esc のほかは、ほかのキーの決まりより先に呼ぶ。

import type { Command, TimelineView } from "../../bridge";
import { t } from "../../text/ja";
import { lastPick } from "./pick";

export interface TimelineKey {
  key: string;
  code?: string;
  ctrl: boolean;
  meta: boolean;
  alt: boolean;
  shift: boolean;
}

export interface TimelineKeyContext {
  /** 文字を打っている最中。 */
  typing: boolean;
  /** 録音中かカウントイン中。 */
  busy: boolean;
  /** キーがアレンジに向いている（ピアノロールがひらいていないか、最後に押したのがアレンジ）。 */
  toArrange: boolean;
}

/** 送る意図。テンポ ・ 拍子 ・ マーカーのキーでなければ null（ほかの決まりへ）。 */
export function timelineKeyCommand(k: TimelineKey, c: TimelineKeyContext, timeline: TimelineView): Command | null {
  if (k.ctrl || k.meta || c.busy) return null;
  if (k.alt) {
    if (k.shift) return null;
    if (k.key === "ArrowRight") return { type: "stepMarker", forward: true };
    if (k.key === "ArrowLeft") return { type: "stepMarker", forward: false };
    return null;
  }
  if (k.shift) return null;
  const sel = timeline.selected;
  if (k.key === "F2") return sel?.kind === "marker" ? { type: "openMarkerName", id: sel.id } : null;
  if (c.typing) return null;
  if (k.key === "Delete") {
    // 曲の頭をえらんでいても、ここで受ける（core は曲の頭を消さない）。クリップへは回さない。
    if (!sel || !c.toArrange || lastPick.kind !== "timeline") return null;
    return { type: "deleteTimeline", target: sel };
  }
  const letter = /^[a-z]$/i.test(k.key) ? k.key.toLowerCase() : k.code === "KeyK" ? "k" : null;
  if (letter === "k") return { type: "addMarker", name: t("marker.newName", { count: timeline.markers.length + 1 }) };
  return null;
}
