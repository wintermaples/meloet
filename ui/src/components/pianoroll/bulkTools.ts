// 「割合」 「だんだん」 「ばらつき」 of 「ベロシティをまとめて変える」 and 「次の音までのばす」 of 「長さをそろえる」 (round 12, P3).
// Pure functions that do not know the DOM: the fields' ranges, reading typed text, and the intents to send. Rust does the maths.

import type { Command, ValueField } from "../../bridge";
import { readVelocityLoose, type FieldRead } from "./noteFields";

export type ToolField = "percent" | "rampFrom" | "rampTo" | "spread";
export type Tool = "percent" | "ramp" | "spread";

interface ToolRange {
  low: number;
  high: number;
  initial: number;
  /** The name the status line gives the field when a value is put at an end or cannot be read. */
  field: ValueField;
}

export const TOOL_FIELDS: Record<ToolField, ToolRange> = {
  percent: { low: 10, high: 300, initial: 100, field: "percent" },
  rampFrom: { low: 1, high: 127, initial: 40, field: "velocity" },
  rampTo: { low: 1, high: 127, initial: 110, field: "velocity" },
  spread: { low: 1, high: 32, initial: 8, field: "amount" },
};

/** The fields each tool reads. */
export const TOOL_INPUTS: Record<Tool, readonly ToolField[]> = {
  percent: ["percent"],
  ramp: ["rampFrom", "rampTo"],
  spread: ["spread"],
};

export const toolInitial = (f: ToolField): string => String(TOOL_FIELDS[f].initial);

/** Typed text to the value to use (put at the nearest end when out of range). Null if it cannot be read. */
export function readTool(f: ToolField, s: string): FieldRead | null {
  const r = readVelocityLoose(s);
  if (!r) return null;
  const { low, high, field } = TOOL_FIELDS[f];
  const value = Math.min(high, Math.max(low, r.value));
  const clamped = value !== r.value ? { field, value: String(value), range: null } : null;
  return { value, clamped };
}

/** ↑ ↓ (Shift: 10 times). It does not go out of range. */
export function stepTool(f: ToolField, s: string, delta: number): string {
  const { low, high, initial } = TOOL_FIELDS[f];
  const from = readTool(f, s)?.value ?? initial;
  return String(Math.min(high, Math.max(low, from + delta)));
}

/** A new seed for each press of 「ばらつき」 「かける」. */
export const newSeed = (): number => Math.floor(Math.random() * 0x1_0000_0000) >>> 0;

/** The intent of a tool with its read values (in the order of TOOL_INPUTS). */
export function toolCommand(tool: Tool, take: number, notes: readonly number[], values: readonly number[], seed: number): Command {
  const n = [...notes];
  switch (tool) {
    case "percent":
      return { type: "scaleVelocity", take, notes: n, percent: values[0]! };
    case "ramp":
      return { type: "rampVelocity", take, notes: n, from: values[0]!, to: values[1]! };
    case "spread":
      return { type: "randomVelocity", take, notes: n, amount: values[0]!, seed };
  }
}
