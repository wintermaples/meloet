// 「読みこんでいます N %」「音を作っています N %」で見せる進み（約束 24 の audioWork）。クリップの下、選んだクリップの欄、状態行の 3 か所が
// これを読む。% だけが変わったときは SHOW_EVERY_MS に 1 回まで見せかえ、3 か所は同じコマで変わる。仕事が始まった ・ 終わった時はすぐ。
// 理由: % が変わるたびに WebKit がページをまるごと描きなおしていた（docs/architecture.md 22-19「区切り4 の直し（画面: 音を作っている %）」）。

import type { AudioJob, AudioWorkView } from "../bridge";
import { current } from "./app.svelte";

export const SHOW_EVERY_MS = 500;

export interface ShownWork {
  readonly jobs: readonly AudioJob[];
  /** 音を作っている仕事を、クリップの id で。 */
  readonly rendering: ReadonlyMap<number, AudioJob>;
  /** 読みこんでいる仕事を、音のファイルの番号で。 */
  readonly loading: ReadonlyMap<number, AudioJob>;
  /** いちばん先に始めた、クリップのある「音を作っている」。 */
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

  /** いま見せる進み。読んだ所は、見せかえたときに読みなおされる。 */
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

  /** テストの間で、前の見せかえの時刻を持ちこさない。 */
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
