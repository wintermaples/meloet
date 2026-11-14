// 出口 ・ 入口の行に出す物を、状態から決める（純粋な関数）。
import type { EndpointView, Friendly } from "../bridge";
import { t, type TextKey } from "../text/ja";

/** ささっているかの文。null は印も文も出さない（PipeWire でわからないとき）。 */
export type LinkLook = "plugged" | "connected" | "nothing" | null;

export interface EndpointLook {
  name: string;
  friendly: string | null;
  link: LinkLook;
}

const FRIENDLY: Record<Friendly, TextKey> = {
  headphones: "friendly.headphones",
  hdmi: "friendly.hdmi",
  lineOut: "friendly.lineOut",
  rearMic: "friendly.rearMic",
};

export function endpointLook(e: EndpointView): EndpointLook {
  const link: LinkLook =
    e.availability === "yes" ? (e.link === "jack" ? "plugged" : "connected") : e.availability === "no" ? "nothing" : null;
  return { name: e.name, friendly: e.friendly ? t(FRIENDLY[e.friendly]) : null, link };
}
