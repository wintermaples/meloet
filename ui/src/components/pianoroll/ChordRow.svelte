<!-- The row of the chords of the key, under the tool row (mock P10-ChordTable, option A). 24 high.
     With the scale fixed, a button for each chord of the key: pressing one sounds it while held and makes it the chord of
     the draw tool. Otherwise the row says how to get them. Disabled on drums. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { term } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { scaleName } from "../format";
  import { Auditioner, fixedOr100 } from "./audition";
  import { chooseChord, chordPick } from "./chordPick.svelte";
  import { auditionKeys, keyChords, samePick, triadName, type ChordPick } from "./chords";
  import { rollShowsDrums } from "./drums";
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
    {:else}
      <span class="off" data-test="pr-key-chords-off">{t("pr.keyChordsOff")}</span>
    {/if}
  {/if}
</div>

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
  .off {
    color: var(--color-gray-7);
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
