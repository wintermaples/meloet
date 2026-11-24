<!-- Notice while files are dragged in (spec 2; spec 6 "Dragging in an SF2"; mock F5-SF2Drop).
     line.mark in accent inside the window edge, and a 400 wide box in the middle (bg gray.3, border line.hair in accent, padding 16). The file name (text.l, 600), what dropping does, and how to cancel.
     For a file that cannot be added, the box border and the name are gray.8 and the text is 「SF2 のファイルではないので、足せません。」.
     For many files, the name part is 「3 つのファイル ・ SF2 は 2 つ」 (spec 3 89-10).
     Audio files (spec 3 29-A, 30-C, "Files that can be read"; mock F7-AudioDrop): for 2 or more, the box splits into 240 left and right (left 「別々のトラックに」, right 「1 本にならべる」),
     and the side the pointer is on (left or right half of the window; B-94-21) gets an accent border and gray.4 bg. With SF2 mixed in: 「SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ」.
     For one file, under the name, where it goes when dropped (spec 6 "Dragging in one audio file": onto a vocal track, or a new vocal track).
     Rust receives the files (it asks the PC). The screen only shows this; it cannot be pressed (it does not block what is under it). Esc is in keys/attach.ts. -->
<script lang="ts">
  import type { FileDropView } from "../bridge";
  import { t } from "../text";
  import { dropModeFor, dropPlaceAt, fileDrag } from "./arrange/fileDrag.svelte";

  let { drop }: { drop: FileDropView } = $props();
  const title = $derived(
    drop.files <= 1 ? drop.name : drop.sf2 > 0 ? t("sf2.manyFiles", { files: drop.files, sf2: drop.sf2 }) : t("sf2.manyFilesNoSf2", { files: drop.files }),
  );
  /** SF2 and audio files mixed: SF2 go to the shelf, audio files to the arrange. */
  const mixed = $derived(drop.audio > 0 && drop.sf2 > 0 ? t("drop.mixed", { sf2: drop.sf2, audio: drop.audio }) : null);
  /** Only one audio file: whether dropping puts it on the vocal track under the pointer. */
  const intoTrack = $derived.by(() => {
    if (drop.audio !== 1 || drop.sf2 > 0) return null;
    const at = fileDrag.at;
    if (at === null) return null;
    return dropPlaceAt(at.x, at.y, "inRow")?.track != null;
  });
  /** Whether the box is split, and if so, the side the pointer is on. */
  const halves = $derived(drop.audio >= 2 ? (fileDrag.at ? dropModeFor(drop.audio, fileDrag.at.x, window.innerWidth) : null) : undefined);
</script>

<div class="edge" data-test="file-drop">
  {#if halves !== undefined}
    <div class="split">
      <div class="halves">
        <div class="half" class:on={halves === "separate"} data-test="file-drop-separate">
          <span class="half-name">{t("drop.separate")}</span>
          <span class="say">{t("drop.separateSay", { files: drop.audio, tracks: drop.audio })}</span>
        </div>
        <div class="half" class:on={halves === "inRow"} data-test="file-drop-in-row">
          <span class="half-name">{t("drop.inRow")}</span>
          <span class="say">{t("drop.inRowSay")}</span>
        </div>
      </div>
      {#if mixed}<div class="half mixed" data-test="file-drop-mixed"><span class="say">{mixed}</span></div>{/if}
    </div>
  {:else}
    <div class="box" class:no={!drop.addable}>
      <span class="name" data-test="file-drop-name">{title}</span>
      {#if mixed}
        <span class="say" data-test="file-drop-mixed">{mixed}</span>
      {:else if drop.audio === 0}
        <span class="say">{drop.addable ? t("sf2.dropHere") : t("sf2.notSf2")}</span>
      {:else if intoTrack !== null}
        <span class="say" data-test="file-drop-audio-say">{t(intoTrack ? "drop.audioOneSay" : "drop.audioOneNewTrack")}</span>
      {/if}
      <span class="m-lbl">{t("sf2.dropCancel")}</span>
    </div>
  {/if}
</div>

<style>
  .edge {
    position: fixed;
    inset: 0;
    z-index: 40;
    pointer-events: none;
    border: var(--line-mark) solid var(--color-accent-ink);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .box {
    width: var(--part-filedrop-width);
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4);
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-accent-ink);
  }
  .name {
    font-size: var(--text-l-size);
    line-height: var(--text-l-line-height);
    font-weight: var(--text-l-weight-600);
    color: var(--color-gray-10);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .say {
    white-space: normal;
  }
  .box.no {
    border-color: var(--color-gray-8);
  }
  .box.no .name {
    color: var(--color-gray-8);
  }
  .split {
    display: flex;
    flex-direction: column;
  }
  .halves {
    display: flex;
  }
  /* Split box (spec 2 "Split box while dragging in"): the side not under the pointer has bg gray.3, border gray.7, text gray.8. */
  .half {
    box-sizing: border-box;
    width: var(--part-filedrop-half);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4);
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-7);
    color: var(--color-gray-8);
  }
  .half.on {
    background: var(--color-gray-4);
    border-color: var(--color-accent-ink);
    color: var(--color-gray-9);
  }
  /* The name is text.l at 600, the same as the F5-SF2Drop box (the mock is 16 px at 600; text.m has no 600). */
  .half-name {
    font-size: var(--text-l-size);
    line-height: var(--text-l-line-height);
    font-weight: var(--text-l-weight-600);
    color: var(--color-gray-9);
    white-space: nowrap;
  }
  .half.on .half-name {
    color: var(--color-gray-10);
  }
  .half.mixed {
    width: calc(2 * var(--part-filedrop-half));
    border-top: 0;
  }
</style>
