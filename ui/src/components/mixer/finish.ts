// 「仕上げのメーター」 (round 12 M3): the numbers it shows, and how often they may change. No DOM.

import { EXPORT_MATCH_LUFS, EXPORT_MATCH_TRUE_PEAK_DB, type Telemetry } from "../../bridge";
import { t } from "../../text/ja";
import { dbNumber } from "../format";

/** The numbers change at most 4 times a second (not every frame: docs/architecture.md 7). */
export const FINISH_TEXT_MS = 250;

export interface FinishTexts {
  /** 「いまの大きさ」: the last 3 s. */
  now: string;
  /** 「曲ぜんたい」: since the play started. */
  whole: string;
  /** 「トゥルーピーク」: the most since the play started. */
  peak: string;
}

/** One decimal with 「−」 (U+2212), 「+」 above 0; none (silence) is 「−∞」. */
export function finishNumber(value: number | null): string {
  if (value === null) return t("common.dbSilentBare");
  const text = dbNumber(value);
  return value > 0 && text !== "0.0" ? `+${text}` : text;
}

export function finishTexts(tm: Pick<Telemetry, "lufsShort" | "lufsIntegrated" | "truePeakDb"> | null): FinishTexts {
  return {
    now: t("mixer.finishLufs", { lufs: finishNumber(tm?.lufsShort ?? null) }),
    whole: t("mixer.finishLufs", { lufs: finishNumber(tm?.lufsIntegrated ?? null) }),
    peak: t("mixer.finishDbtp", { peak: finishNumber(tm?.truePeakDb ?? null) }),
  };
}

/** 「公開の目安は −14 LUFS、−1 dBTP」, from the numbers 「公開先に合わせる」 of the export uses. */
export const finishTarget = (): string =>
  t("mixer.finishTarget", { lufs: String(EXPORT_MATCH_LUFS).replace("-", "−"), peak: String(EXPORT_MATCH_TRUE_PEAK_DB).replace("-", "−") });

/**
 * `run` at once when poked, then at most once every `ms`: a poke within that time waits for its end (one run however many
 * pokes came). `stop` drops a run that waits.
 */
export function calmly(run: () => void, ms: number, now: () => number = () => performance.now()): { poke(): void; stop(): void } {
  let last = -Infinity;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const fire = () => {
    timer = null;
    last = now();
    run();
  };
  return {
    poke() {
      if (timer !== null) return;
      const wait = last + ms - now();
      if (wait <= 0) fire();
      else timer = setTimeout(fire, wait);
    },
    stop() {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    },
  };
}
