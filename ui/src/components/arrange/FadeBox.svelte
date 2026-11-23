<!-- The box of 「曲の終わりにフェードアウトをつける…」 (round 12 M5): digits on the bar ruler, ending at the song's end, with the look of
     the repeat box. 4 at first and chosen; Enter sends one addFadeOut (a number outside 1 to 16 goes to the nearer end and the status line
     says so), Esc or a press outside closes it, ↑ ↓ change it by 1 (10 with Shift). While it is open the status line says what Enter does. -->
<script lang="ts">
  import { onDestroy, tick as nextTick } from "svelte";
  import { FADE_OUT_BARS_DEFAULT, FADE_OUT_BARS_MAX, type TrackView } from "../../bridge";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text";
  import type { TempoMap } from "../../time/tempoMap";
  import { tickToX, type View } from "../arrangeGeom";
  import { dragReading } from "../pianoroll/view.svelte";
  import { readBars } from "../valueClamp";
  import { closeFadeBox, fadeBox, songEndTick } from "./fadeBox.svelte";

  interface Props {
    tracks: readonly TrackView[];
    view: View;
    map: TempoMap;
    /** Counting in or recording, or the arrange folded into the overview: the box closes. */
    shut: boolean;
  }
  let { tracks, view, map, shut }: Props = $props();

  let width = $state(0);
  const x = $derived(Math.min(Math.max(tickToX(view, songEndTick(tracks, map)) - width, 0), Math.max(0, view.width - width)));
  let draft = $state("");
  let field = $state<HTMLInputElement>();
  let was = false;
  const read = $derived(readBars(draft, FADE_OUT_BARS_MAX));
  $effect(() => {
    const open = fadeBox.open;
    if (open === was) return;
    was = open;
    if (!open) return;
    draft = String(FADE_OUT_BARS_DEFAULT);
    void nextTick().then(() => {
      field?.focus();
      field?.select();
    });
  });
  $effect(() => {
    if (fadeBox.open && shut) closeFadeBox();
  });
  $effect(() => {
    if (!fadeBox.open) return;
    dragReading.text = read === null ? null : t("arrange.readFadeOut", { count: read.value });
    return () => {
      dragReading.text = null;
    };
  });
  onDestroy(closeFadeBox);
  function key(e: KeyboardEvent): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      // A box that is not a whole number stays open.
      if (read !== null) {
        closeFadeBox();
        send({ type: "addFadeOut", bars: read.value });
        if (read.clamp) send({ type: "clampValue", ...read.clamp });
      }
    } else if (e.key === "Escape") {
      closeFadeBox();
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const by = (e.key === "ArrowUp" ? 1 : -1) * (e.shiftKey ? 10 : 1);
      draft = String(Math.min(FADE_OUT_BARS_MAX, Math.max(1, (read?.value ?? FADE_OUT_BARS_DEFAULT) + by)));
    } else {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#if fadeBox.open && !shut}
  <div class="fade-box" role="presentation" style:left="{x}px" bind:clientWidth={width} onpointerdown={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
    <input
      class="fade-field"
      class:broken={read === null}
      type="text"
      inputmode="numeric"
      spellcheck="false"
      autocomplete="off"
      aria-label={t("arrange.fadeOutBars")}
      aria-invalid={read === null}
      data-test="fade-out-field"
      bind:this={field}
      bind:value={draft}
      onkeydown={key}
      onblur={closeFadeBox}
    />
  </div>
{/if}

<style>
  /* The look of the repeat box of the arrange and of the box of 「この前に空きを入れる…」, two digits wide. */
  .fade-box {
    position: absolute;
    z-index: 4;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
  }
  .fade-field {
    box-sizing: border-box;
    width: calc(2ch + 2 * var(--space-1) + 2 * var(--line-hair));
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    background: var(--color-gray-0);
    color: var(--color-gray-10);
    font: inherit;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-weight: var(--text-s-weight-600);
    outline: none;
  }
  .fade-field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
</style>
