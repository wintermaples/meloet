// The key rules (spec 4). Pure functions that do not know the DOM.

export type Platform = "mac" | "other";

/**
 * Only inside the output chooser (flow 2) are ↑ ↓ Enter taken here. When keys go to the shelf, ↑ ↓ Enter Space F go to the shelf.
 * When the undo history band has focus, ↑ ↓ Home Enter go to the band.
 */
export type KeyContext = "main" | "outputChooser" | "shelf" | "history";

export interface KeyInput {
  /** KeyboardEvent.key */
  key: string;
  /** KeyboardEvent.code. On the Mac, Option turns key into another character, so digits and letters are also read from here. */
  code?: string;
  ctrl: boolean;
  meta: boolean;
  alt: boolean;
  shift: boolean;
  /** The IME is composing (KeyboardEvent.isComposing). Keys meanwhile belong to the IME. */
  composing?: boolean;
}

export type KeyAction =
  | { kind: "arrange" }
  | { kind: "toggleBand"; band: "pianoRoll" | "mixer" | "history" | "shelf" | "params" }
  | { kind: "back" }
  | { kind: "audioSettings" }
  | { kind: "scale"; scale: 100 | 125 | 150 }
  | { kind: "chooserUp" }
  | { kind: "chooserDown" }
  | { kind: "chooserConfirm" }
  | { kind: "playStop" }
  | { kind: "record" }
  /** Shift R: 「さっきのを残す」 (round 12, R1). */
  | { kind: "keepPlayed" }
  | { kind: "toStart" }
  | { kind: "loop" }
  | { kind: "metronome" }
  /** The next take (1) or the previous take (-1) of the chosen clip. */
  | { kind: "stepTake"; delta: 1 | -1 }
  | { kind: "splitClips" }
  /** Mod J: join the chosen keyboard clips of one track (spec 4, round 10). */
  | { kind: "joinClips" }
  /** Mod R: the box of 「くり返す…」 on the chosen clips (round 12 A1). */
  | { kind: "repeatClips" }
  | { kind: "deleteSelected" }
  | { kind: "newSong" }
  | { kind: "openSong" }
  | { kind: "saveSong" }
  | { kind: "saveSongAs" }
  /** Mod Shift E: open the export dialog (spec 4). */
  | { kind: "export" }
  | { kind: "quit" }
  /** Shift M, Shift S: mute, solo of the selected track (song.playTarget). */
  | { kind: "trackMute" }
  | { kind: "trackSolo" }
  /** F10: open the first menu of the menu bar (B-59). */
  | { kind: "menuBar" }
  | { kind: "undo" }
  | { kind: "redo" }
  /** Zoom in (1) or out (-1) across, in whichever of the piano roll and the arrange was pressed last. */
  | { kind: "zoom"; delta: 1 | -1 }
  // Piano roll (spec 4). attach.ts decides where these go.
  | { kind: "tool"; tool: "select" | "draw" | "erase" }
  /** ← →: steps grid cells; ↑ ↓: keys semitones (an octave with Shift). */
  | { kind: "nudge"; steps: number; keys: number }
  | { kind: "selectAll" }
  /** Q: quantize the selected notes (all notes when none are selected). */
  | { kind: "quantize" }
  | { kind: "duplicate" }
  | { kind: "cut" }
  | { kind: "copy" }
  | { kind: "paste" }
  /** F2: rename the selected track (an input box in place). With a clip chosen, the clip's name instead (round 10). */
  | { kind: "renameTrack" }
  // In the shelf (spec 4): choose a row up or down, put in the chosen instrument, play or stop the sample, the favorite mark.
  | { kind: "shelfStep"; delta: 1 | -1 }
  | { kind: "shelfConfirm" }
  | { kind: "shelfSample" }
  | { kind: "shelfFavorite" }
  /** B: bypass or restore the chosen insert (spec 3 (86-1)). */
  | { kind: "bypass" }
  // The undo history band (spec 4). ↑ goes to a newer step (the row above), ↓ to an older one. Home goes to the current step, Enter undoes back to here.
  | { kind: "historyStep"; delta: 1 | -1 }
  | { kind: "historyHome" }
  | { kind: "historyConfirm" };

