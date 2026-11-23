// How the input meter and the sound settings write numbers and note names. Pure functions with no DOM.

const NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

/** MIDI number → note name. C4 = 60. */
export function noteName(note: number): string {
  return `${NAMES[note % 12]}${Math.floor(note / 12) - 1}`;
}

/** The held keys from low to high, as 「C4 E4 G4」. */
export function heldText(notes: Iterable<number>): string {
  return [...notes]
    .sort((a, b) => a - b)
    .map(noteName)
    .join(" ");
}

/** 3 digits with leading zeros (「012」). */
export function pad3(value: number): string {
  return String(value).padStart(3, "0");
}

/** 4 digits with a sign (「+0000」「−0512」). The minus sign is 「−」 (U+2212), as in the spec. */
export function signed4(value: number): string {
  return `${value < 0 ? "−" : "+"}${String(Math.abs(value)).padStart(4, "0")}`;
}

/** The border between sustain ON and OFF (the CC 64 value). */
export const SUSTAIN_ON = 64;
