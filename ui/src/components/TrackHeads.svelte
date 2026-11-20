<!-- トラックの見出しの列（モック R7-Arrange6・R6-Tracks ・ F7-VocalTrack）と、いちばん下の「＋ 鍵盤のトラック」「＋ 歌のトラック」。アレンジの見出しの箱（trackMeters の区画）の中に置く。
     押すとトラックを選ぶ（B-70）。名前を 2 回押す ・ F2 ・ 右クリックの「名前を変える」で、名前の場所がその場の入力の枠（仕様書 2 章）。
     見出しを押したまま上下に動かすと並べかえ（仕様書 2 章「トラックを並べかえる印」）: 画面だけが先に動き、離したときに MoveTrack を 1 つ。
     space.1 より動かさなかったら、ふつうに押した（選ぶ）。Esc はキーの順のいちばん前でやめる（pianoroll/view.svelte.ts の cancelDrag）。
     音源の名前を押すと、その音源をえらぶ（パラメータの欄。19-A）。入れかえは右クリックの「音源を入れかえる…」と欄のボタン。
     入れかえのためし弾き中は、ためし弾きの音源の名前を accent で（モック F4-ShelfReplace）。
     歌のトラック（仕様書 2 章「歌のトラックの見出し」）: 2 段目は「入口 1 ・ モノ ▾」と「モニター」。● は選んでいなくても自分でつける。
     ● がついている間、レベルの棒は入ってくる音（計器の mics）。 -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { TrackView } from "../bridge";
  import { glSlot } from "../gl/slot";
  import { term, tip } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { findTerm, t } from "../text/ja";
  import { dbNumber } from "./format";
  import { openContextMenu } from "./menu/contextMenu.svelte";
  import { metrics } from "./metrics.svelte";
  import { onDragCancel } from "./pianoroll/view.svelte";
  import { trackContextMenu, vocalInputRows } from "./trackMenu";
  import { soundRefName, soundViewName, standInTip } from "./sounds";
  import { moveSlotAt, openShelf, trackEdit } from "./tracks.svelte";
  import { arrangeRows, showsAddRow } from "./automation/autoGeom";
  import LaneAddRow from "./automation/LaneAddRow.svelte";
  import LaneHead from "./automation/LaneHead.svelte";
  import { windowOf } from "./params/selected";
  import WindowMark from "./WindowMark.svelte";

  const song = $derived(current.state!.song);
  const shelf = $derived(current.state!.shelf);
  const transport = $derived(current.state!.transport);
  const busy = $derived(transport.state === "recording" || transport.state === "countIn");

  const shiftKey = (letter: string) => `${t("key.shift")} ${letter}`;

  /** 入れかえのためし弾き中のトラックの id と、ためし弾きの音源。 */
  const auditioned = $derived(
    shelf.mode.kind === "replace" && shelf.selectedSound !== null
      ? { track: shelf.mode.track, name: soundRefName(shelf.selectedSound, current.state!.library) }
      : null,
  );

  // ---- 名前の入力の枠 ----

  let field = $state<HTMLInputElement>();
  let draft = $state("");
  $effect(() => {
    const id = trackEdit.renaming;
    if (id === null) return;
    const track = song.tracks.find((x) => x.id === id);
    if (!track) {
      trackEdit.renaming = null;
      return;
    }
    draft = track.name;
    void tick().then(() => {
      field?.focus();
      field?.select();
    });
  });
  // 名前を打っている間に録音が始まったら、やめる（core は受けない）。
  $effect(() => {
    if (busy) trackEdit.renaming = null;
  });

  function commitName(track: TrackView): void {
    trackEdit.renaming = null;
    const name = draft.trim();
    // 空にしたら、もとの名前のまま（仕様書 3 章）。同じ名前なら手にしない。
    if (name !== "" && name !== track.name) send({ type: "renameTrack", track: track.id, name });
  }
  function fieldKey(e: KeyboardEvent, track: TrackView): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      commitName(track);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      trackEdit.renaming = null;
    }
  }

  // ---- 押す ・ 並べかえる ----

  let press: { id: number; from: number; y0: number; pointer: number; el: HTMLElement } | null = null;
  let drag = $state<{ id: number; from: number; dy: number } | null>(null);
  const trackH = $derived(metrics.px("size.track"));
  /** トラックの行と、その下のオートメーションの帯（時間の側と同じ並び。automation/autoGeom.ts）。 */
  const rows = $derived(
    arrangeRows(song.tracks, current.state!.automation.folded, { trackH, laneH: metrics.px("size.lane"), rowH: metrics.px("size.row") }),
  );
  const bounds = $derived([...rows.tops, rows.height]);
  const dropAt = (d: { from: number; dy: number }) => moveSlotAt(d.from, bounds, rows.tops[d.from]! + trackH / 2 + d.dy);
  const slot = $derived(drag ? dropAt(drag) : null);

  function headDown(e: PointerEvent, track: TrackView, at: number): void {
    if (e.button !== 0 || (e.target as Element | null)?.closest("button, input")) return;
    const el = e.currentTarget as HTMLElement;
    press = { id: track.id, from: at, y0: e.clientY, pointer: e.pointerId, el };
    el.setPointerCapture?.(e.pointerId);
  }
  function headMove(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointer) return;
    const dy = e.clientY - press.y0;
    if (!drag) {
      if (busy || Math.abs(dy) < metrics.px("space.1")) return;
      drag = { id: press.id, from: press.from, dy };
    } else drag = { ...drag, dy };
  }
  function headUp(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointer) return;
    const p = press;
    const d = drag;
    endPress();
    if (d) {
      const to = dropAt(d).to;
      if (to !== d.from) send({ type: "moveTrack", track: d.id, to });
    } else if (song.playTarget !== p.id) send({ type: "selectTrack", track: p.id, fromArm: false });
  }
  function endPress(): void {
    if (press?.el.hasPointerCapture?.(press.pointer)) press.el.releasePointerCapture(press.pointer);
    press = null;
    drag = null;
  }
  onMount(() =>
    onDragCancel(() => {
      if (!drag) return false;
      endPress();
      return true;
    }),
  );

  function headContext(e: MouseEvent, track: TrackView): void {
    e.preventDefault();
    if (drag) return;
    const select = song.playTarget !== track.id && !busy;
    if (select) send({ type: "selectTrack", track: track.id, fromArm: false });
    const armed = track.kind.kind === "keys" && song.keysTarget === track.id;
    openContextMenu(e.clientX, e.clientY, trackContextMenu(current.state!, track, armed || select, send, shiftKey));
  }

  function replaceInstrument(track: TrackView): void {
    if (!busy) send(openShelf({ kind: "replace", track: track.id }));
  }
  function instrumentContext(e: MouseEvent, track: TrackView): void {
    e.preventDefault();
    e.stopPropagation();
    openContextMenu(e.clientX, e.clientY, [{ kind: "row", label: "track.replaceSound", usable: !busy, run: () => replaceInstrument(track) }]);
  }
  function addTrack(): void {
    if (!busy) send(openShelf({ kind: "newTrack" }));
  }
  function addVocalTrack(): void {
    if (!busy) send({ type: "addVocalTrack" });
  }

  /** 鍵盤のトラックの ● は弾くと鳴る先（SongView.keysTarget）、歌のトラックの ● は自分でつけた物。 */
  const armOn = (track: TrackView) => (track.kind.kind === "vocal" ? track.kind.armed : song.keysTarget === track.id);
  function arm(track: TrackView): void {
    if (track.kind.kind === "vocal") send({ type: "toggleVocalArm", track: track.id });
    else send({ type: "selectTrack", track: track.id, fromArm: true });
  }

  /** 入口の一覧（モック F7-VocalTrack）: 入口 1 ・ モノ ／ 入口 2 ・ モノ ／ 入口 1 ・ 2 ・ ステレオ ／ 音の設定…。ボタンのすぐ下にひらく。 */
  function openInputs(e: MouseEvent, track: TrackView): void {
    e.stopPropagation();
    const kind = track.kind;
    if (kind.kind !== "vocal") return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openContextMenu(r.left, r.bottom, vocalInputRows(track.id, kind.input, busy, send));
  }
