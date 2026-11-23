// Decides what an output or input row shows, from the state (pure functions).
import type { EndpointView, Friendly, Unusable } from "../bridge";
import { t, type TextKey } from "../text/ja";

/** The plugged-in text. null shows neither mark nor text (when PipeWire cannot tell). */
export type LinkLook = "plugged" | "connected" | "nothing" | null;

export interface EndpointLook {
  name: string;
  /** The field next to the name (150). A plain-words name, or the device name to tell rows apart. */
  side: string | null;
  /** true when side is the device name (cut with 「…」 when long; hover shows all of it). */
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
 * A normal row. When sameName is true (outputs with the same name are listed) and the device name is known,
 * add the device name instead of the plain-words name to tell them apart (spec 3).
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

/** The reason it cannot be used. When the PC has not chosen a plugged-in input (other inputs are plugged in too), say to choose it in the PC settings instead of 「さすと…」. */
function reasonOf(o: EndpointView, kind: "output" | "input"): string {
  if (kind === "input" && o.unusable === "switchedByPc" && o.availability === "yes") return t("msg.inputPluggedNotChosen");
  return t(REASON[o.unusable!]);
}

/** Splits outputs or inputs into choosable ones and 「いまは使えない出口（入口）」. The order stays as Rust decided. */
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
