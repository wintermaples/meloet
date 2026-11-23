<!-- The list of 「進行から並べる ▾」 (round 12, P6; the picture in the spec). A row: ▶ (plays the chords), the name, the chords in the scale.
     ↑ ↓ or a press choose a row; Enter or 「並べる」 puts the chosen one. On Esc or a press outside, the opener closes it. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { MusicalScale } from "../../bridge";
  import { t } from "../../text/ja";
  import { PROGRESSIONS, progressionText, type Progression } from "./chords";

  interface Props {
    scale: Pick<MusicalScale, "root" | "mode">;
    /** The row whose ▶ is playing. */
    playing: Progression | null;
    onlisten: (id: Progression) => void;
    onplace: (id: Progression) => void;
  }
  let { scale, playing, onlisten, onplace }: Props = $props();

  let chosen = $state<Progression>(PROGRESSIONS[0]!.id);
  let list = $state<HTMLElement>();
  onMount(() => queueMicrotask(() => list?.focus()));

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.altKey || ev.ctrlKey || ev.metaKey || ev.shiftKey || ev.isComposing) return;
    const i = PROGRESSIONS.findIndex((p) => p.id === chosen);
    if (ev.key === "ArrowUp") chosen = PROGRESSIONS[Math.max(0, i - 1)]!.id;
    else if (ev.key === "ArrowDown") chosen = PROGRESSIONS[Math.min(PROGRESSIONS.length - 1, i + 1)]!.id;
    else if (ev.key === "Enter") onplace(chosen);
    else return;
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

{#snippet play()}<svg class="icon" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l8 5-8 5z" /></svg>{/snippet}
{#snippet stop()}<svg class="icon" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1h8v8h-8z" /></svg>{/snippet}

<div class="menu" role="listbox" tabindex="-1" aria-label={t("pr.progressionOpen")} bind:this={list} {onkeydown} data-test="pr-progressions">
  {#each PROGRESSIONS as p (p.id)}
    {@const on = chosen === p.id}
    <div class="row" class:on role="option" aria-selected={on} tabindex="-1" data-row={p.id} onpointerdown={() => (chosen = p.id)}>
      <button
        class="listen"
        class:playing={playing === p.id}
        aria-label={t("pr.progressionListen")}
        data-test="pr-progression-listen"
        onclick={() => onlisten(p.id)}>{#if playing === p.id}{@render stop()}{:else}{@render play()}{/if}</button
      >
      <span class="name">{t(p.name)}</span>
      <span class="m-spacer"></span>
      <span class="m-num m-lbl chords">{progressionText(p.id, scale)}</span>
    </div>
  {/each}
  <div class="foot">
    <span class="m-lbl">{t("pr.progressionHint")}</span>
    <span class="m-spacer"></span>
    <button class="m-btn" data-test="pr-progression-place" onclick={() => onplace(chosen)}>{t("pr.progressionPlace")}</button>
  </div>
</div>

<style>
  .menu {
    min-width: var(--part-list-width);
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    outline: none;
  }
  .row,
  .foot {
    height: var(--part-list-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .foot {
    border-bottom: 0;
  }
  .row:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* The chosen row (spec 2 「選んだ行」: bg gray.4, a line.mark accent on the left). */
  .row.on {
    background: var(--color-gray-4);
    box-shadow: inset var(--line-mark) 0 0 0 var(--color-accent);
  }
  .name {
    color: var(--color-gray-8);
  }
  .row.on .name,
  .row:hover .name {
    color: var(--color-gray-10);
  }
  .listen {
    width: var(--part-shelfrow-sample);
    height: var(--part-shelfrow-sample);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .listen:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .listen.playing {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .icon {
    width: var(--part-button-icon);
    height: var(--part-button-icon);
    fill: currentColor;
  }
</style>
