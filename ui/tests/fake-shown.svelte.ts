// 部品のテスト用の src/components/pianoroll/shown.svelte の代わり。見せているクリップとテイクを外から入れる。
// 使いかた: vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));
import type { ClipView, TakeNotes, TrackView } from "../src/bridge";

let clip = $state.raw<ClipView | null>(null);
let track = $state.raw<TrackView | null>(null);
let take = $state.raw<TakeNotes | null>(null);

export const shown = {
  get clip(): ClipView | null {
    return clip;
  },
  get track(): TrackView | null {
    return track;
  },
  get take(): TakeNotes | null {
    return take;
  },
  get origin(): TakeNotes | null {
    return null;
  },
};

export function followShownTake(): void {}

export function setShown(next: { clip: ClipView | null; track: TrackView | null; take: TakeNotes | null }): void {
  clip = next.clip;
  track = next.track;
  take = next.take;
}