const TOOL_DIGITS: Record<string, "select" | "draw" | "erase"> = { "1": "select", "2": "draw", "3": "erase" };
const NUDGES: Record<string, { steps: number; keys: number }> = {
  ArrowLeft: { steps: -1, keys: 0 },
  ArrowRight: { steps: 1, keys: 0 },
  ArrowUp: { steps: 0, keys: 1 },
  ArrowDown: { steps: 0, keys: -1 },
};
const MOD_EDITS: Record<string, KeyAction> = {
  a: { kind: "selectAll" },
  d: { kind: "duplicate" },
  x: { kind: "cut" },
  c: { kind: "copy" },
  v: { kind: "paste" },
};

/**
 * Mod + and Mod − (spec 4). + needs Shift on some layouts (Shift = on US, Shift ; on JIS), so Shift is not looked at.
 * = without Shift and the number pad + − do the same.
 */
function zoomKey(k: KeyInput): 1 | -1 | null {
  if (k.key === "+" || k.key === "=" || k.code === "NumpadAdd") return 1;
  if (k.key === "-" || k.key === "_" || k.code === "NumpadSubtract") return -1;
  return null;
}

const BAND_KEYS: Record<string, KeyAction> = {
  a: { kind: "arrange" },
  e: { kind: "toggleBand", band: "pianoRoll" },
  m: { kind: "toggleBand", band: "mixer" },
  h: { kind: "toggleBand", band: "history" },
  p: { kind: "toggleBand", band: "shelf" },
  u: { kind: "toggleBand", band: "params" },
  r: { kind: "record" },
  l: { kind: "loop" },
  c: { kind: "metronome" },
  t: { kind: "stepTake", delta: 1 },
  b: { kind: "bypass" },
  s: { kind: "splitClips" },
};

/** One letter (lower case). When key is not a Latin letter (a kana layout, for example), it is read from code (KeyA…). */
function letter(k: KeyInput): string | null {
  if (/^[a-z]$/i.test(k.key)) return k.key.toLowerCase();
  const m = k.code?.match(/^Key([A-Z])$/);
  return m ? m[1]!.toLowerCase() : null;
}

function digit(k: KeyInput): string | null {
  if (/^[0-9]$/.test(k.key)) return k.key;
  const m = k.code?.match(/^(?:Digit|Numpad)([0-9])$/);
  return m ? m[1]! : null;
}

/**
 * Decides what to do from a key event. null when nothing is done.
 * typing: text is being typed (isTyping). One-letter keys and Space do not work meanwhile. Esc, Enter, Mod keys and Home do.
 */
