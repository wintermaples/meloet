// Chord help in the piano roll (spec 3, "Rules added in round 10"; mocks P10-ChordStamp and option A of P10-ChordTable).
// No DOM. The draw tool has one choice: a chord shape (the pressed row is the root) or a chord of the key (the root is
// the nearest row with that note name). The last one chosen wins.

import type { MusicalScale, ScaleMode } from "../../bridge";
import { t, type TextKey } from "../../text/ja";
import { pitchClassName } from "../format";
import { KEY_MAX, KEY_MIN, noteName, SCALE_STEPS } from "./geom";

export type ChordShape = "single" | "major" | "minor" | "seventh" | "majorSeventh" | "minorSeventh" | "sus4" | "dim" | "aug";

/** The order of the list 「和音の形」. */
export const CHORD_SHAPES: readonly ChordShape[] = ["single", "major", "minor", "seventh", "majorSeventh", "minorSeventh", "sus4", "dim", "aug"];

/** Semitones above the root. */
export const SHAPE_STEPS: Readonly<Record<ChordShape, readonly number[]>> = {
  single: [0],
  major: [0, 4, 7],
  minor: [0, 3, 7],
  seventh: [0, 4, 7, 10],
  majorSeventh: [0, 4, 7, 11],
  minorSeventh: [0, 3, 7, 10],
  sus4: [0, 5, 7],
  dim: [0, 3, 6],
  aug: [0, 4, 8],
};

const SHAPE_TEXT: Readonly<Record<ChordShape, { name: TextKey; onC: TextKey }>> = {
  single: { name: "pr.chordSingle", onC: "pr.chordOnCSingle" },
  major: { name: "pr.chordMajor", onC: "pr.chordOnCMajor" },
  minor: { name: "pr.chordMinor", onC: "pr.chordOnCMinor" },
  seventh: { name: "pr.chordSeventh", onC: "pr.chordOnCSeventh" },
  majorSeventh: { name: "pr.chordMajorSeventh", onC: "pr.chordOnCMajorSeventh" },
  minorSeventh: { name: "pr.chordMinorSeventh", onC: "pr.chordOnCMinorSeventh" },
  sus4: { name: "pr.chordSus4", onC: "pr.chordOnCSus4" },
  dim: { name: "pr.chordDim", onC: "pr.chordOnCDim" },
  aug: { name: "pr.chordAug", onC: "pr.chordOnCAug" },
};

export const shapeName = (shape: ChordShape) => t(SHAPE_TEXT[shape].name);
/** The notes of the shape with C as the root, as the mock writes them (with flats). */
export const shapeOnC = (shape: ChordShape) => t(SHAPE_TEXT[shape].onC);

/** A chord of the key: three notes of the scale, each one skipping the next note of the scale. */
export type Triad = "major" | "minor" | "dim" | "aug";

const TRIAD_STEPS: Readonly<Record<Triad, readonly number[]>> = { major: [0, 4, 7], minor: [0, 3, 7], dim: [0, 3, 6], aug: [0, 4, 8] };

/** `root` of a chord of the key is a note name (0 is C). */
export type ChordPick = { kind: "shape"; shape: ChordShape } | { kind: "key"; root: number; triad: Triad };

export const SINGLE: ChordPick = { kind: "shape", shape: "single" };

export function pickSteps(pick: ChordPick): readonly number[] {
  return pick.kind === "shape" ? SHAPE_STEPS[pick.shape] : TRIAD_STEPS[pick.triad];
}

export const isChord = (pick: ChordPick) => pickSteps(pick).length > 1;

export const samePick = (a: ChordPick, b: ChordPick) =>
  a.kind === "shape" ? b.kind === "shape" && a.shape === b.shape : b.kind === "key" && a.root === b.root && a.triad === b.triad;

/** The key nearest to `key` with the note name `pitchClass`; the lower one when both are 6 semitones away. */
export function nearestRoot(key: number, pitchClass: number): number {
  const below = key - ((((key - pitchClass) % 12) + 12) % 12);
  const root = key - below <= below + 12 - key ? below : below + 12;
  if (root < KEY_MIN) return root + 12;
  if (root > KEY_MAX) return root - 12;
  return root;
}

export function rootOf(pick: ChordPick, key: number): number {
  return pick.kind === "shape" ? key : nearestRoot(key, pick.root);
}

/** The keys written when the row `key` is pressed, low to high. Notes above 127 are left out. */
export function stampKeys(pick: ChordPick, key: number): number[] {
  const root = rootOf(pick, key);
  return pickSteps(pick)
    .map((s) => root + s)
    .filter((k) => k >= KEY_MIN && k <= KEY_MAX);
}

function triadOf(third: number, fifth: number): Triad | null {
  for (const [triad, steps] of Object.entries(TRIAD_STEPS) as [Triad, readonly number[]][]) {
    if (steps[1] === third && steps[2] === fifth) return triad;
  }
  return null;
}

/**
 * The chords of a scale, from the tonic up. `steps` are the notes of the scale in semitones above the tonic.
 * A scale without 7 notes is built the same way; a stack that is not major, minor, dim or aug has no name and is left out.
 */
