<!-- The row of the chords of the key, under the tool row (mock P10-ChordTable, option A). 24 high.
     With the scale fixed, a button for each chord of the key: pressing one sounds it while held and makes it the chord of
     the draw tool. Otherwise the row says how to get them. Disabled on drums.
     With the scale fixed, 「進行から並べる ▾」 at the right end opens the list of progressions (round 12, P6). -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { menuOpened } from "../../lib/menu";
  import { mapOf } from "../../time/bars";
  import { term } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { scaleName } from "../format";
  import { Auditioner, fixedOr100 } from "./audition";
  import { chooseChord, chordPick } from "./chordPick.svelte";
  import { auditionKeys, keyChords, progressionChords, samePick, triadName, type ChordPick, type Progression } from "./chords";
  import { rollShowsDrums } from "./drums";
  import { ChordSequence, chordMs, firstBar, progressionNotes } from "./progression";
  import ProgressionMenu from "./ProgressionMenu.svelte";
  import { shown } from "./shown.svelte";

  const app = $derived(current.state!);
  const scale = $derived(app.pianoRoll.scale);
  const drums = $derived(!!rollShowsDrums(app, shown.clip, shown.track));
  const chords = $derived(
    keyChords(scale).map((c) => ({ pick: { kind: "key", root: c.root, triad: c.triad } satisfies ChordPick, name: triadName(c.root, c.triad) })),
  );

  const audition = new Auditioner((c) => void send(c));
  onDestroy(() => audition.stop());

  function press(e: PointerEvent, pick: Extract<ChordPick, { kind: "key" }>): void {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    chooseChord(pick);
    audition.play(auditionKeys(pick.root, pick.triad), fixedOr100(app));
  }
  const release = () => audition.stop();

  // ---- 「進行から並べる ▾」 ----

  let open = $state(false);
  let opener = $state<HTMLButtonElement>();
  let popup = $state<HTMLElement>();
  let place = $state({ right: 0, top: 0 });
  let playing = $state<Progression | null>(null);
  const sequence = new ChordSequence(
    (c) => void send(c),
    (id) => (playing = id as Progression | null),
  );
  onDestroy(() => sequence.stop());

  function placeMenu(): void {
    if (!opener) return;
    const r = opener.getBoundingClientRect();
    place = { right: window.innerWidth - r.right, top: r.bottom };
  }

  function closeList(): void {
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    open = false;
    if (hadFocus) opener?.focus();
  }

  function listen(id: Progression): void {
    if (playing === id) sequence.stop();
    else sequence.play(id, progressionChords(id, scale), fixedOr100(app), chordMs(app.timeline.tempoBpm));
  }

  function placeChords(id: Progression): void {
    const clip = shown.clip;
    const map = mapOf(app.timeline);
    const first = firstBar(map, app.transport.position, clip);
    const notes = progressionNotes(progressionChords(id, scale), first, map, fixedOr100(app));
    send({ type: "addNotesAt", take: clip?.takeId ?? null, notes });
    closeList();
  }

  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  $effect(() => {
    if (!open) return;
    placeMenu();
    window.addEventListener("resize", placeMenu);
    const forget = menuOpened(() => closeList());
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || opener?.contains(target))) return;
      closeList();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      sequence.stop();
      forget();
      window.removeEventListener("resize", placeMenu);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });
  // The list is only there with the scale fixed and off drums.
  $effect(() => {
    if (open && (!scale.lock || drums)) open = false;
  });
</script>

<div class="chords" class:unusable={drums} aria-disabled={drums || undefined} data-test="pr-chord-row">
  {#if drums}
    <span class="name">{t("pr.keyChords")}</span>
  {:else}
    <span class="m-lbl name"
      ><span use:term={"調の和音"}>{t("pr.keyChords")}</span>{#if scale.lock}{" "}<span class="m-num">{scaleName(scale.root, scale.mode)}</span>{/if}</span
    >
    {#if scale.lock}
      <div class="buttons">
        {#each chords as chord (chord.name)}
          {@const on = samePick(chordPick.pick, chord.pick)}
          <button
            class="m-btn m-num chord"
            class:on
            aria-pressed={on}
            onpointerdown={(e) => press(e, chord.pick)}
            onpointerup={release}
            onpointercancel={release}
            onclick={() => chooseChord(chord.pick)}
            data-test="pr-key-chord">{chord.name}</button
          >
        {/each}
      </div>
      <span class="m-spacer"></span>
      <button
        class="m-btn"
        class:on={open}
        aria-haspopup="listbox"
        aria-expanded={open}
        bind:this={opener}
        onclick={() => (open = !open)}
        data-test="pr-progression-open">{t("pr.progressionOpen")}</button
      >
    {:else}
      <span class="off" data-test="pr-key-chords-off">{t("pr.keyChordsOff")}</span>
    {/if}
  {/if}
</div>

{#if open}
  <div class="popup" use:toBody bind:this={popup} style:right="{place.right}px" style:top="{place.top}px">
    <ProgressionMenu {scale} {playing} onlisten={listen} onplace={placeChords} />
  </div>
{/if}

<style>
  .chords {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
    overflow: hidden;
  }
  .name {
    flex-shrink: 0;
  }
  .unusable .name {
    color: var(--color-gray-7);
  }
  .buttons {
    display: flex;
    gap: var(--space-1);
  }
  .chord {
    height: var(--part-toggle-size);
    min-width: var(--space-6);
    padding: 0 var(--space-1);
  }
  .chord.on,
  .chord.on:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .m-btn.on {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  /* Stacking: the same as the lists of the 「選んだノート」 panel. */
  .popup {
    position: fixed;
    z-index: 40;
  }
  .off {
    color: var(--color-gray-7);
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
