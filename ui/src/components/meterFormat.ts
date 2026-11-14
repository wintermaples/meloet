// 入力計と音の設定の、数や音名の書きかた。DOM を知らない純粋な関数。

const NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

/** MIDI の番号 → 音名。C4 = 60。 */
export function noteName(note: number): string {
  return `${NAMES[note % 12]}${Math.floor(note / 12) - 1}`;
}

/** 押している鍵を低い順に「C4 E4 G4」。 */
export function heldText(notes: Iterable<number>): string {
  return [...notes]
    .sort((a, b) => a - b)
    .map(noteName)
    .join(" ");
}

/** 0 うめの 3 けた（「012」）。 */
export function pad3(value: number): string {
  return String(value).padStart(3, "0");
}

/** 符号つき 4 けた（「+0000」「−0512」）。マイナスは仕様書と同じ「−」（U+2212）。 */
export function signed4(value: number): string {
  return `${value < 0 ? "−" : "+"}${String(Math.abs(value)).padStart(4, "0")}`;
}

/** サステインの ON と OFF の境（CC 64 の値）。 */
export const SUSTAIN_ON = 64;
