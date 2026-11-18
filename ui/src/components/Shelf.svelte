<!-- 棚（右の欄。モック F4-ShelfSample・F4-ShelfAudition・F4-ShelfReplace。幅 size.shelf）。
     見出し（▾ 棚、音源 N ・ エフェクト 6 の切りかえ、P）、いまのモードの文、さがす枠、すべて ・ お気に入り、グループと音源の行。
     さがす文字、お気に入りだけ、たたんだグループは画面だけが持つ（tracks.svelte.ts）。行をえらぶ ・ 入れる ・ やめる ・ 星 ・ サンプルは core への意図。
     第 4 回の棚は、プラグインの見出しもスキャンも形式の印も出さない（仕様書 3 章 Y10）。エフェクトの中身は区切り3 なので「使えない」。 -->
<script lang="ts">
  import { untrack } from "svelte";
  import { clockOffsetUs, type InstrumentKind } from "../bridge";
  import ScrollArea from "../lib/ScrollArea.svelte";
  import { term } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { findTerm, t } from "../text/ja";
  import Button from "./Button.svelte";
  import { textParts } from "./format";
  import { BUILTIN_EFFECTS, shelfCounts, shelfGroups, shelfUi } from "./tracks.svelte";

  const app = $derived(current.state!);
  const shelf = $derived(app.shelf);
  const groups = $derived(shelfGroups(shelf, shelfUi));
  const counts = $derived(shelfCounts(shelf));
  const busy = $derived(app.transport.state === "recording" || app.transport.state === "countIn");

  const replaceTrack = $derived(shelf.mode.kind === "replace" ? app.song.tracks.find((x) => x.id === (shelf.mode as { track: number }).track) : null);
  const title = $derived(
    shelf.mode.kind === "replace" && replaceTrack
      ? t("shelf.titleReplace", { track: t("track.label", { number: replaceTrack.number, name: replaceTrack.name }) })
      : t("shelf.titleNewTrack"),
  );

  const nameOf = (i: InstrumentKind) => t(`instrument.${i}`);

  // ひらいたとき、モード（新しいトラック ・ 入れかえ）が変わったときは、棚にキーを向ける。
  const modeKey = $derived(shelf.mode.kind === "replace" ? `replace:${shelf.mode.track}` : "newTrack");
  $effect(() => {
    void modeKey;
    untrack(() => (shelfUi.keys = true));
  });

  function pick(i: InstrumentKind): void {
    shelfUi.keys = true;
    if (shelf.selected !== i) send({ type: "shelfSelect", instrument: i });
  }
  function toggleGroup(group: (typeof groups)[number]["group"]): void {
    shelfUi.folded = shelfUi.folded.includes(group) ? shelfUi.folded.filter((g) => g !== group) : [...shelfUi.folded, group];
  }
  function sample(i: InstrumentKind): void {
    send(shelf.sample?.instrument === i ? { type: "stopSample" } : { type: "playSample", instrument: i });
  }

  // ---- サンプルの「0:02 / 0:04」。Rust の時計（started_us）から、画面の時計で数える（1 秒ごとの知らせはない） ----

  let offsetUs: number | null = null;
  let nowUs = $state(0);
  $effect(() => {
    if (!shelf.sample) return;
    const tickNow = () => {
      if (offsetUs !== null) nowUs = performance.now() * 1000 + offsetUs;
    };
    if (offsetUs === null)
      clockOffsetUs().then(
        (v) => {
          offsetUs = v;
          tickNow();
        },
        () => {},
      );
    tickNow();
    const timer = setInterval(tickNow, 250);
    return () => clearInterval(timer);
  });
  const clock = (us: number) => {
    const s = Math.max(0, Math.floor(us / 1e6));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  const sampleTime = $derived.by(() => {
    const s = shelf.sample;
    if (!s) return null;
    const at = offsetUs === null ? 0 : Math.min(s.lengthUs, Math.max(0, nowUs - s.startedUs));
    return t("shelf.sampleTime", { at: clock(at), length: clock(Math.round(s.lengthUs / 1e6) * 1e6) });
  });
</script>

<!-- 星（part.shelfrow.star）（仕様書 2 章「音源の行（棚）」）と、サンプルのボタンの ▶ ・ ■。 -->
{#snippet star(on: boolean)}
  <svg class="star" class:on viewBox="0 0 12 12" aria-hidden="true"
    ><path d="M6 .5l1.7 3.6 3.9.5-2.9 2.7.7 3.9L6 9.3 2.6 11.2l.7-3.9L.4 4.6l3.9-.5z" /></svg
  >
{/snippet}
{#snippet play()}<svg class="icon" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l8 5-8 5z" /></svg>{/snippet}
{#snippet stop()}<svg class="icon" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1h8v8h-8z" /></svg>{/snippet}

<aside class="panel" data-band="shelf" data-shelf data-test="shelf">
  <div class="head">
    <button class="m-plain fold m-focus-in" onclick={() => send({ type: "toggleBand", band: "shelf" })} data-test="rail-fold">
      <span class="m-hi">▾</span>
      <span class="m-name">{t("band.shelf")}</span>
    </button>
    <span class="m-spacer"></span>
    <span class="switches">
      <Button on>{t("shelf.instrumentCount", { count: counts.all })}</Button>
      <Button disabled>{t("shelf.effectCount", { count: BUILTIN_EFFECTS })}</Button>
    </span>
    <span class="m-num m-lbl">{t("key.shelf")}</span>
  </div>
  <div class="mode" data-test="shelf-mode">{title}</div>
  <label class="bar">
    <span class="m-lbl">{t("shelf.search")}</span>
    <input class="search" type="text" placeholder={t("shelf.searchLabel")} bind:value={shelfUi.query} data-test="shelf-search" />
  </label>
  <div class="bar">
    <Button on={!shelfUi.favoritesOnly} onclick={() => (shelfUi.favoritesOnly = false)} test="shelf-all"
      >{t("shelf.all")} <span class="m-num">{counts.all}</span></Button
    >
    <Button on={shelfUi.favoritesOnly} onclick={() => (shelfUi.favoritesOnly = true)} test="shelf-favorites"
      >{t("shelf.favorites")} <span class="m-num">{counts.favorites}</span></Button
    >
    <span class="m-spacer"></span>
    <span class="m-lbl keys"
      >{#each textParts("shelf.keys", { sampleKey: t("key.playStop"), favoriteKey: t("key.favorite") }) as part, i (i)}{#if part.hole !== null}<span
            class="m-num">{part.text}</span
          >{:else}{part.text}{/if}{/each}</span
    >
  </div>
  <ScrollArea data-test="shelf-list">
    {#each groups as g (g.group)}
      <button class="m-plain group m-focus-in" onclick={() => toggleGroup(g.group)} data-group={g.group}>
        <span class="m-hi">{g.folded ? "▸" : "▾"}</span>
        <span class="m-hi group-name">{t(`instrumentGroup.${g.group}`)}</span>
        <span class="m-num m-lbl">{g.rows.length}</span>
      </button>
      {#if !g.folded}
        {#each g.rows as i (i)}
          {@const selected = shelf.selected === i}
          {@const fav = shelf.favorites.includes(i)}
          {@const playing = shelf.sample?.instrument === i}
          <div class="row" class:selected role="presentation" data-row={i} onclick={() => pick(i)}>
            <button
              class="m-plain star-hit m-focus-in"
              aria-label={t("shelf.favorite")}
              aria-pressed={fav}
              onclick={(e) => {
                e.stopPropagation();
                send({ type: "toggleFavorite", instrument: i });
              }}>{@render star(fav)}</button
            >
            <button
              class="sample"
              class:playing
              aria-label={t("shelf.playSample")}
              use:term={"サンプル"}
              data-test="shelf-sample"
              onclick={(e) => {
                e.stopPropagation();
                sample(i);
              }}>{#if playing}{@render stop()}{:else}{@render play()}{/if}</button
            >
            {#if findTerm(nameOf(i))}
              <span class="name" use:term={nameOf(i)}>{nameOf(i)}</span>
            {:else}
              <span class="name">{nameOf(i)}</span>
            {/if}
            {#if playing && sampleTime}
              <span class="m-num time" data-test="shelf-sample-time">{sampleTime}</span>
            {:else if shelf.mode.kind === "replace" && shelf.current === i}
              <span class="m-lbl">{t("shelf.current")}</span>
            {/if}
          </div>
          {#if selected}
            <div class="audition" data-test="shelf-audition">
              <span class="accent">{t("shelf.auditioning")}</span>
              {#if shelf.loading === i}
                <span class="say">{t("msg.auditionLoading", { name: nameOf(i) })}</span>
              {:else}
                <span class="say">{t(shelf.mode.kind === "replace" ? "msg.auditionReplace" : "msg.audition")}</span>
              {/if}
              <div class="buttons">
                {#if shelf.loading !== i}
                  <Button primary onclick={() => send({ type: "shelfConfirm" })} disabled={busy} test="shelf-confirm">
                    {shelf.mode.kind === "replace"
                      ? t("button.replaceInstrument")
                      : t("button.makeTrack", { number: app.song.tracks.length + 1 })}
                    <span class="m-num m-lbl">{t("key.enter")}</span>
                  </Button>
                {/if}
                <Button onclick={() => send({ type: "shelfCancel" })} test="shelf-cancel"
                  >{t("button.cancel")} <span class="m-num m-lbl">{t("key.esc")}</span></Button
                >
              </div>
            </div>
          {/if}
        {/each}
      {/if}
    {/each}
  </ScrollArea>
</aside>

<style>
  .panel {
    width: var(--size-shelf);
    flex-shrink: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border-left: var(--line-hair) solid var(--color-gray-6);
  }
  .head,
  .group {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    white-space: nowrap;
  }
  .fold {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: 100%;
  }
  .switches {
    display: flex;
    gap: var(--space-1);
  }
  /* いまのモードの文（モック F4-ShelfSample: 地 gray.1、下に accent の線）。 */
  .mode {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
    background: var(--color-gray-1);
    border-bottom: var(--line-hair) solid var(--color-accent);
    color: var(--color-accent);
    white-space: nowrap;
    overflow: hidden;
  }
  .bar {
    height: var(--mock-shelf-bar);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .keys {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .search {
    flex: 1;
    min-width: 0;
    height: var(--part-input-height);
    padding: 0 var(--part-input-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    color: var(--color-gray-9);
    font: inherit;
    outline: none;
  }
  .search::placeholder {
    color: var(--color-gray-7);
  }
  .search:focus {
    border-color: var(--color-gray-8);
  }
  .group {
    width: 100%;
  }
  .group-name {
    flex: 1;
    min-width: 0;
  }
  .group:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* 音源の行（仕様書 2 章: 高さ 24。星、サンプルのボタン、名前）。 */
  .row {
    height: var(--part-shelfrow-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3) 0 var(--space-2);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    white-space: nowrap;
    cursor: pointer;
  }
  .row:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* ためし弾き中: 地 gray.4、左に accent の line.mark。 */
  .row.selected {
    background: var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-accent);
    padding-left: calc(var(--space-2) - var(--line-mark));
  }
  .star-hit {
    width: var(--mock-shelf-star-hit);
    height: var(--mock-shelf-star-hit);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .star {
    width: var(--part-shelfrow-star);
    height: var(--part-shelfrow-star);
    fill: none;
    stroke: var(--color-gray-7);
    stroke-width: 1;
  }
  .star.on {
    fill: var(--color-gray-10);
    stroke: var(--color-gray-10);
  }
  .sample {
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
  .sample:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .sample.playing {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .icon {
    width: var(--part-button-icon);
    height: var(--part-button-icon);
    fill: currentColor;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-10);
  }
  .time {
    color: var(--color-accent);
  }
  /* えらんだ行の下にひらく案内とボタン（モック F4-ShelfAudition）。 */
  .audition {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    border-left: var(--line-mark) solid var(--color-accent);
  }
  .accent {
    color: var(--color-accent);
  }
  .say {
    white-space: normal;
  }
  .buttons {
    display: flex;
    gap: var(--space-2);
  }
</style>
