// キーの決まり（仕様書 4 章）。DOM を知らない純粋な関数。

export type Platform = "mac" | "other";

/**
 * 出口えらび（流れ 2）の中にいるときだけ、↑ ↓ Enter をここで受ける。棚にキーが向いているときは、↑ ↓ Enter Space F を棚に。
 * 取り消しの履歴の帯にフォーカスがあるときは、↑ ↓ Home Enter を帯に。
 */
export type KeyContext = "main" | "outputChooser" | "shelf" | "history";

export interface KeyInput {
  /** KeyboardEvent.key */
  key: string;
  /** KeyboardEvent.code。Mac の Option は key を別の文字にかえるので、数字と文字はこちらでも見る。 */
  code?: string;
  ctrl: boolean;
  meta: boolean;
  alt: boolean;
  shift: boolean;
  /** IME で変換している最中（KeyboardEvent.isComposing）。そのあいだのキーは IME のもの。 */
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
  | { kind: "toStart" }
  | { kind: "loop" }
  | { kind: "metronome" }
  /** えらんだクリップの次のテイク（1）、前のテイク（-1）。 */
  | { kind: "stepTake"; delta: 1 | -1 }
  | { kind: "splitClips" }
  /** Mod J: join the chosen keyboard clips of one track (spec 4, round 10). */
  | { kind: "joinClips" }
  | { kind: "deleteSelected" }
  | { kind: "newSong" }
  | { kind: "openSong" }
  | { kind: "saveSong" }
  | { kind: "saveSongAs" }
  /** Mod Shift E: 書き出しのダイアログをひらく（仕様書 4 章）。 */
  | { kind: "export" }
  | { kind: "quit" }
  /** Shift M ・ Shift S: 選んだトラック（song.playTarget）のミュート ・ ソロ。 */
  | { kind: "trackMute" }
  | { kind: "trackSolo" }
  /** F10: 上のメニューの行の最初のメニューをひらく（B-59）。 */
  | { kind: "menuBar" }
  | { kind: "undo" }
  | { kind: "redo" }
  /** 横に拡大（1）・縮小（-1）。ピアノロールとアレンジの、最後に押したほう。 */
  | { kind: "zoom"; delta: 1 | -1 }
  // ピアノロール（仕様書 4 章）。どこに向けるかは attach.ts が決める。
  | { kind: "tool"; tool: "select" | "draw" | "erase" }
  /** ← →: グリッド steps 個、↑ ↓: keys 半音（Shift で 1 オクターブ）。 */
  | { kind: "nudge"; steps: number; keys: number }
  | { kind: "selectAll" }
  /** Q: 選んだノート（なければ全部）にクオンタイズをかける。 */
  | { kind: "quantize" }
  | { kind: "duplicate" }
  | { kind: "cut" }
  | { kind: "copy" }
  | { kind: "paste" }
  /** F2: 選んだトラックの名前を変える（その場の入力の枠）。With a clip chosen, the clip's name instead (round 10). */
  | { kind: "renameTrack" }
  // 棚の中（仕様書 4 章）。行を上下にえらぶ、えらんだ音源を入れる、サンプルを聞く ・ 止める、お気に入りの印。
  | { kind: "shelfStep"; delta: 1 | -1 }
  | { kind: "shelfConfirm" }
  | { kind: "shelfSample" }
  | { kind: "shelfFavorite" }
  /** B: えらんでいるインサートをとばす ・ もどす（仕様書 3 章 86-1）。 */
  | { kind: "bypass" }
  // 取り消しの履歴の帯（仕様書 4 章）。↑ は新しい手（上の行）へ、↓ は古い手へ。Home でいまの手へ、Enter でここまで戻す。
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
 * Mod ＋ ・ Mod −（仕様書 4 章）。＋ は配列によって Shift がいる（US は Shift ＝、JIS は Shift ；）ので Shift は見ない。
 * Shift なしの ＝ と、テンキーの ＋ − も同じにする。
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

/** 英字 1 つ（小文字）。key が英字でなければ（かな入力の配列など）code の KeyA… から読む。 */
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
 * キーのできごとから、することを決める。何もしないなら null。
 * typing: いま文字を打っている（isTyping）。一文字のキーと Space はそのあいだ効かない。Esc・Enter・Mod つき・Home は効く。
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
    // 文字を打っている最中の Mod A ・ X ・ C ・ V は、枠の中の文字のもの。
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
  // 仕様書 4 章: 棚の中の ↑ ↓ と Enter は、文字を打っている最中（さがす枠）も効く。
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

  // 仕様書 4 章: Home は文字を打っている最中も効く。Space は効かず、空白として入る。Delete も効かず、枠の中の文字を消す。
  if (k.key === "Home") return k.shift ? null : { kind: "toStart" };
  if (typing) return null;
  // 棚の中の Space は、再生ではなくサンプル（仕様書 3 章）。
  if (k.key === " " || k.code === "Space") return k.shift ? null : context === "shelf" ? { kind: "shelfSample" } : { kind: "playStop" };
  if (k.key === "Delete") return k.shift ? null : { kind: "deleteSelected" };

  const nudge = NUDGES[k.key];
  if (nudge) return k.shift && nudge.keys === 0 ? null : { kind: "nudge", steps: nudge.steps, keys: nudge.keys * (k.shift ? 12 : 1) };
  const d = digit(k);
  if (d && TOOL_DIGITS[d]) return k.shift ? null : { kind: "tool", tool: TOOL_DIGITS[d]! };

  const l = letter(k);
  if (k.shift) {
    // Shift M ・ Shift S は選んだトラックのミュート ・ ソロ。ほかの一文字のキーと同じく、文字を打っている最中は効かない（B-70）。
    if (l === "m") return { kind: "trackMute" };
    if (l === "s") return { kind: "trackSolo" };
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

/** いま文字を打っているか。input（文字を入れる種類）、textarea、contenteditable にカーソルがあるとき。 */
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

/** KeyboardEvent から KeyInput を作る。 */
export function keyInputOf(ev: KeyboardEvent): KeyInput {
  return { key: ev.key, code: ev.code, ctrl: ev.ctrlKey, meta: ev.metaKey, alt: ev.altKey, shift: ev.shiftKey, composing: ev.isComposing };
}