export function keyAction(k: KeyInput, typing: boolean, platform: Platform, context: KeyContext = "main"): KeyAction | null {
  if (k.composing) return null;
  const mod = platform === "mac" ? k.meta : k.ctrl;
  const otherMod = platform === "mac" ? k.ctrl : k.meta;
  if (otherMod) return null;

  if (mod) {
    const zoom = k.alt ? null : zoomKey(k);
    if (zoom) return { kind: "zoom", delta: zoom };
    const l = k.alt ? null : letter(k);
    if (k.shift) return l === "z" ? { kind: "redo" } : l === "s" ? { kind: "saveSongAs" } : l === "e" ? { kind: "export" } : null;
    if (l === "n") return { kind: "newSong" };
    if (l === "o") return { kind: "openSong" };
    if (l === "s") return { kind: "saveSong" };
    if (l === "z") return { kind: "undo" };
    if (l === "q") return { kind: "quit" };
    if (l === "j") return { kind: "joinClips" };
    if (l === "r") return { kind: "repeatClips" };
    // While typing, Mod A, X, C, V belong to the text in the box.
    if (l && MOD_EDITS[l] && !(typing && l !== "d")) return MOD_EDITS[l]!;
    if (!k.alt && (k.key === "," || k.code === "Comma")) return { kind: "audioSettings" };
    if (k.alt) {
      const d = digit(k);
      if (d === "1") return { kind: "scale", scale: 100 };
      if (d === "2") return { kind: "scale", scale: 125 };
      if (d === "3") return { kind: "scale", scale: 150 };
    }
    return null;
  }
  if (k.alt) return null;

  if (k.key === "Escape") return k.shift ? null : { kind: "back" };
  if (k.key === "F10") return k.shift ? null : { kind: "menuBar" };

  if (!k.shift && context === "outputChooser") {
    if (k.key === "ArrowUp") return { kind: "chooserUp" };
    if (k.key === "ArrowDown") return { kind: "chooserDown" };
    if (k.key === "Enter") return { kind: "chooserConfirm" };
  }
  // Spec 4: ↑ ↓ and Enter in the shelf also work while typing (in the search box).
  if (!k.shift && context === "shelf") {
    if (k.key === "ArrowUp") return { kind: "shelfStep", delta: -1 };
    if (k.key === "ArrowDown") return { kind: "shelfStep", delta: 1 };
    if (k.key === "Enter") return { kind: "shelfConfirm" };
  }
  if (k.key === "F2") return k.shift ? null : { kind: "renameTrack" };
  if (!k.shift && !typing && context === "history") {
    if (k.key === "ArrowUp") return { kind: "historyStep", delta: 1 };
    if (k.key === "ArrowDown") return { kind: "historyStep", delta: -1 };
    if (k.key === "Home") return { kind: "historyHome" };
    if (k.key === "Enter") return { kind: "historyConfirm" };
  }

  // Spec 4: Home works while typing. Space does not; it goes in as a space. Delete does not either; it deletes text in the box.
  if (k.key === "Home") return k.shift ? null : { kind: "toStart" };
  if (typing) return null;
  // Space in the shelf plays the sample, not the song (spec 3).
  if (k.key === " " || k.code === "Space") return k.shift ? null : context === "shelf" ? { kind: "shelfSample" } : { kind: "playStop" };
  if (k.key === "Delete") return k.shift ? null : { kind: "deleteSelected" };

  const nudge = NUDGES[k.key];
  if (nudge) return k.shift && nudge.keys === 0 ? null : { kind: "nudge", steps: nudge.steps, keys: nudge.keys * (k.shift ? 12 : 1) };
  const d = digit(k);
  if (d && TOOL_DIGITS[d]) return k.shift ? null : { kind: "tool", tool: TOOL_DIGITS[d]! };

  const l = letter(k);
  if (k.shift) {
    // Shift M, Shift S mute and solo the selected track. Like other one-letter keys, they do not work while typing (B-70).
    if (l === "m") return { kind: "trackMute" };
    if (l === "s") return { kind: "trackSolo" };
    if (l === "r") return { kind: "keepPlayed" };
    return l === "t" ? { kind: "stepTake", delta: -1 } : null;
  }
  if (l === "q") return { kind: "quantize" };
  if (l === "f") return context === "shelf" ? { kind: "shelfFavorite" } : null;
  return (l && BAND_KEYS[l]) || null;
}

export function platformOf(nav: { platform?: string; userAgent?: string }): Platform {
  return /Mac|iPhone|iPad/.test(nav.platform || nav.userAgent || "") ? "mac" : "other";
}

const NON_TEXT_INPUTS = new Set(["button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit"]);

/** Whether text is being typed: the cursor is in an input (of a text kind), a textarea, or a contenteditable. */
export function isTyping(doc: Document = document): boolean {
  let el: Element | null = doc.activeElement;
  while (el?.shadowRoot?.activeElement) el = el.shadowRoot.activeElement;
  if (!el) return false;
  if (el instanceof HTMLTextAreaElement) return !el.readOnly && !el.disabled;
  if (el instanceof HTMLInputElement) return !NON_TEXT_INPUTS.has(el.type) && !el.readOnly && !el.disabled;
  if (el instanceof HTMLElement && el.isContentEditable) return true;
  const editable = el.closest("[contenteditable]");
  return editable !== null && editable.getAttribute("contenteditable") !== "false";
}

/** Makes a KeyInput from a KeyboardEvent. */
export function keyInputOf(ev: KeyboardEvent): KeyInput {
  return { key: ev.key, code: ev.code, ctrl: ev.ctrlKey, meta: ev.metaKey, alt: ev.altKey, shift: ev.shiftKey, composing: ev.isComposing };
}
