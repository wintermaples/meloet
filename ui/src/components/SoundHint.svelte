<!-- Hints above the arrange (they sit inside the time area of the arrange, position: relative).
     「音が出ました」: spec 6 "Sound came out", mock F-Connect3. The empty arrange: mock F-Empty. -->
<script lang="ts">
  import { current } from "../state/app.svelte";
  import { t } from "../text";
  import { textParts } from "./format";

  const hint = $derived(current.state?.workspace.hint ?? null);
  /**
   * Sentences after the second are shown only when the record button can be used (spec 6; spec 3 「案内の文は、いま使えない部品を押すようには言わない」).
   * The third sentence of F-Empty (about adding tracks) only when 「＋ 鍵盤のトラック」 and 「＋ 歌のトラック」 can be used (not recording, not counting in).
   */
  const recordUsable = $derived(current.state?.transport.canRecord ?? false);
  const addTrackUsable = $derived(current.state?.transport.state !== "recording" && current.state?.transport.state !== "countIn");
  const keys = $derived(textParts("arrange.soundCameOutKeys", { recordKey: t("key.record"), playKey: t("key.playStop") }));
</script>

{#if hint === "soundCameOut"}
  <div class="hint" data-test="sound-hint">
    <span class="strong">{t("msg.soundCameOut")}</span>{#if recordUsable}{" "}{t("msg.soundCameOutRecord")}{" "}<span
        class="dim"
        >{#each keys as part, i (i)}{#if part.hole !== null}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}{/each}</span
      >{/if}
  </div>
{:else if hint === "emptyArrange"}
  <div class="hint" data-test="empty-hint">
    <span class="strong">{t("arrange.emptyTitle")}</span>
    {t("arrange.emptyBody")}{#if addTrackUsable}{" "}<span class="dim">{t("arrange.emptyAddTrack")}</span>{/if}
  </div>
{/if}

<style>
  .hint {
    position: absolute;
    left: var(--mock-arrange-note-left);
    top: var(--mock-arrange-note-top);
    z-index: 1;
    box-sizing: border-box;
    width: var(--mock-sound-hint-width);
    padding: var(--space-3);
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-7);
    color: var(--color-gray-9);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
    white-space: normal;
  }
  .strong {
    color: var(--color-gray-10);
  }
  .dim {
    color: var(--color-gray-8);
  }
</style>
