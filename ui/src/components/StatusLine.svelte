<!-- 状態行（モック F-Connect1〜3・F-BufferFixed・F-Zoom のいちばん下の行）。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { Band, Command, StatusMessage } from "../bridge";
  import type { Scale } from "../design/tokens";
  import { platformOf } from "../keys/keys";
  import { menuOpened } from "../lib/menu";
  import { term } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { t, type TextKey } from "../text/ja";
  import { msText, textParts, type TextPart } from "./format";
  import { statusFit, type StatusRightShown } from "./statusFit";
  import ZoomMenu from "./ZoomMenu.svelte";

  const app = $derived(current.state!);

  interface Shown {
    parts: TextPart[];
    /** 値を gray.10 で出す穴。 */
    strong: string[];
    button: { label: string; command: Command } | null;
  }

  const BAND_NAMES: Record<Band, TextKey> = {
    pianoRoll: "band.pianoRoll",
    mixer: "band.mixer",
    history: "band.history",
    shelf: "band.shelf",
    params: "band.params",
  };

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
      case "scaleDoesNotFit":
        return plain("msg.scaleKept", { requested: m.requested, kept: m.kept });
      case "scaleWindowGrown":
        return plain("msg.scaleWindowGrown", { scale: m.scale });
      case "scaleLoweredForScreen":
        return plain("msg.scaleLowered", { from: m.from, to: m.to });
      case "bandFoldedWindowLow":
        return plain("msg.bandFoldedWindowLow", { band: t(BAND_NAMES[m.band]) });
      case "bandFoldedToOpen":
        return plain("msg.bandFoldedToOpen", { opened: t(BAND_NAMES[m.opened]), folded: t(BAND_NAMES[m.folded]) });
      default:
        return null;
    }
  }

  const message = $derived(shown(app.status.message));

  const outputName = $derived(app.outputs.list.find((o) => o.id === app.outputs.chosen)?.name ?? null);
  const keyboardName = $derived(app.keyboards.list.find((k) => k.id === app.keyboards.active)?.name ?? null);
  const actual = $derived(app.audio.actualFrames);
  const requested = $derived(app.audio.requestedFrames);
  const scale = $derived(app.workspace.scale);
  const unfit = $derived(app.workspace.unfitScales);

  // 拡大率の一覧。画面だけが持つ「ひらいている」。率を決めるのは core（setScale を送るだけ）。
  let open = $state(false);
  let zoomButton = $state<HTMLButtonElement>();
  let popup = $state<HTMLElement>();
  const platform = platformOf(navigator);

  // 一覧はダイアログより手前に出すので、.app の重なりの外（body）に置き、状態行の右上にそろえる。
  let anchor = $state({ right: 0, bottom: 0 });
  function place(): void {
    if (!row) return;
    const r = row.getBoundingClientRect();
    anchor = { right: window.innerWidth - r.right, bottom: window.innerHeight - r.top };
  }
  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  function closeZoom(): void {
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    open = false;
    if (hadFocus) zoomButton?.focus();
  }

  function pick(s: Scale): void {
    send({ type: "setScale", percent: s });
    closeZoom();
  }

  $effect(() => {
    if (!open) return;
    place();
    window.addEventListener("resize", place);
    const close = () => closeZoom();
    const forget = menuOpened(close);
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || zoomButton?.contains(target))) return;
      close();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", place);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });

  // 右がわの入れかた: 見えない写し（ruler）で、ちぢめない幅を測る。
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let fit = $state<StatusRightShown>({ inputs: true, output: true, buffer: true });

  function measure(): void {
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    if (open) place();
    const available = row.clientWidth - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0);
    const width = (part: string) => ruler!.querySelector<HTMLElement>(`[data-part="${part}"]`)?.offsetWidth;
    fit = statusFit({
      available,
      gap,
      left: width("left") ?? 0,
      right: { inputs: width("inputs"), output: width("output"), buffer: width("buffer") },
      scale: width("scale") ?? 0,
    });
  }

  onMount(() => {
    const observer = new ResizeObserver(() => measure());
    if (row) observer.observe(row);
    if (ruler) observer.observe(ruler);
    return () => observer.disconnect();
  });
  $effect(() => {
    void [message, outputName, keyboardName, actual, requested, scale];
    measure();
  });
