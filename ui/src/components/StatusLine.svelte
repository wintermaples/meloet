<!-- 状態行（モック F-Connect1〜3・F-BufferFixed のいちばん下の行）。 -->
<script lang="ts">
  import type { Command, StatusMessage } from "../bridge";
  import { term } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { t, type TextKey } from "../text/ja";
  import { msText, textParts, type TextPart } from "./format";

  const app = $derived(current.state!);

  interface Shown {
    parts: TextPart[];
    /** 値を gray.10 で出す穴。 */
    strong: string[];
    button: { label: string; command: Command } | null;
  }

  // 録音待機と言う文（status.outputChosen・status.newSong）は、今回は本当でないので出さない（docs/progress.md）。
  function shown(m: StatusMessage | null): Shown | null {
    const plain = (key: TextKey, values: Record<string, string | number> = {}): Shown => ({
      parts: textParts(key, values),
      strong: [],
      button: null,
    });
    const settings = (advanced: boolean, label: string) => ({ label, command: { type: "openSettings", advanced } as Command });
    switch (m?.id) {
      case "searchingKeyboard":
        return plain("status.searchingKeyboard");
      case "keyboardFound":
        return plain("msg.keyboardFound", { keyboard: m.keyboard });
      case "keyboardLost":
        return plain("msg.keyboardLost", { keyboard: m.keyboard });
      case "outputNotPlugged":
      case "outputMissing":
        return { ...plain("msg.outputNotPlugged", { output: m.output }), button: settings(false, t("button.openAudioSettings")) };
      case "bufferFixed":
        return {
          parts: textParts("msg.bufferFixedStatus", {
            actual: m.actual,
            requested: m.requested,
            latencyMs: msText(m.latencyMs),
            latencyMsAtRequested: msText(m.latencyMsAtRequested),
          }),
          strong: ["actual", "latencyMs"],
          button: settings(true, t("button.details")),
        };
      case "settingsOpen":
        return plain("status.settingsOpen");
      default:
        return null;
    }
  }

  const message = $derived(shown(app.status.message));

  const outputName = $derived(app.outputs.list.find((o) => o.id === app.outputs.chosen)?.name ?? null);
  const keyboardName = $derived(app.keyboards.list.find((k) => k.id === app.keyboards.active)?.name ?? null);
  const actual = $derived(app.audio.actualFrames);
  const requested = $derived(app.audio.requestedFrames);
</script>

<footer class="status">
  <span>{t("status.stopped")}</span>
  {#if message}
    <span class="m-lbl message">
      {#each message.parts as part, i (i)}
        {#if part.hole !== null}
          <span class="m-num" class:m-hi={message.strong.includes(part.hole)}>{part.text}</span>
        {:else}{part.text}{/if}
      {/each}
    </span>
    {#if message.button}
      {@const command = message.button.command}
      <button class="m-plain m-hi link" onclick={() => send(command)}>{message.button.label}</button>
    {/if}
  {/if}
  <span class="m-spacer"></span>
  <span class="m-lbl">{outputName ? t("status.output", { output: outputName }) : t("status.outputNone")}</span>
  <span class="m-lbl">{keyboardName ? t("status.inputs", { inputs: keyboardName }) : t("status.inputsNone")}</span>
  {#if actual !== null}
    <span class="m-lbl" use:term={"バッファ"}>
      {#if actual !== requested}
        {#each textParts("status.bufferMismatch", { actual, requested }) as part, i (i)}
          {#if part.hole !== null}<span class="m-num" class:m-hi={part.hole === "actual"}>{part.text}</span>{:else}{part.text}{/if}
        {/each}
      {:else}
        {#each textParts("status.buffer", { actual }) as part, i (i)}
          {#if part.hole !== null}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}
        {/each}
      {/if}
    </span>
  {/if}
</footer>

<style>
  .status {
    height: var(--size-status);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: 0 var(--space-3);
    background: var(--color-gray-0);
    border-top: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .message {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .link {
    flex-shrink: 0;
    text-decoration: underline;
  }
</style>
