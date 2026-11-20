// What the parameters panel shows and which plugins have a window open, read off the state
// (protocol 18: `ParamsView`, `WindowsView`).

import type { InsertRef, ParamsView, PluginSlotRef, PluginView, PluginWindowView, SelectedInsert, SelectedPlugin, SongView, WindowsView } from "../../bridge";

/** The selection when it is an effect that comes with meloet. */
export function builtinOf(params: ParamsView): SelectedInsert | null {
  return params.selected?.kind === "builtin" ? params.selected : null;
}

/** The selection when it is a plugin (in an insert slot, or the instrument of a track). */
export function pluginOf(params: ParamsView): SelectedPlugin | null {
  return params.selected?.kind === "plugin" ? params.selected : null;
}

/** The insert slot the selection sits in, whatever it holds: the slot the mixer frames. null for an instrument. */
export function selectedInsert(params: ParamsView): InsertRef | null {
  const sel = params.selected;
  if (!sel) return null;
  if (sel.kind === "builtin") return sel.insert;
  return sel.kind === "plugin" && sel.slot.kind === "insert" ? sel.slot.insert : null;
}

/** The track whose instrument is the selection. */
export function selectedInstrument(params: ParamsView): number | null {
  const sel = params.selected;
  if (sel?.kind === "sound") return sel.track;
  return sel?.kind === "plugin" && sel.slot.kind === "instrument" ? sel.slot.track : null;
}

/** The plugin that sits at `slot` in the song: its name, format and state. */
export function pluginAt(song: SongView, slot: PluginSlotRef): PluginView | null {
  if (slot.kind === "instrument") {
    const sound = song.tracks.find((t) => t.id === slot.track)?.sound;
    return sound?.kind === "plugin" ? sound : null;
  }
  const { strip, slot: index } = slot.insert;
  const slots =
    strip.kind === "master" ? song.master.slots : strip.kind === "return" ? song.returns[strip.bus === "a" ? 0 : 1].slots : song.tracks.find((t) => t.id === strip.id)?.slots;
  const held = slots?.[index]?.effect;
  return held?.kind === "plugin" ? held : null;
}

const sameSlot = (a: PluginSlotRef | null, b: PluginSlotRef | null) => JSON.stringify(a) === JSON.stringify(b);

/** The open window of the plugin at `slot` (null: the plugin the shelf auditions), for the mark 「窓がひらいている」. */
export function windowOf(windows: WindowsView, slot: PluginSlotRef | null): PluginWindowView | null {
  return windows.list.find((w) => sameSlot(w.slot, slot)) ?? null;
}
