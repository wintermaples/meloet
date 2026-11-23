// The box of 「くり返す…」 (round 12 A1): which clips it repeats and which clip it sits on belong to the screen only;
// Enter sends one repeatClips. Opened from the clip's right-click menu, 「編集」 and Mod R; Arrange.svelte draws it.

export const repeatBox = $state({ clips: null as number[] | null, at: null as number | null });

/** `at`: the clip the box sits on (the one right-clicked); null for the clip of the group that ends last. */
export function openRepeatBox(clips: readonly number[], at: number | null = null): void {
  repeatBox.clips = [...clips];
  repeatBox.at = at;
}

export function closeRepeatBox(): void {
  repeatBox.clips = null;
  repeatBox.at = null;
}
