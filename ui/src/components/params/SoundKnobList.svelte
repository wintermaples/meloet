<!-- The four rows of a sound that comes with meloet (round 12 E3): 「明るさ」「立ち上がり」「のび」「ゆれ」, rows of the same form as a
     built-in effect's (ParamRow). A knob the sound does not use (a drum kit's 「立ち上がり」 and 「ゆれ」) is shown disabled.
     ↑ ↓ choose a row among the usable ones; Enter gives the typing box. The mark on the right adds the knob's lane. -->
<script lang="ts">
  import type { AutoTarget, SelectedSound, SoundKnobsView } from "../../bridge";
  import { SOUND_KNOBS } from "../../bridge/types";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { livePosition } from "../automation/autoState.svelte";
  import { laneMoves, laneOf } from "../automation/laneValue";
  import ParamRow from "./ParamRow.svelte";
  import { paramUi } from "./paramUi.svelte";
  import { arrowToChosenRow } from "./rowKeys";

  let { sel, knobs }: { sel: SelectedSound; knobs: SoundKnobsView } = $props();

  const instrument = $derived(sel.sound.kind === "builtin" ? sel.sound.instrument : null);
  const track = $derived(current.state!.song.tracks.find((tr) => tr.id === sel.track) ?? null);
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  const targetOf = (param: number): AutoTarget => ({ kind: "instrument", param });

  function markOf(param: number) {
    const tr = track;
    const target = targetOf(param);
    return {
      on: tr !== null && laneOf(tr, target) !== undefined,
      usable: tr !== null && !busy && knobs.usable[param] === true,
      add: () => tr && send({ type: "addLane", home: { kind: "track", track: tr.id }, target }),
    };
  }

  /** The value a band moves (the place on the line, in percent). */
  function autoValue(param: number): number | null {
    const tr = track;
    const target = targetOf(param);
    if (!tr || !laneMoves(laneOf(tr, target), current.state!.automation.held, tr.id)) return null;
    const pos = livePosition(tr.id, target);
    return pos === null ? null : pos * 100;
  }

  let list = $state<HTMLElement>();
  const usableRows = $derived(SOUND_KNOBS.map((_, i) => i).filter((i) => knobs.usable[i]));

  function pick(param: number): void {
    if (list && !list.contains(document.activeElement)) list.focus({ preventScroll: true });
    if (knobs.row !== param) send({ type: "selectParam", param });
  }

  // Spec 4 "In the parameters: choose the row above / below" (↑ ↓), "move the value" (← →) and "type a value" (Enter), as in BuiltinList.
  function onkeydown(ev: KeyboardEvent): void {
    if (arrowToChosenRow(ev, list, knobs.row)) return;
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.shiftKey || ev.isComposing) return;
    if (ev.key === "Enter") {
      ev.preventDefault();
      ev.stopPropagation();
      if (knobs.row !== null && knobs.usable[knobs.row]) paramUi.typing = knobs.row;
      return;
    }
    if (ev.key !== "ArrowUp" && ev.key !== "ArrowDown") return;
    ev.preventDefault();
    ev.stopPropagation();
    const rows = usableRows;
    if (rows.length === 0) return;
    const at = knobs.row === null ? -1 : rows.indexOf(knobs.row);
    const next = at < 0 ? (ev.key === "ArrowDown" ? rows[0] : rows.at(-1)) : rows[Math.max(0, Math.min(rows.length - 1, at + (ev.key === "ArrowDown" ? 1 : -1)))];
    if (next !== undefined) pick(next);
  }
</script>

<div class="list" role="presentation" tabindex="-1" bind:this={list} {onkeydown} data-test="sound-knobs">
    {#each instrument === null ? [] : SOUND_KNOBS as k, i (k)}
      <ParamRow
        knob={{ track: sel.track, instrument: instrument!, knob: k }}
        disabled={!knobs.usable[i]}
        param={i}
        name={t(`knob.${k}`)}
        value={knobs.values[i] ?? 0}
        selected={knobs.row === i}
        onselect={() => pick(i)}
        mark={markOf(i)}
        auto={autoValue(i)}
      />
    {/each}
</div>

<style>
  .list {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    outline: none;
  }
</style>
