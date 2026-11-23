<!-- 「ミュートのセット ▾」 at the head of the mixer band (B-97-3): opens the list of muteSets.svelte.ts under it. It came after the
     parts the spec shows in that row, so it is shown only in the room they leave (it folds away first); then 「トラック ▸ ミュートのセット ▸」
     is the way in. A hidden copy keeps its width for the fit. -->
<script lang="ts">
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import Button from "../Button.svelte";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { muteSetEntries } from "./muteSets.svelte";

  let ruler = $state<HTMLElement>();
  let slot = $state<HTMLElement>();
  let fits = $state(true);

  /** The room the other parts of the row leave, against the button's width and the gap before it. */
  function measure(): void {
    const row = ruler?.parentElement;
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    let used = (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0);
    let parts = 0;
    for (const el of row.children) {
      if (!(el instanceof HTMLElement) || el === ruler || el === slot || el.classList.contains("m-spacer")) continue;
      used += el.offsetWidth;
      parts += 1;
    }
    // The spacer is a part too, with its gap.
    used += gap * parts;
    const next = used + ruler.offsetWidth + gap <= row.clientWidth;
    if (next !== fits) fits = next;
  }

  $effect(() => {
    const row = ruler?.parentElement;
    if (!row || !ruler) return;
    const seen = new ResizeObserver(measure);
    seen.observe(row);
    seen.observe(ruler);
    measure();
    return () => seen.disconnect();
  });

  function open(): void {
    if (!slot) return;
    const r = slot.getBoundingClientRect();
    openContextMenu(r.left, r.bottom, muteSetEntries(current.state!, send, { x: r.left, y: r.bottom }));
  }
</script>

<span class="ruler" aria-hidden="true" bind:this={ruler}><Button>{t("mixer.muteSets")}</Button></span>
{#if fits}
  <span class="slot" data-test="mute-sets" bind:this={slot}><Button onclick={open} test="mute-sets-button">{t("mixer.muteSets")}</Button></span>
{/if}

<style>
  .ruler {
    position: absolute;
    visibility: hidden;
    pointer-events: none;
  }
  .slot {
    flex-shrink: 0;
    display: flex;
  }
</style>
