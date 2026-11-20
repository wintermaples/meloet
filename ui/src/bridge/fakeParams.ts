// The parameters of the fake's plugins, as `plugin_params` gives them: the lists of the mocks
// F-ParamFew, F-ParamMany, F-ParamSearch, F-ParamNoWindow and F5-MixerBypass. Where a mock shows
// only some rows of a group, the rest are filler with plain names.

import type { ParamGroup, ParamKind, ParamRow, ParamTable, ParamValues } from "./types";

const SMOOTH: ParamKind = { kind: "smooth" };
const TOGGLE: ParamKind = { kind: "toggle" };
const steps = (labels: string[], count = labels.length): ParamKind => ({ kind: "steps", steps: count, labels });

type Spec = [name: string, text: string | null, value?: number | null, kind?: ParamKind];

/** Rows numbered from `from`, in order. A row with no value given sits in the middle. */
function rows(from: number, specs: Spec[]): ParamRow[] {
  return specs.map(([name, text, value = 0.5, kind = SMOOTH], i) => ({ id: from + i, name, kind, default: 0.5, value, text }));
}

const one = (specs: Spec[]): ParamGroup[] => [{ name: null, params: rows(0, specs) }];

// F-ParamFew: 18 parameters, no groups.
const DRAGONFLY: Spec[] = [
  ["Dry Level", "80 %", 0.8],
  ["Early Level", "10 %", 0.1],
  ["Early Send", "20 %", 0.2],
  ["Late Level", "20 %", 0.2],
  ["Size", "24 m", 0.3],
  ["Width", "100 %", 1],
  ["Predelay", "4 ms", 0.04],
  ["Decay", "1.3 s", 0.12],
  ["Diffuse", "90 %", 0.9],
  ["Spin", "3.3 Hz", 0.33],
  ["Wander", "15 ms", 0.3],
  ["Modulation", "15 %", 0.15],
  ["Low Cut", "50 Hz", 0.25],
  ["Low Crossover", "200 Hz", 0.15],
  ["Low Decay Mult", "1.20 x", 0.4],
  ["High Cut", "16000 Hz", 1],
  ["High Crossover", "6000 Hz", 0.3],
  ["High Decay Mult", "0.50 x", 0.25],
];

// F-ParamNoWindow: 12 parameters. The mock names only the step each of the two lists is on ("Hall", "Sine", "3 / 12").
const MOSSY: Spec[] = [
  ["Type", "Hall", 0.25, steps(["Room", "Hall", "Plate", "Chamber", "Spring"])],
  ["Freeze", "オフ", 0, TOGGLE],
  ["Mix", "35 %", 0.35],
  ["Size", "62 %", 0.62],
  ["Decay", "2.40 s", 0.24],
  ["Predelay", "12 ms", 0.06],
  ["Damping", "4.2 kHz", 0.4],
  ["Low Cut", "80 Hz", 0.08],
  ["Mod Shape", "Sine", 2 / 11, steps([], 12)],
  ["Mod Depth", "18 %", 0.18],
  ["Mod Rate", "0.80 Hz", 0.08],
  ["Bypass", "オフ", 0, TOGGLE],
];

// F5-MixerBypass: 9 parameters.
const MVERB: Spec[] = [
  ["Damping", "50 %", 0.5],
  ["Density", "50 %", 0.5],
  ["Bandwidth", "75 %", 0.75],
  ["Decay", "50 %", 0.5],
  ["Predelay", "0 %", 0],
  ["Size", "75 %", 0.75],
  ["Gain", "100 %", 1],
  ["Mix", "30 %", 0.3],
  ["Early / Late Mix", "50 %", 0.5],
];

/** The ids of the two rows an automation lane writes in F-ParamMany. */
export const SURGE_AUTOMATED = [1000, 1012];

// F6-ParamMany (and F-ParamMany, F-ParamSearch, whose 2855 the spec reads as 774, 91-3): 774 parameters in six groups
// (8 + 21 + 120 + 300 + 300 + 25), and 2080 that only take a keyboard's knobs ("MIDI CC 0|0" … "MIDI CC 15|129").
// "cutoff" is in 12 names of Scene A and 12 of Scene B, and nowhere else.
function surgeGroups(): ParamGroup[] {
  const filler = (from: number, count: number, name: (n: number) => string): ParamRow[] =>
    rows(from, Array.from({ length: count }, (_, i): Spec => [name(i + 1), "0.00 %", 0]));
  const scene = (from: number, s: "A" | "B"): ParamRow[] => {
    const hz = s === "A" ? ["523.25 Hz", "1046.50 Hz"] : ["440.00 Hz", "880.00 Hz"];
    const named: Spec[] = [
      [`${s} Filter 1 Cutoff`, hz[0]!, 0.6],
      [`${s} Filter 2 Cutoff`, hz[1]!, 0.7],
      [`${s} Filter 2 Cutoff Offset`, "+0.00", 0.5],
      [`${s} FEG > Filter 1 Cutoff`, s === "A" ? "+12.00" : "+0.00", 0.6],
      [`${s} FEG > Filter 2 Cutoff`, "+0.00", 0.5],
      [`${s} Highpass Cutoff`, "8.18 Hz", 0],
      ...Array.from({ length: 6 }, (_, i): Spec => [`${s} LFO ${i + 1} > Filter 1 Cutoff`, "+0.00", 0.5]),
      [`${s} Osc 1 Pitch`, "+0.00", 0.5],
    ];
    return [...rows(from, named), ...filler(from + named.length, 300 - named.length, (n) => `${s} Parameter ${n}`)];
  };
  const macros: Spec[] = ["35.00 %", "0.00 %", "62.50 %", "0.00 %", "0.00 %", "100.00 %", "0.00 %", "12.00 %"].map(
    (text, i): Spec => [`Macro ${i + 1}`, text, Number.parseFloat(text) / 100],
  );
  return [
    { name: "Macros", params: rows(0, macros) },
    { name: "Global", params: filler(100, 21, (n) => `Global ${n}`) },
    { name: "FX", params: filler(200, 120, (n) => `FX ${n}`) },
    { name: "Scene A", params: scene(1000, "A") },
    { name: "Scene B", params: scene(2000, "B") },
    { name: null, params: filler(3000, 25, (n) => `Other ${n}`) },
  ];
}

