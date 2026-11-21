// 出口 ・ 入口の行に出す物を、状態から決める（純粋な関数）。
import type { EndpointView, Friendly, Unusable } from "../bridge";
import { t, type TextKey } from "../text/ja";

/** ささっているかの文。null は印も文も出さない（PipeWire でわからないとき）。 */
export type LinkLook = "plugged" | "connected" | "nothing" | null;

export interface EndpointLook {
  name: string;
  /** 名前のとなりの欄（150）。やさしい言いかえか、見わけるための装置の名前。 */
  side: string | null;
  /** side が装置の名前のとき true（長ければ「…」でちぢめ、マウスを乗せると全部出す）。 */
  sideIsDevice?: boolean;
  link: LinkLook;
}

const FRIENDLY: Record<Friendly, TextKey> = {
  headphones: "friendly.headphones",
  hdmi: "friendly.hdmi",
  lineOut: "friendly.lineOut",
  rearMic: "friendly.rearMic",
};

const REASON: Record<Unusable, TextKey> = {
  switchedByPc: "msg.unusableSwitchedByPc",
  stoppedByPc: "msg.unusableStoppedByPc",
};

/**
 * ふつうの行。sameName が true（同じ名前の出口がならぶ）で装置の名前がわかるときは、
 * 言いかえのかわりに装置の名前をそえて見わける（仕様書 3 章）。
 */
export function endpointLook(e: EndpointView, sameName = false): EndpointLook {
  const link: LinkLook =
    e.availability === "yes" ? (e.link === "jack" ? "plugged" : "connected") : e.availability === "no" ? "nothing" : null;
  if (sameName && e.device) return { name: e.name, side: e.device, sideIsDevice: true, link };
  return { name: e.name, side: e.friendly ? t(FRIENDLY[e.friendly]) : null, link };
}

export interface UnusableLook {
  id: string;
  name: string;
  device: string | null;
  reason: string;
}

export interface EndpointGroups {
  usable: { id: string; look: EndpointLook }[];
  unusable: UnusableLook[];
  switchedByPc: number;
  stoppedByPc: number;
}

/** 使えないわけの文。ささっている入口を PC がえらんでいないとき（ほかの入口もささっている）は、「さすと…」ではなく、PC の設定でえらぶ事を言う。 */
function reasonOf(o: EndpointView, kind: "output" | "input"): string {
  if (kind === "input" && o.unusable === "switchedByPc" && o.availability === "yes") return t("msg.inputPluggedNotChosen");
  return t(REASON[o.unusable!]);
}

/** 出口か入口を、えらべる物と「いまは使えない出口（入口）」に分ける。ならびは Rust が決めた順のまま。 */
export function endpointGroups(list: readonly EndpointView[], kind: "output" | "input" = "output"): EndpointGroups {
  const usable = list.filter((o) => o.unusable === null);
  const count = new Map<string, number>();
  for (const o of usable) count.set(o.name, (count.get(o.name) ?? 0) + 1);
  const unusable = list.flatMap((o) =>
    o.unusable === null ? [] : [{ id: o.id, name: o.name, device: o.device, reason: reasonOf(o, kind) }],
  );
  return {
    usable: usable.map((o) => ({ id: o.id, look: endpointLook(o, count.get(o.name)! > 1) })),
    unusable,
    switchedByPc: list.filter((o) => o.unusable === "switchedByPc").length,
    stoppedByPc: list.filter((o) => o.unusable === "stoppedByPc").length,
  };
}
