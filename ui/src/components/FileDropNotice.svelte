<!-- ファイルを持ってきている間の知らせ（仕様書 2 章、6 章「SF2 を持ってきている」、モック F5-SF2Drop）。
     窓のふちの内側に accent の line.mark、まん中に幅 400 の箱（地 gray.3、枠 accent の line.hair、内 16）。ファイルの名前（text.l、600）、落とすと何が起きるか、やめかた。
     足せないファイルは、箱の枠と名前を gray.8 にし、文を「SF2 のファイルではないので、足せません。」に。
     ファイルを受けるのは Rust（PC から聞く）。画面は見せるだけで、押せない（下の物をさえぎらない）。Esc は keys/attach.ts。 -->
<script lang="ts">
  import type { FileDropView } from "../bridge";
  import { t } from "../text/ja";

  let { drop }: { drop: FileDropView } = $props();
</script>

<div class="edge" data-test="file-drop">
  <div class="box" class:no={!drop.addable}>
    <span class="name" data-test="file-drop-name">{drop.name}</span>
    <span class="say">{drop.addable ? t("sf2.dropHere") : t("sf2.notSf2")}</span>
    <span class="m-lbl">{t("sf2.dropCancel")}</span>
  </div>
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
</style>