</script>

{#snippet left()}
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
{/snippet}

{#snippet output()}{outputName ? t("status.output", { output: outputName }) : t("status.outputNone")}{/snippet}
{#snippet inputs()}{keyboardName ? t("status.inputs", { inputs: keyboardName }) : t("status.inputsNone")}{/snippet}
{#snippet buffer()}
  {#if actual !== requested}
    {#each textParts("status.bufferMismatch", { actual: actual!, requested }) as part, i (i)}
      {#if part.hole !== null}<span class="m-num" class:m-hi={part.hole === "actual"}>{part.text}</span>{:else}{part.text}{/if}
    {/each}
  {:else}
    {#each textParts("status.buffer", { actual: actual! }) as part, i (i)}
      {#if part.hole !== null}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}
    {/each}
  {/if}
{/snippet}
{#snippet zoom()}<span class="m-num rate">{t("status.scale", { scale })}</span>▾{/snippet}

<div class="status-line">
  {#if open}
    <div class="popup" style:right="{anchor.right}px" style:bottom="{anchor.bottom}px" use:toBody bind:this={popup}>
      <ZoomMenu {scale} {unfit} {platform} onpick={pick} />
    </div>
  {/if}
  <footer class="status" bind:this={row}>
    {@render left()}
    <span class="m-spacer"></span>
    {#if fit.output}<span class="m-lbl" data-test="status-output">{@render output()}</span>{/if}
    {#if fit.inputs}<span class="m-lbl" data-test="status-inputs">{@render inputs()}</span>{/if}
    {#if actual !== null && fit.buffer}
      <span class="m-lbl" data-test="status-buffer" use:term={"バッファ"}>{@render buffer()}</span>
    {/if}
    <button
      class="m-plain zoom"
      class:open
      aria-label={t("zoom.heading")}
      aria-haspopup="menu"
      aria-expanded={open}
      data-test="status-scale"
      bind:this={zoomButton}
      onclick={() => (open = !open)}>{@render zoom()}</button
    >
  </footer>
  <div class="ruler" aria-hidden="true" bind:this={ruler}>
    <span class="group" data-part="left">{@render left()}</span>
    <span class="m-lbl" data-part="output">{@render output()}</span>
    <span class="m-lbl" data-part="inputs">{@render inputs()}</span>
    {#if actual !== null}<span class="m-lbl" data-part="buffer">{@render buffer()}</span>{/if}
    <span class="zoom wide" data-part="scale">{@render zoom()}</span>
  </div>
</div>

<style>
  .status-line {
    position: relative;
    flex-shrink: 0;
  }
  /* 重なり: ダイアログ（30）より上、用語の説明（1000）より下。 */
  .popup {
    position: fixed;
    z-index: 40;
    margin-right: var(--space-3);
  }
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
  .status > :not(.message):not(.m-spacer) {
    flex-shrink: 0;
  }

  /* 拡大率: ふつう（地も枠も内がわもなし。モック Main）/ 乗った（文字 gray.10）/ ひらいている（地 gray.4、枠 gray.8。モック F-Zoom）。 */
  .zoom {
    height: 100%;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    white-space: nowrap;
  }
  .rate {
    color: var(--color-gray-10);
  }
  button.zoom:hover {
    color: var(--color-gray-10);
  }
  button.zoom.open {
    padding: 0 var(--part-button-pad-x);
    background: var(--color-gray-4);
    border: var(--line-hair) solid var(--color-gray-8);
    color: var(--color-gray-10);
  }

  /* 写しでは、ひらいたときの幅（内がわと枠つき）で測る。 */
  .ruler .wide {
    padding: 0 var(--part-button-pad-x);
    border: var(--line-hair) solid transparent;
  }
  .ruler {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    gap: var(--space-4);
    width: max-content;
    height: var(--size-status);
    visibility: hidden;
    pointer-events: none;
    white-space: nowrap;
  }
  .ruler > *,
  .group {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
  }
  .group {
    gap: var(--space-4);
  }
</style>
