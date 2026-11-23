// The name a strip goes by in the mixer, the parameters panel and the status line (B-86).

import type { InsertView, LibraryView, SongView, StripRef } from "../../bridge/types";
import { t } from "../../text/ja";
import { insertName, pluginRowOf } from "../sounds";

/**
 * The name a return takes from its top-most effect: an effect that comes with meloet by its name, a plugin by its
 * kind (14-A, 「リバーブ」), or by its own name when its kind is 「そのほか」 or the shelf does not have it (user:2026-11-19).
 */
export function returnEffectName(insert: InsertView, library: LibraryView): string {
  const effect = insert.effect;
  if (effect.kind === "builtin") return insertName(insert);
  const kind = pluginRowOf(library, effect.key)?.kind;
  return kind?.role === "effect" && kind.category !== "other" ? t(`pluginKind.effect.${kind.category}`) : effect.name;
}

/** A track: 「2 ベース」. A return: its letter and its top-most effect (「A リバーブ」), or the letter alone. */
export function stripName(song: SongView, strip: StripRef, library: LibraryView): string {
  switch (strip.kind) {
    case "track": {
      const track = song.tracks.find((tr) => tr.id === strip.id);
      return track ? t("track.label", { number: track.number, name: track.name }) : "";
    }
    case "return": {
      const bus = t(strip.bus === "a" ? "mixer.sendA" : "mixer.sendB");
      const top = song.returns[strip.bus === "a" ? 0 : 1].slots.find((e) => e !== null);
      return top ? t("mixer.returnName", { bus, effect: returnEffectName(top, library) }) : bus;
    }
    case "master":
      return t("mixer.master");
  }
}
