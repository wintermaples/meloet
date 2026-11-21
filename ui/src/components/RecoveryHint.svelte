<!-- なおすためのファイルの案内（仕様書 3 章・6 章、モック F-Recover1・F-Recover3）。流れ 3 の案内と同じ置き場と形。
     保存するか、「戻さない」か「閉じる」を押すまで出ている（決めるのは Rust）。 -->
<script lang="ts">
  import { platformOf } from "../keys/keys";
  import { term } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import Button from "./Button.svelte";
  import { dayAndTime, termParts } from "./format";
  import { mapOf } from "../time/bars";
  import { positionText } from "./time";

  const TERM = "なおすためのファイル";
  const recovery = $derived(current.state?.workspace.recovery ?? null);
  const platform = platformOf(navigator);
  const saveKey = `${platform === "mac" ? t("key.cmd") : t("key.ctrl")} ${t("key.saveSong")}`;

  // 録っていた歌を残したとき（仕様書 3 章「録っている途中で PC が終わったとき」）の 1 行。
  const vocalLine = $derived.by((): string | null => {
    const timeline = current.state?.timeline;
    if (recovery?.kind !== "recovered" || !recovery.vocals || !timeline) return null;
    return t("recovery.vocalKept", { count: recovery.vocals.count, position: positionText(recovery.vocals.until, mapOf(timeline)) });
  });

  const lines = $derived.by((): { strong: string; dim: string } | null => {
    if (!recovery) return null;
    const song = recovery.song ?? t("transport.songNew");
    if (recovery.kind === "unreadable") {
      return {
        strong: t("msg.recoveryUnreadable"),
        dim: recovery.song === null ? t("msg.recoveryUnreadableNew") : t("msg.recoveryUnreadableSaved", { song }),
      };
    }
    const values = { ...dayAndTime(recovery.writtenMs), song };
    const takes = recovery.takes > 0 ? t("msg.recoveredTakes", { takes: recovery.takes }) : "";
    return recovery.cause === "pcEnding"
      ? { strong: t("msg.recoveredPcEnding"), dim: t("msg.recoveredPcEndingDetail", values) + takes }
      : { strong: t("msg.recoveredSudden"), dim: t("msg.recoveredSuddenDetail", values) + takes };
  });
</script>

{#if recovery && lines}
  <div class="hint" data-test="recovery-hint">
    <span class="strong"
      >{#each termParts(lines.strong, TERM) as part, i (i)}{#if part.term}<span use:term={TERM}>{part.text}</span
          >{:else}{part.text}{/if}{/each}</span
    >
    <span class="dim">{lines.dim}</span>
    {#if vocalLine}<span data-test="recovery-vocals">{vocalLine}</span>{/if}
    <div class="buttons">
      {#if recovery.kind === "recovered"}
        <Button primary test="recovery-save" onclick={() => send({ type: "saveSong" })}
          >{t("button.save")}<span class="key">{saveKey}</span></Button
        >
        <Button test="recovery-discard" onclick={() => send({ type: "discardRecovered" })}>{t("button.dontRestore")}</Button>
      {:else}
        <Button test="recovery-show-file" onclick={() => send({ type: "showRecoveryFile" })}>{t("button.openFileLocation")}</Button>
        <Button test="recovery-close" onclick={() => send({ type: "dismissHint" })}>{t("button.close")}</Button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .hint {
    position: absolute;
    left: var(--mock-arrange-note-left);
    top: var(--mock-arrange-note-top);
    z-index: 1;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    width: var(--mock-recovery-hint-width);
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
  .buttons {
    display: flex;
    gap: var(--space-2);
  }
  .key {
    font-family: var(--font-mono);
    color: var(--color-gray-8);
  }
</style>
