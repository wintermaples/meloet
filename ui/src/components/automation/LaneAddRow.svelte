<!-- トラックの帯の下の「＋ 帯を足す ▾」の行（仕様書 3 章 92-5 ・ 20-A、モック Opt7-AddLane ・ F7-Answers92）。
     押すと一覧（音量 ・ パン ・ ミュート ・ センド A ・ センド B ／ エフェクトのパラメータ…）。足してある帯には選ぶ印。
     行を押すと AddLane（足してある帯がたたんであれば、core がひらく）。「エフェクトのパラメータ…」は右のパラメータの欄の
     一覧をひらく（そこの行の右の印で帯を足す）。右はしの数は、たたんでいるパラメータの帯の数（モック F6-ParamMany の 2）。 -->
<script lang="ts">
  import type { AutoTarget, Command, LaneName, TrackView } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { laneNameText } from "../history/stepText";
  import { openContextMenu, type MenuEntry } from "../menu/contextMenu.svelte";
  import { hasLaneRef, laneOf } from "./laneValue";

  let { track }: { track: TrackView } = $props();

  const folded = $derived(current.state!.automation.folded);
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  const foldedParams = $derived(
    track.lanes.filter((l) => (l.target.kind === "insert" || l.target.kind === "instrument") && hasLaneRef(folded, track.id, l.target)).length,
  );

  /** パラメータの欄にひらく物: プラグインの音源、なければいちばん上のエフェクト。 */
  const openParams = $derived.by((): Command | null => {
    if (track.sound.kind === "plugin") return { type: "selectInstrument", track: track.id };
    const slot = track.slots.findIndex((s) => s !== null);
    return slot < 0 ? null : { type: "selectInsert", insert: { strip: { kind: "track", id: track.id }, slot } };
  });

  const TARGETS: readonly { target: AutoTarget; name: LaneName }[] = [
    { target: { kind: "volume" }, name: { kind: "volume" } },
    { target: { kind: "pan" }, name: { kind: "pan" } },
    { target: { kind: "mute" }, name: { kind: "mute" } },
    { target: { kind: "send", bus: "a" }, name: { kind: "send", bus: "a" } },
    { target: { kind: "send", bus: "b" }, name: { kind: "send", bus: "b" } },
  ];

  function entries(): MenuEntry[] {
    const id = track.id;
    const rows: MenuEntry[] = TARGETS.map(({ target, name }) => ({
      kind: "row",
      label: "lane.volume",
      text: laneNameText(name),
      usable: !busy,
      checked: laneOf(track, target) !== undefined,
      run: () => send({ type: "addLane", track: id, target }),
    }));
    const params = openParams;
    rows.push({ kind: "separator" }, { kind: "row", label: "lane.addEffectParams", usable: params !== null, run: () => params && send(params) });
    return rows;
  }

  function open(ev: MouseEvent): void {
    if (busy) return;
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    openContextMenu(r.left, r.bottom, entries());
  }
</script>

<div class="add-row" data-test="lane-add-row">
  <button class="m-plain m-lbl item" class:usable={!busy} aria-disabled={busy || undefined} aria-haspopup="menu" data-test="lane-add" onclick={open}
    >{t("lane.add")}</button
  >
  <span class="m-spacer"></span>
  {#if foldedParams > 0}<span class="m-num m-lbl count">{foldedParams}</span>{/if}
</div>

<style>
  .add-row {
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-2) 0 var(--space-6);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .item {
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-7);
    cursor: default;
  }
  .count {
    flex-shrink: 0;
  }
  .item.usable {
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .item.usable:hover {
    color: var(--color-gray-10);
  }
</style>
