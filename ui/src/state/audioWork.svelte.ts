// The progress shown as 「読みこんでいます N %」 and 「音を作っています N %」 (`audioWork` of protocol 24). Three places read it: under the
// clip, the panel of the chosen clip, and the status line. When only the percent changes, it is shown again at most once every
// SHOW_EVERY_MS, and the three places change in the same frame. When a job starts or ends, at once.
// Why: WebKit repainted the whole page each time the percent changed (architecture 22-22, "Step 4 fix (screen: render percent)").

import type { AudioJob, AudioWorkView } from "../bridge";
import { current } from "./app.svelte";

export const SHOW_EVERY_MS = 500;

export interface ShownWork {
  readonly jobs: readonly AudioJob[];
  /** The rendering jobs, by clip id. */
  readonly rendering: ReadonlyMap<number, AudioJob>;
  /** The loading jobs, by audio file number. */
  readonly loading: ReadonlyMap<number, AudioJob>;
  /** The first rendering job that started and has a clip. */
  readonly firstRendering: AudioJob | null;
}

export const NO_WORK: ShownWork = { jobs: [], rendering: new Map(), loading: new Map(), firstRendering: null };

export function shownOf(jobs: readonly AudioJob[]): ShownWork {
  if (jobs.length === 0) return NO_WORK;
  const rendering = new Map<number, AudioJob>();
  const loading = new Map<number, AudioJob>();
  let firstRendering: AudioJob | null = null;
  for (const j of jobs) {
    if (j.kind === "loading") {
      if (!loading.has(j.audio)) loading.set(j.audio, j);
    } else if (j.clip !== null) {
      if (!rendering.has(j.clip)) rendering.set(j.clip, j);
      firstRendering ??= j;
    }
  }
  return { jobs, rendering, loading, firstRendering };
}

const keyOf = (jobs: readonly AudioJob[]) => jobs.map((j) => `${j.kind}/${j.audio}/${j.clip}`).join(" ");
const samePercents = (a: readonly AudioJob[], b: readonly AudioJob[]) => a.every((j, i) => j.percent === b[i]!.percent);

export class WorkShown {
  #tick = $state(0);
  #seen: AudioWorkView | null = null;
  #shown: ShownWork = NO_WORK;
  #key = "";
  #at = -Infinity;
  #timer: ReturnType<typeof setTimeout> | null = null;
  readonly #read: () => AudioWorkView | null;
  readonly #now: () => number;

  constructor(read: () => AudioWorkView | null, now: () => number = () => performance.now()) {
    this.#read = read;
    this.#now = now;
  }

  /** The progress to show now. Whoever reads it reads it again when it is updated. */
  get current(): ShownWork {
    void this.#tick;
    const work = this.#read();
    if (work !== this.#seen) {
      this.#seen = work;
      const jobs = work?.jobs ?? [];
      const key = keyOf(jobs);
      if (key !== this.#key || this.#now() - this.#at >= SHOW_EVERY_MS) this.#take(jobs, key);
      else if (!samePercents(jobs, this.#shown.jobs)) this.#later();
    }
    return this.#shown;
  }

  /** Do not carry the time of the last update from one test to the next. */
  forget(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#seen = null;
    this.#shown = NO_WORK;
    this.#key = "";
    this.#at = -Infinity;
  }

  #take(jobs: readonly AudioJob[], key: string): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    if (key === this.#key && samePercents(jobs, this.#shown.jobs)) return;
    this.#key = key;
    this.#at = this.#now();
    this.#shown = shownOf(jobs);
  }

  #later(): void {
    if (this.#timer !== null) return;
    this.#timer = setTimeout(
      () => {
        this.#timer = null;
        const jobs = this.#read()?.jobs ?? [];
        const before = this.#shown;
        this.#take(jobs, keyOf(jobs));
        if (this.#shown !== before) this.#tick++;
      },
      Math.max(0, this.#at + SHOW_EVERY_MS - this.#now()),
    );
  }
}

export const shownWork = new WorkShown(() => current.state?.audioWork ?? null);
