<!-- ファイルを持ってきている間の知らせ（仕様書 2 章、6 章「SF2 を持ってきている」、モック F5-SF2Drop）。
     窓のふちの内側に accent の line.mark、まん中に幅 400 の箱（地 gray.3、枠 accent の line.hair、内 16）。ファイルの名前（text.l、600）、落とすと何が起きるか、やめかた。
     足せないファイルは、箱の枠と名前を gray.8 にし、文を「SF2 のファイルではないので、足せません。」に。
     いくつも持ってきたときは、名前の所が「3 つのファイル ・ SF2 は 2 つ」（仕様書 3 章 89-10）。
     音のファイル（仕様書 3 章 29-A ・ 30-C ・「読めるファイル」、モック F7-AudioDrop）: 2 つ以上なら箱を左右 240 ずつに分け（左「別々のトラックに」、右「1 本にならべる」）、
     ポインタがいる側（窓の左半分か右半分。B-94-21）を accent の枠と gray.4 の地に。SF2 もまざっていれば「SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ」。
     ファイルを受けるのは Rust（PC から聞く）。画面は見せるだけで、押せない（下の物をさえぎらない）。Esc は keys/attach.ts。 -->
<script lang="ts">
  import type { FileDropView } from "../bridge";
  import { t } from "../text/ja";
  import { dropModeFor, fileDrag } from "./arrange/fileDrag.svelte";

  let { drop }: { drop: FileDropView } = $props();
  const title = $derived(
    drop.files <= 1 ? drop.name : drop.sf2 > 0 ? t("sf2.manyFiles", { files: drop.files, sf2: drop.sf2 }) : t("sf2.manyFilesNoSf2", { files: drop.files }),
  );
  /** SF2 と音のファイルをまぜて持ってきた: SF2 は棚へ、音のファイルはアレンジへ。 */
  const mixed = $derived(drop.audio > 0 && drop.sf2 > 0 ? t("drop.mixed", { sf2: drop.sf2, audio: drop.audio }) : null);
  /** 箱を左右に分けるか、分けたならポインタがいる側。 */
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
    border: var(--line-mark) solid var(--color-accent);
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
    border: var(--line-hair) solid var(--color-accent);
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
  /* 分かれた箱（仕様書 2 章「持ってきている間の、分かれた箱」）: 乗っていない側は地 gray.3 ・ 枠 gray.7、文字は gray.8。 */
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
    border-color: var(--color-accent);
    color: var(--color-gray-9);
  }
  /* 名前は F5-SF2Drop の箱と同じ text.l の 600（モックは 16 px の 600。text.m に 600 はない）。 */
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
