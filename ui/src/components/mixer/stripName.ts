// The name a strip goes by in the mixer, the parameters panel and the status line (B-86).

import type { SongView, StripRef } from "../../bridge/types";
import { t } from "../../text/ja";
import { insertName } from "../sounds";

/** A track: "2 ベース". A return: its letter and its top-most effect ("A リバーブ"), or the letter alone. */
export function stripName(song: SongView, strip: StripRef): string {
  switch (strip.kind) {
    case "track": {
      const track = song.tracks.find((tr) => tr.id === strip.id);
      return track ? t("track.label", { number: track.number, name: track.name }) : "";
    }
    case "return": {
      const bus = t(strip.bus === "a" ? "mixer.sendA" : "mixer.sendB");
      const top = song.returns[strip.bus === "a" ? 0 : 1].slots.find((e) => e !== null);
      return top ? t("mixer.returnName", { bus, effect: insertName(top) }) : bus;
    }
    case "master":
      return t("mixer.master");
  }
}