export function chordsOfSteps(tonic: number, steps: readonly number[]): { root: number; triad: Triad }[] {
  const n = steps.length;
  const out: { root: number; triad: Triad }[] = [];
  if (n < 3) return out;
  const at = (i: number) => steps[i % n]! + 12 * Math.floor(i / n);
  for (let i = 0; i < n; i++) {
    const triad = triadOf(at(i + 2) - at(i), at(i + 4) - at(i));
    if (triad) out.push({ root: (tonic + steps[i]!) % 12, triad });
  }
  return out;
}

/** The 7-note scale whose chords a scale of 5 or 6 notes lists. */
const PARENT_MODE: Partial<Record<ScaleMode, ScaleMode>> = { majorPentatonic: "major", minorPentatonic: "minor", blues: "minor" };

export function keyChords(scale: Pick<MusicalScale, "root" | "mode">): { root: number; triad: Triad }[] {
  return chordsOfSteps(scale.root, SCALE_STEPS[PARENT_MODE[scale.mode] ?? scale.mode]);
}

const TRIAD_NAME: Readonly<Record<Triad, TextKey | null>> = { major: null, minor: "pr.keyChordMinor", dim: "pr.keyChordDim", aug: "pr.keyChordAug" };

/** 「C」「Dm」「Bdim」. */
export function triadName(root: number, triad: Triad): string {
  const key = TRIAD_NAME[triad];
  return key ? t(key, { root: pitchClassName(root) }) : pitchClassName(root);
}

/** The name on the draw button (「メジャー」「Dm」); null for one note. */
export function pickName(pick: ChordPick): string | null {
  if (!isChord(pick)) return null;
  return pick.kind === "shape" ? shapeName(pick.shape) : triadName(pick.root, pick.triad);
}

/** The status line while the draw tool is over the row `key`. */
export function stampStatus(pick: ChordPick, key: number): string | null {
  if (!isChord(pick)) return null;
  const notes = stampKeys(pick, key).map(noteName).join(" ");
  if (pick.kind === "key") return t("pr.keyChordStatus", { chord: triadName(pick.root, pick.triad), notes });
  return t("pr.chordStatus", { shape: shapeName(pick.shape), root: noteName(key), notes });
}

/** What pressing a chord of the key plays: the root between C4 and B4. */
export function auditionKeys(root: number, triad: Triad): number[] {
  return TRIAD_STEPS[triad].map((s) => 60 + root + s);
}

// ---- 「進行から並べる」 (round 12, P6) ----

/** The scale whose chords a 5- or 6-note scale shows (P1): major pentatonic → major; minor pentatonic and blues → minor; 7-note scales → themselves. */
export function parentScale(mode: ScaleMode): ScaleMode {
  if (mode === "majorPentatonic") return "major";
  if (mode === "minorPentatonic" || mode === "blues") return "minor";
  return mode;
}

/** Semitones from a scale's root up to the tonic of the major key a progression is read in: the major scale with the same notes (minor kinds: the relative major). */
const MAJOR_TONIC: Partial<Record<ScaleMode, number>> = { minor: 3, harmonicMinor: 3, melodicMinor: 3, dorian: 10, mixolydian: 5 };

export type Progression = "royal" | "canon" | "komuro" | "circle" | "pop" | "sad";

/** A degree of the major scale: semitones above the tonic, and the chord there. */
type Degree = { step: number; triad: Triad };
const I: Degree = { step: 0, triad: "major" };
const iii: Degree = { step: 4, triad: "minor" };
const IV: Degree = { step: 5, triad: "major" };
const V: Degree = { step: 7, triad: "major" };
const vi: Degree = { step: 9, triad: "minor" };

/** The order of the list. */
export const PROGRESSIONS: readonly { id: Progression; name: TextKey; degrees: readonly Degree[] }[] = [
  { id: "royal", name: "pr.progRoyal", degrees: [IV, V, iii, vi] },
  { id: "canon", name: "pr.progCanon", degrees: [I, V, vi, iii, IV, I, IV, V] },
  { id: "komuro", name: "pr.progKomuro", degrees: [vi, IV, V, I] },
  { id: "circle", name: "pr.progCircle", degrees: [I, vi, IV, V] },
  { id: "pop", name: "pr.progPop", degrees: [I, V, vi, IV] },
  { id: "sad", name: "pr.progSad", degrees: [vi, V, IV, V] },
];

/** The major key a progression is read in. */
export function progressionTonic(scale: Pick<MusicalScale, "root" | "mode">): number {
  return (scale.root + (MAJOR_TONIC[parentScale(scale.mode)] ?? 0)) % 12;
}

/** The chords of a progression in the scale. */
export function progressionChords(id: Progression, scale: Pick<MusicalScale, "root" | "mode">): { root: number; triad: Triad }[] {
  const tonic = progressionTonic(scale);
  const p = PROGRESSIONS.find((x) => x.id === id)!;
  return p.degrees.map((d) => ({ root: (tonic + d.step) % 12, triad: d.triad }));
}

/** 「F – G – Em – Am」. */
export function progressionText(id: Progression, scale: Pick<MusicalScale, "root" | "mode">): string {
  return progressionChords(id, scale)
    .map((c) => triadName(c.root, c.triad))
    .join(t("pr.progSeparator"));
}

/** The keys placed for a chord: root position, the root between C3 and B3. */
export function placedKeys(root: number, triad: Triad): number[] {
  return TRIAD_STEPS[triad].map((s) => 48 + root + s);
}