function surgeKnobs(): ParamRow[] {
  const names = Array.from({ length: 16 * 130 }, (_, i): Spec => [`MIDI CC ${Math.floor(i / 130)}|${i % 130}`, "0.00 %", 0]);
  return rows(4000, names);
}

/** `SelectedPlugin.table` of each fake plugin: a scene names the plugin by it. */
export const FAKE_TABLES = { dragonfly: 1, mossy: 2, mverb: 3, surge: 4 } as const;

const KNOBS: Record<number, () => ParamRow[]> = { [FAKE_TABLES.surge]: surgeKnobs };

const GROUPS: Record<number, () => ParamGroup[]> = {
  [FAKE_TABLES.dragonfly]: () => one(DRAGONFLY),
  [FAKE_TABLES.mossy]: () => one(MOSSY),
  [FAKE_TABLES.mverb]: () => one(MVERB),
  [FAKE_TABLES.surge]: surgeGroups,
};

/** The reply of `plugin_params`. */
export function fakeParamTable(table: number): ParamTable | null {
  const groups = GROUPS[table];
  return groups ? { table, seq: 0, groups: groups(), keyboardKnobs: KNOBS[table]?.() ?? [] } : null;
}

export const fakeParamCount = (table: number): number => (GROUPS[table]?.() ?? []).reduce((n, g) => n + g.params.length, 0);

/** The fake plugin's text of a value: the number in its unit for "%", the step's name, オン ・ オフ; else 0..1. */
function textOf(row: ParamRow, value: number): string {
  const kind = row.kind;
  if (kind.kind === "toggle") return value >= 0.5 ? "オン" : "オフ";
  if (kind.kind === "steps") {
    const step = Math.round(value * (kind.steps - 1));
    return kind.labels[step] ?? `${step + 1} / ${kind.steps}`;
  }
  const unit = row.text?.match(/^[+−-]?[\d.]+\s*(.*)$/)?.[1] ?? "";
  const places = row.text?.match(/\.(\d+)/)?.[1]?.length ?? 0;
  return unit === "%" ? `${(value * 100).toFixed(places)} %` : value.toFixed(2);
}

/**
 * The values of the fake's plugins, as one transport holds them: `SetPluginParam` and `TypeParam` change them, and
 * each change comes back as a `ParamValues` message of its table (with `seq` one up).
 */
export class FakeParamTables {
  private readonly tables = new Map<number, ParamTable>();

  private held(table: number): ParamTable | null {
    let t = this.tables.get(table);
    if (!t) {
      const made = fakeParamTable(table);
      if (!made) return null;
      this.tables.set(table, (t = made));
    }
    return t;
  }

  private row(table: number, id: number): ParamRow | null {
    const held = this.held(table);
    for (const g of held?.groups ?? []) for (const r of g.params) if (r.id === id) return r;
    return held?.keyboardKnobs.find((r) => r.id === id) ?? null;
  }

  /** The reply of `plugin_params`. */
  table(table: number): ParamTable | null {
    const t = this.held(table);
    return t ? structuredClone(t) : null;
  }

  set(table: number, id: number, value: number): ParamValues | null {
    const row = this.row(table, id);
    const t = this.held(table);
    if (!row || !t) return null;
    row.value = Math.max(0, Math.min(1, value));
    row.text = textOf(row, row.value);
    t.seq += 1;
    return { table, seq: t.seq, values: [{ id, value: row.value, text: row.text }] };
  }

  /** A typed value: the number is read as % of the range. Past an end: put at the end, `clamped` says so. Unreadable: `rejected`. */
  type(table: number, id: number, text: string): { values: ParamValues | null; clamped: { name: string; value: string; range: string } | null; rejected: string | null } {
    const row = this.row(table, id);
    if (!row) return { values: null, clamped: null, rejected: null };
    const n = Number.parseFloat(text.trim().replace(/^[−ー]/, "-"));
    if (!Number.isFinite(n)) return { values: null, clamped: null, rejected: row.text ?? "—" };
    const want = n / 100;
    const values = this.set(table, id, want);
    const clamped = want < 0 || want > 1 ? { name: row.name, value: row.text ?? "", range: `${textOf(row, 0)}〜${textOf(row, 1)}` } : null;
    return { values, clamped, rejected: null };
  }
}