</script>

<div class="tracks">
  {#each song.tracks as track, at (track.id)}
    {@const lifted = drag?.id === track.id}
    <div
      class="track-head"
      class:selected={song.playTarget === track.id}
      class:lifted
      style:transform={lifted ? `translateY(${drag!.dy}px)` : null}
      role="presentation"
      data-test="track-head"
      onpointerdown={(e) => headDown(e, track, at)}
      onpointermove={headMove}
      onpointerup={headUp}
      onpointercancel={endPress}
      oncontextmenu={(e) => headContext(e, track)}
    >
      <div class="track-color" style:background="var(--color-track-{((track.color - 1) % 5) + 1})"></div>
      <div class="track-body">
        <div class="line">
          <span class="m-num m-lbl track-number">{track.number}</span>
          {#if trackEdit.renaming === track.id}
            <input
              class="name-field"
              type="text"
              aria-label={t("menu.renameTrack")}
              data-test="track-name-field"
              bind:this={field}
              bind:value={draft}
              onkeydown={(e) => fieldKey(e, track)}
              onblur={() => {
                // 枠の外を押したら決める（ヤチヨさんの決め 2026-11-18）。Esc のあとは renaming が null。
                if (trackEdit.renaming === track.id) commitName(track);
              }}
            />
          {:else}
            <span
              class="m-name track-name"
              role="presentation"
              use:term={"トラック"}
              ondblclick={() => !busy && (trackEdit.renaming = track.id)}>{track.name}</span
            >
          {/if}
          <!-- 録音中 ・ カウントイン中も使える（A-14）。 -->
          <button
            class="tog live"
            class:on={track.muted}
            aria-pressed={track.muted}
            aria-label={t("arrange.mute")}
            data-test="track-mute"
            use:term={"ミュート"}
            onclick={() => send({ type: "toggleMute", track: track.id })}>{t("arrange.muteShort")}</button
          >
          <button
            class="tog live"
            class:on={track.soloed}
            aria-pressed={track.soloed}
            aria-label={t("arrange.solo")}
            use:term={"ソロ"}
            onclick={() => send({ type: "toggleSolo", track: track.id })}>{t("arrange.soloShort")}</button
          >
          <button
            class="tog live arm"
            class:on={armOn(track)}
            aria-pressed={armOn(track)}
            aria-label={t("arrange.arm")}
            data-test="track-arm"
            use:term={"録音待機"}
            onclick={() => arm(track)}>{t("arrange.armShort")}</button
          >
        </div>
        <div class="line gap2">
          {#if track.kind.kind === "vocal"}
            {@const vocal = track.kind}
            <button
              class="m-plain m-lbl instrument"
              class:no-input={!vocal.inputReady}
              aria-haspopup="menu"
              data-test="track-input"
              onclick={(e) => openInputs(e, track)}
              >{vocal.inputReady ? t("vocal.inputChoose", { input: t(`vocal.input.${vocal.input}`) }) : t("vocal.noInput")}</button
            >
            <button
              class="monitor"
              class:on={vocal.monitor}
              aria-pressed={vocal.monitor}
              data-test="track-monitor"
              use:term={"モニター"}
              onclick={() => send({ type: "toggleMonitor", track: track.id })}>{t("vocal.monitor")}</button
            >
          {:else if auditioned?.track === track.id}
            <button class="m-plain instrument auditioned" data-test="track-instrument" onclick={() => replaceInstrument(track)}
              >{t("arrange.instrumentAuditioned", { instrument: auditioned.name })}</button
            >
          {:else if track.standIn}
            <!-- サイン波で代わりに鳴らしている（仕様書 3 章 85: gray.8。乗ると、わけ）。 -->
            <button class="m-plain m-lbl instrument" data-test="track-instrument" use:tip={standInTip(track.standIn, track.sound)} onclick={() => replaceInstrument(track)}
              oncontextmenu={(e) => instrumentContext(e, track)}
              >{t("track.standIn")}</button
            >
          {:else if track.sound.kind === "plugin"}
            <!-- プラグインの音源: 押すと、えらぶ ＋ 窓（SelectInstrument）。入れかえは右クリックの「音源を入れかえる…」（user:2026-11-19）。 -->
            <button
              class="m-plain m-lbl instrument plugin"
              data-test="track-instrument"
              onclick={() => send({ type: "selectInstrument", track: track.id })}
              oncontextmenu={(e) => instrumentContext(e, track)}
              ><span class="cut">{track.sound.name}</span>{#if windowOf(current.state!.windows, { kind: "instrument", track: track.id })}<WindowMark
                />{/if}</button
            >
          {:else}
            {@const name = soundViewName(track.sound)}
            <!-- 用語の説明がある音色（サイン波）だけ、乗せると説明が出る。 -->
            {#if findTerm(name)}
              <button
                class="m-plain m-lbl instrument"
                data-test="track-instrument"
                use:term={name}
                onclick={() => send({ type: "selectInstrument", track: track.id })}
                oncontextmenu={(e) => instrumentContext(e, track)}>{name}</button
              >
            {:else}
              <button
                class="m-plain m-lbl instrument"
                data-test="track-instrument"
                onclick={() => send({ type: "selectInstrument", track: track.id })}
                oncontextmenu={(e) => instrumentContext(e, track)}>{name}</button
              >
            {/if}
          {/if}
          {#if track.kind.kind === "keys"}
            <span class="m-num" use:term={"dB"}>{track.gainDb === null ? t("common.dbSilent") : t("common.db", { value: dbNumber(track.gainDb) })}</span>
          {/if}
        </div>
        <div
          class="meter"
          use:glSlot={{
            kind: "meter",
            source: track.kind.kind === "vocal" && track.kind.armed ? { mic: track.id } : { track: track.number },
            gap: metrics.px("line.hair"),
          }}
        ></div>
      </div>
    </div>
    {#each rows.lanes.filter((l) => l.track.id === track.id) as l (JSON.stringify(l.lane.target))}
      <LaneHead {track} lane={l.lane} />
    {/each}
    {#if showsAddRow(track)}<LaneAddRow {track} />{/if}
  {/each}
  {#if slot && drag && slot.to !== drag.from}
    <div class="insert" style:top="{bounds[slot.slot]}px" data-test="track-insert"></div>
  {/if}
</div>
<div class="adds">
  <button class="m-plain add keys" class:usable={!busy} aria-disabled={busy || undefined} data-test="add-track" onclick={addTrack}>
    {t("arrange.addKeysTrack")}
  </button>
  <button class="m-plain add" class:usable={!busy} aria-disabled={busy || undefined} data-test="add-vocal-track" onclick={addVocalTrack}>
    {t("arrange.addVocalTrack")}
  </button>
</div>

<style>
  .tracks {
    position: relative;
  }
  .track-head {
    position: relative;
    height: var(--size-track);
    display: flex;
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    background: var(--color-gray-3);
  }
  /* 選んだトラック（B-70）: 一覧の「選んだ行」と同じ地。 */
  .track-head.selected {
    background: var(--color-gray-4);
  }
  /* つまみ上げた見出し（仕様書 2 章「トラックを並べかえる印」）: accent の line.hair の枠。ほかの見出しより上に描く。 */
  .track-head.lifted {
    z-index: 2;
    outline: var(--line-hair) solid var(--color-accent);
    outline-offset: calc(-1 * var(--line-hair));
  }
  /* 入る所: accent の line.mark の横線。見出しの境に重ねる。 */
  .insert {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--line-mark);
    margin-top: calc(-1 * var(--line-mark) / 2);
    background: var(--color-accent);
    pointer-events: none;
    z-index: 3;
  }
  .track-color {
    width: var(--line-track);
    flex-shrink: 0;
  }
  .track-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-2) 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .line.gap2 {
    gap: var(--space-2);
  }
  .track-number {
    width: var(--mock-track-number);
    flex-shrink: 0;
  }
  .track-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* トラックの名前の入力の枠（仕様書 2 章: 地 gray.0、枠 gray.10、太さ 600）。高さはモック R6-Tracks の 20（M ・ S ・ ● と同じ）。 */
  .name-field {
    flex: 1;
    min-width: 0;
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    font-weight: var(--text-s-weight-600);
    outline: none;
  }
  .instrument {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .instrument:hover {
    color: var(--color-gray-10);
  }
  .instrument.plugin {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .cut {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .instrument.auditioned,
  .instrument.auditioned:hover {
    color: var(--color-accent);
  }
  .tog {
    width: var(--part-toggle-size);
    height: var(--part-toggle-size);
    flex-shrink: 0;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-5);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-7);
    font: inherit;
    text-align: center;
    cursor: default;
  }
  .tog.live {
    border-color: var(--color-gray-6);
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .tog.live:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  /* オン（仕様書 2 章: 枠と文字 accent。● だけ rec）。 */
  .tog.live.on {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .tog.arm.on {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  /* 押した（仕様書 2 章: bg gray.2 ・ 枠 gray.8）。 */
  .tog.live:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .meter {
    height: calc(var(--part-meter-horizontal) * 2 + var(--line-hair));
  }
  /* 歌のトラックの「入口なし ▾」（仕様書 3 章: gray.8）。 */
  .instrument.no-input {
    color: var(--color-gray-8);
  }
  /* 「モニター」（モック F7-VocalTrack: 高さ 20。オンは「えらんでいる」の形 = 枠と文字 accent）。 */
  .monitor {
    box-sizing: border-box;
    flex-shrink: 0;
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    font: inherit;
    cursor: pointer;
  }
  .monitor:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .monitor:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .monitor.on {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  /* 「＋ 鍵盤のトラック」「＋ 歌のトラック」（モック F7-VocalTrack: 文字 gray.9、左の箱は幅 120）。録音中は「使えない」。 */
  .adds {
    display: flex;
  }
  .add.keys {
    width: var(--mock-arrange-add-keys);
  }
  .add {
    flex-shrink: 0;
    white-space: nowrap;
    height: var(--mock-arrange-add-track);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    color: var(--color-gray-7);
    cursor: default;
  }
  .add.usable {
    color: var(--color-gray-9);
    cursor: pointer;
  }
  .add.usable:hover {
    color: var(--color-gray-10);
  }
</style>
