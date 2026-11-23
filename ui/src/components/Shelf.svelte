<!-- The shelf (the right panel; mock F4-ShelfSample, F4-ShelfAudition, F4-ShelfReplace, F5-SF2Shelf, F5-ShelfFx, F5-ShelfSearch, F5-PluginLoading, F5-PluginAudition, R8-Missing; width size.shelf).
     Heading (▾ shelf, the instruments N / effects N toggle, P), the text of the current mode, the search box, all / favorites, the list (shelfList.ts).
     The list goes built-in → added later (SF2 files → plugin kinds). Even with hundreds of rows, only the visible rows are made.
     The query, favorites only and folded headings are kept only by the screen (tracks.svelte.ts). Choosing a row, put in, cancel, star and sample are intents to core.
     In a shelf opened from 「空き ＋」 (insertEffect), press an effect row or choose with ↑ ↓, then put it in with Enter or 「入れる」. Effects in the instrument mode are view only (docs/history/design-notes.ja.md A-14).
     A shelf opened from 「入れかえる…」 in the context menu (replaceEffect; spec 3 (86-3), mock R9-Inserts) has the same form: the current row shows 「いまのエフェクト」, and the box has the replace text and 「入れかえる」.
     A box under the chosen row (audition, loading, cannot load, put in the insert). When the row of the effect to put in is not visible, at the bottom of the list.
     While scanning, the scan strip sits right under the 「あとから入れたもの」 heading (Y13). Stopped plugins get a box with a red border under the shelf (spec 6 "a plugin stopped"). -->
<script lang="ts">
  import { untrack } from "svelte";
  import { clockOffsetUs, type EffectRef, type MissingView, type PluginRowView, type PluginSlotRef, type SoundRef } from "../bridge";
  import ScrollArea from "../lib/ScrollArea.svelte";
  import { term, tip } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { findTerm, t } from "../text/ja";
  import Button from "./Button.svelte";
  import { textParts } from "./format";
  import { openContextMenu, type MenuEntry } from "./menu/contextMenu.svelte";
  import { metrics } from "./metrics.svelte";
  import { stripName } from "./mixer/stripName";
  import ScanStrip from "./plugins/ScanStrip.svelte";
  import { favoriteCount, rowIs, shelfCounts, shelfModel, withBox, type ShelfEntry, type ShelfRow } from "./shelfList";
  import { effectRefName, formatName, formatsTip, rowHasKey, sameSound, soundRefName } from "./sounds";
  import { pickCommand, pickedRow, sampleCommand, shelfPicks, shelfUi, toggleHeading } from "./tracks.svelte";

  const app = $derived(current.state!);
  const shelf = $derived(app.shelf);
  const library = $derived(app.library);
  const scan = $derived(app.plugins.scan);
  const busy = $derived(app.transport.state === "recording" || app.transport.state === "countIn");

  const replaceTrack = $derived(shelf.mode.kind === "replace" ? app.song.tracks.find((x) => x.id === (shelf.mode as { track: number }).track) : null);
  const title = $derived(
    shelf.mode.kind === "insertEffect"
      ? t("shelf.titleInsert", { strip: stripName(app.song, shelf.mode.strip, library) })
      : shelf.mode.kind === "replaceEffect"
        ? t("shelf.titleReplaceEffect", { strip: stripName(app.song, shelf.mode.strip, library), slot: shelf.mode.slot + 1 })
        : shelf.mode.kind === "replace" && replaceTrack
        ? t("shelf.titleReplace", { track: t("track.label", { number: replaceTrack.number, name: replaceTrack.name }) })
        : t("shelf.titleNewTrack"),
  );
  const inserting = $derived(shelf.mode.kind === "insertEffect" || shelf.mode.kind === "replaceEffect");
  const effectsView = $derived(shelfUi.view === "effects");
  const counts = $derived(shelfCounts(shelf, library));
  const favorites = $derived(favoriteCount(shelfUi.view, shelf, library));
  const fetchingNow = $derived(app.sounds.fetch.state === "fetching" ? app.sounds.fetch.instrument : null);
  const model = $derived(shelfModel(shelfUi.view, shelf, library, shelfUi, fetchingNow));
  const picks = $derived(shelfPicks(app));
  const picked = $derived(pickedRow(app, model.rows));
  /** Whether to show the box: there is a chosen instrument (audition, loading, could not load). In a shelf for putting in an effect, always (「入れる」 is disabled until something is chosen). */
  const boxShown = $derived(picks && (effectsView || (shelf.selectedSound ?? shelf.loadingSound ?? shelf.unreadable) !== null));

  type Entry = ShelfEntry | { kind: "scan"; id: "scan" };
  const entries = $derived.by((): Entry[] => {
    const list: Entry[] = boxShown ? withBox(model.entries, picked, inserting) : [...model.entries];
    if (scan.state === "running") {
      const at = list.findIndex((e) => e.id === "section:added");
      list.splice(at + 1, 0, { kind: "scan", id: "scan" });
    }
    return list;
  });

  // When it opens, or when the mode (new track, replace, put in effect) changes, send keys to the shelf and show the list of that mode.
  const modeKey = $derived(JSON.stringify(shelf.mode));
  $effect(() => {
    void modeKey;
    untrack(() => {
      shelfUi.keys = true;
      shelfUi.view = shelf.mode.kind === "insertEffect" || shelf.mode.kind === "replaceEffect" ? "effects" : "instruments";
    });
  });

  function pick(row: ShelfRow): void {
    shelfUi.keys = true;
    const c = pickCommand(app, row);
    if (c) send(c);
  }
  const isFavorite = (row: ShelfRow): boolean => {
    if (row.plugin) {
      const list: (SoundRef | EffectRef)[] = row.sound ? shelf.favoriteSounds : shelf.favoriteEffects;
      return list.some((f) => f.kind === "plugin" && rowHasKey(row.plugin!, f.key));
    }
    if (row.sound) return shelf.favoriteSounds.some((f) => sameSound(f, row.sound));
    return shelf.favoriteEffects.some((f) => rowIs(row, null, f));
  };
  function toggleFavorite(row: ShelfRow): void {
    if (row.sound) send({ type: "toggleFavoriteSound", sound: row.sound });
    else if (row.effect) send({ type: "toggleFavoriteEffect", effect: row.effect });
  }
  /** 「＋1」: the list to choose the format to use (spec 2 "Plugin row in the shelf"). */
  function chooseFormat(e: MouseEvent, row: PluginRowView): void {
    e.stopPropagation();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const entries: MenuEntry[] = row.formats.map((f) => ({
      kind: "row",
      label: `plugin.format.${f.format}`,
      checked: f.key === row.key,
      usable: true,
      run: () => {
        if (f.key !== row.key) send({ type: "usePluginFormat", key: f.key });
      },
    }));
    openContextMenu(r.left, r.bottom, entries);
  }

  // ---- Make only the visible rows (rows and headings are 24 high; box, notice and scan strip use measured heights) ----

  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let viewH = $state(0);
  const measured: Record<string, number> = $state({});
  const rowPx = $derived(metrics.px("part.shelfrow.height"));
  const fixed = (e: Entry) => e.kind === "row" || e.kind === "heading" || e.kind === "section";
  const offsets = $derived.by(() => {
    const out = [0];
    for (const e of entries) out.push(out.at(-1)! + (fixed(e) ? rowPx : (measured[e.id] ?? rowPx * 3)));
    return out;
  });
  /** Rows added before and after the visible part (so fast scrolling shows no gaps). */
  const OVERSCAN = 8;
  const range = $derived.by(() => {
    // Before sizes are measured (before layout), all of them. Rows in a folded heading are never made.
    if (viewH <= 0) return { from: 0, to: entries.length };
    const top = scrollTop;
    const bottom = scrollTop + viewH;
    let first = 0;
    while (first < entries.length && offsets[first + 1]! <= top) first++;
    let last = first;
    while (last < entries.length && offsets[last]! < bottom) last++;
    return { from: Math.max(0, first - OVERSCAN), to: Math.min(entries.length, last + OVERSCAN) };
  });
  const visible = $derived(entries.slice(range.from, range.to));

  $effect(() => {
    const v = viewport;
    if (!v) return;
    const read = () => {
      scrollTop = v.scrollTop;
      viewH = v.clientHeight;
    };
    read();
    v.addEventListener("scroll", read, { passive: true });
    const observer = new ResizeObserver(read);
    observer.observe(v);
    return () => {
      v.removeEventListener("scroll", read);
      observer.disconnect();
    };
  });

  // Keep the row chosen with ↑ ↓ (and the box under it) visible. Only when the chosen thing changes (not when a heading is just opened).
  const pickKey = $derived(JSON.stringify(effectsView ? shelf.chosenEffect : (shelf.selectedSound ?? shelf.loadingSound)));
  let shownPick: string | null = null;
  $effect(() => {
    const key = pickKey;
    if (shownPick === null || key === shownPick) {
      shownPick = key;
      return;
    }
    shownPick = key;
    const id = untrack(() => picked?.id ?? null);
    untrack(() => {
      const v = viewport;
      const at = id === null ? -1 : entries.findIndex((e) => e.id === id);
      if (!v || at < 0) return;
      const top = offsets[at]!;
      const bottom = offsets[entries[at + 1]?.kind === "box" ? at + 2 : at + 1]!;
      if (top < v.scrollTop) v.scrollTop = top;
      else if (bottom > v.scrollTop + v.clientHeight) v.scrollTop = Math.max(top, bottom - v.clientHeight);
    });
  });

  // ---- The 「0:02 / 0:04」 of a sample. Counted with the screen clock from the Rust clock (started_us); there is no notice every second ----

  let offsetUs: number | null = null;
  let nowUs = $state(0);
  const playing = $derived(shelf.soundSample?.playing ?? null);
  $effect(() => {
    if (!playing) return;
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
    if (!shelf.soundSample) return null;
    const p = shelf.soundSample.playing;
    if (!p) return t("shelf.sampleLoading");
    const at = offsetUs === null ? 0 : Math.min(p.lengthUs, Math.max(0, nowUs - p.startedUs));
    return t("shelf.sampleTime", { at: clock(at), length: clock(Math.round(p.lengthUs / 1e6) * 1e6) });
  });

  // ---- Box ----

  const pickedName = $derived(picked?.sound ? soundRefName(picked.sound, library) : (picked?.name ?? ""));
  /** The mix of reverb and delay put on a track or the master starts at 30 % (spec 3 (86); the text is from mock F5-ShelfFx). */
  const mixStart = $derived.by(() => {
    if (shelf.mode.kind !== "insertEffect" || shelf.mode.strip.kind === "return" || !shelf.chosenEffect) return false;
    const e = shelf.chosenEffect;
    if (e.kind === "builtin") return e.effect === "reverb" || e.effect === "delay";
    const row = library.plugins.find((r) => rowHasKey(r, e.key));
    return row?.kind.role === "effect" && (row.kind.category === "reverb" || row.kind.category === "delay");
  });

  // ---- Stopped plugins (under the shelf) ----

  const stopped = $derived.by(() => {
    const out: { name: string; where: string; slot: PluginSlotRef }[] = [];
    const song = app.song;
    for (const tr of song.tracks) {
      if (tr.sound.kind === "plugin" && tr.sound.state === "stopped")
        out.push({ name: tr.sound.name, where: stripName(song, { kind: "track", id: tr.id }, library), slot: { kind: "instrument", track: tr.id } });
    }
    const strips = [
      ...song.tracks.map((tr) => ({ strip: { kind: "track" as const, id: tr.id }, slots: tr.slots })),
      { strip: { kind: "return" as const, bus: "a" as const }, slots: song.returns[0].slots },
      { strip: { kind: "return" as const, bus: "b" as const }, slots: song.returns[1].slots },
      { strip: { kind: "master" as const }, slots: song.master.slots },
    ];
    for (const { strip, slots } of strips)
      slots.forEach((s, slot) => {
        if (s?.effect.kind === "plugin" && s.effect.state === "stopped")
          out.push({ name: s.effect.name, where: stripName(song, strip, library), slot: { kind: "insert", insert: { strip, slot } } });
      });
    return out;
  });

  const missingRef = (m: MissingView) => (m.kind === "sf2" ? { kind: "sf2" as const, file: m.file } : { kind: "plugin" as const, key: m.key });
</script>

<!-- Star (part.shelfrow.star) (spec 2 "Instrument row (shelf)") and the sample button ▶ / ■. -->
{#snippet star(on: boolean)}
  <svg class="star" class:on viewBox="0 0 12 12" aria-hidden="true"
    ><path d="M6 .5l1.7 3.6 3.9.5-2.9 2.7.7 3.9L6 9.3 2.6 11.2l.7-3.9L.4 4.6l3.9-.5z" /></svg
  >
{/snippet}
{#snippet play()}<svg class="icon" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l8 5-8 5z" /></svg>{/snippet}
{#snippet stop()}<svg class="icon" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1h8v8h-8z" /></svg>{/snippet}

{#snippet formats(plugin: PluginRowView)}
  {@const used = plugin.formats.find((f) => f.key === plugin.key)?.format ?? plugin.formats[0]?.format}
  {#if used}<span class="m-num mark used">{formatName(used)}</span>{/if}
  {#if plugin.formats.length > 1}
    <button class="m-plain m-num m-lbl more" data-test="shelf-formats" use:tip={formatsTip(plugin)} onclick={(e) => chooseFormat(e, plugin)}
      >{t("shelf.moreFormats", { count: plugin.formats.length - 1 })}</button
    >
  {/if}
{/snippet}

<!-- A row of a preset not fetched yet (spec 2, spec 3 (45-A), mock F8-PackGet): name gray.8, and 「取ってきています」 or 「まだ」 on the right.
     No star and no ▶; it cannot be chosen. The name starts at the ▶ position. -->
{#snippet notYetRow(r: ShelfRow)}
  <div class="row look not-yet" class:added={r.added} data-row={r.sound?.kind === "builtin" ? r.sound.instrument : r.id} data-test="shelf-not-yet">
    <span class="star-hit"></span>
    <span class="name gone">{r.name}</span>
    <span class="m-lbl" data-test="shelf-not-yet-state">{t(r.notYet === "fetching" ? "shelf.packFetching" : "shelf.packNotYet")}</span>
  </div>
{/snippet}

{#snippet row(r: ShelfRow)}
  {@const selected = picked?.id === r.id && boxShown}
  {@const look = !picks}
  {@const sampling = r.sound !== null && shelf.soundSample !== null && rowIs(r, shelf.soundSample.sound)}
  <div
    class="row"
    class:added={r.added}
    class:selected
    class:look
    role="presentation"
    data-row={r.sound ? (r.sound.kind === "builtin" ? r.sound.instrument : r.id) : undefined}
    data-effect={r.effect ? (r.effect.kind === "builtin" ? r.effect.effect : r.id) : undefined}
    data-test="shelf-row"
    onclick={() => pick(r)}
  >
    <button
      class="m-plain star-hit m-focus-in"
      aria-label={t("shelf.favorite")}
      aria-pressed={isFavorite(r)}
      data-test="shelf-star"
      onclick={(e) => {
        e.stopPropagation();
        toggleFavorite(r);
      }}>{@render star(isFavorite(r))}</button
    >
    {#if r.sound}
      {@const sound = r.sound}
      <button
        class="sample"
        class:playing={sampling}
        aria-label={t("shelf.playSample")}
        use:term={"サンプル"}
        data-test="shelf-sample"
        onclick={(e) => {
          e.stopPropagation();
          send(sampleCommand(app, sound));
        }}>{#if sampling && shelf.soundSample?.playing}{@render stop()}{:else}{@render play()}{/if}</button
      >
    {/if}
    {#if r.builtin && findTerm(r.name)}
      <span class="name" use:term={r.name}>{r.name}</span>
    {:else}
      <span class="name">{r.name}</span>
    {/if}
    {#if sampling && sampleTime}
      <span class="m-num time" data-test="shelf-sample-time">{sampleTime}</span>
    {:else if r.sound && shelf.mode.kind === "replace" && rowIs(r, shelf.currentSound)}
      <span class="m-lbl">{t("shelf.current")}</span>
    {:else if !r.sound && shelf.mode.kind === "replaceEffect" && rowIs(r, null, shelf.currentEffect)}
      <span class="m-lbl" data-test="shelf-current-effect">{t("shelf.currentEffect")}</span>
    {:else if r.number}
      <span class="m-num m-lbl">{r.number}</span>
    {:else if r.plugin}
      <span class="m-lbl vendor">{r.plugin.vendor}</span>
      {@render formats(r.plugin)}
    {:else if r.effect}
      <span class="m-lbl">{t("params.builtin")}</span>
    {/if}
  </div>
{/snippet}

{#snippet box()}
  {#if effectsView}
    {@const mode = shelf.mode}
    <div class="box" data-test="shelf-insert">
      {#if mode.kind === "insertEffect" && shelf.chosenEffect}
        <span class="say"
          >{t("shelf.insertInto", { strip: stripName(app.song, mode.strip, library), slot: mode.slot + 1 })}{#if mixStart}{t("shelf.mixStart")}{/if}</span
        >
      {:else if mode.kind === "replaceEffect" && shelf.chosenEffect}
        <span class="say"
          >{t("shelf.insertReplace", {
            strip: stripName(app.song, mode.strip, library),
            slot: mode.slot + 1,
            current: shelf.currentEffect ? effectRefName(shelf.currentEffect, library) : "",
            effect: effectRefName(shelf.chosenEffect, library),
          })}</span
        >
      {/if}
      <div class="buttons">
        <Button primary onclick={() => send({ type: "shelfConfirm" })} disabled={shelf.chosenEffect === null} test="shelf-insert-confirm">
          {mode.kind === "replaceEffect" ? t("button.replaceInstrument") : t("button.insertEffect")}
          <span class="m-num m-lbl">{t("key.enter")}</span>
        </Button>
        {#if shelf.chosenEffect !== null}
          <Button onclick={() => send({ type: "shelfCancel" })} test="shelf-cancel"
            >{t("button.cancel")} <span class="m-num m-lbl">{t("key.esc")}</span></Button
          >
        {/if}
      </div>
    </div>
  {:else if shelf.selectedSound === null && shelf.loadingSound === null && shelf.unreadable}
    <!-- Under a row that could not load, the box of the notice of missing things (spec 3 (89-12)). -->
    <div class="notice" data-test="shelf-unreadable">
      <span class="say">{t("plugin.loadFailedShelf", { name: soundRefName(shelf.unreadable, library) })}</span>
      <div class="buttons">
        <Button onclick={() => send({ type: "shelfCancel" })} test="shelf-cancel"
          >{t("button.cancel")} <span class="m-num m-lbl">{t("key.esc")}</span></Button
        >
      </div>
    </div>
  {:else if shelf.loadingSound}
    {@const plugin = shelf.loadingSound.kind === "plugin"}
    <div class="box" data-test="shelf-loading">
      <span class="accent">{plugin ? t("shelf.loadingTitle") : t("shelf.auditioning")}</span>
      <span class="say">{plugin ? t("plugin.loading", { name: pickedName }) : t("msg.auditionLoading", { name: pickedName })}</span>
      <div class="buttons">
        <Button onclick={() => send({ type: "shelfCancel" })} test="shelf-cancel"
          >{t("button.cancel")} <span class="m-num m-lbl">{t("key.esc")}</span></Button
        >
      </div>
    </div>
  {:else}
    <div class="box" data-test="shelf-audition">
      <span class="accent">{t("shelf.auditioning")}</span>
      <span class="say">{t(shelf.mode.kind === "replace" ? "msg.auditionReplace" : "msg.audition")}</span>
      <div class="buttons">
        <Button primary onclick={() => send({ type: "shelfConfirm" })} disabled={busy} test="shelf-confirm">
          {shelf.mode.kind === "replace" ? t("button.replaceInstrument") : t("button.makeTrack", { number: app.song.tracks.length + 1 })}
          <span class="m-num m-lbl">{t("key.enter")}</span>
        </Button>
        <Button onclick={() => send({ type: "shelfCancel" })} test="shelf-cancel"
          >{t("button.cancel")} <span class="m-num m-lbl">{t("key.esc")}</span></Button
        >
      </div>
      {#if shelf.selectedSound?.kind === "plugin"}
        <div class="buttons window">
          <Button onclick={() => send({ type: "openPluginWindow", slot: null })} test="shelf-open-window">{t("button.openWindow")}</Button>
          <span class="m-lbl">{t("shelf.windowHint")}</span>
        </div>
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet notice(m: MissingView | null)}
  {#if m === null}
    {#if app.sounds.fetch.state !== "fetching"}<div class="notice" data-test="shelf-notice-pack">
      <span class="say">{t("shelf.soundsMissing")}</span>
      <div class="buttons">
        <Button primary onclick={() => send({ type: "fetchPack" })} test="shelf-fetch-pack"
          >{t("button.fetchPack")}</Button
        >
      </div>
    </div>{/if}
  {:else if m.kind === "sf2"}
    <div class="notice" data-test="shelf-notice-sf2">
      <span class="say">{t("sf2.missing", { file: m.file, count: m.tracks })}</span>
      <div class="buttons">
        <Button primary disabled={busy} onclick={() => send({ type: "addSf2" })} test="shelf-add-sf2">{t("button.addSf2")}</Button>
        <Button disabled={busy} onclick={() => send({ type: "removeMissing", missing: missingRef(m) })} test="shelf-remove-missing"
          >{t("button.removeFromSong")}</Button
        >
      </div>
    </div>
  {:else}
    <div class="missing" data-test="shelf-notice-plugin">
      <div class="row added look">
        <span class="name gone">{m.name}</span>
        <span class="m-lbl">{t("shelf.missingRow")}</span>
        {#if m.format}<span class="m-num mark">{formatName(m.format)}</span>{/if}
      </div>
      <div class="notice">
        {#if m.shownPath !== null}
          <span class="say"
            >{#each textParts("msg.pluginMissing", { path: m.shownPath }) as part, i (i)}{#if part.hole !== null}<span class="m-num">{part.text}</span
                >{:else}{part.text}{/if}{/each}</span
          >
        {/if}
        <div class="buttons">
          <Button primary onclick={() => send({ type: "addPluginPlace" })} test="shelf-locate">{t("button.locate")}</Button>
          <Button onclick={() => send({ type: "rescanPlugins" })} test="shelf-rescan">{t("button.rescan")}</Button>
          <Button disabled={busy} onclick={() => send({ type: "removeMissing", missing: missingRef(m) })} test="shelf-remove-missing"
            >{t("button.removeFromSong")}</Button
          >
        </div>
      </div>
    </div>
  {/if}
{/snippet}

<aside class="panel" data-band="shelf" data-shelf data-test="shelf">
  <div class="head">
    <button class="m-plain fold m-focus-in" onclick={() => send({ type: "toggleBand", band: "shelf" })} data-test="rail-fold">
      <span class="m-hi">▾</span>
      <span class="m-name">{t("band.shelf")}</span>
    </button>
    <span class="m-spacer"></span>
    <span class="switches">
      <Button on={!effectsView} onclick={() => (shelfUi.view = "instruments")} test="shelf-instruments"
        >{t("shelf.instrumentCount", { count: counts.instruments })}</Button
      >
      <Button on={effectsView} onclick={() => (shelfUi.view = "effects")} test="shelf-effects"
        >{t("shelf.effectCount", { count: counts.effects })}</Button
      >
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
      >{t("shelf.all")} <span class="m-num">{effectsView ? counts.effects : counts.instruments}</span></Button
    >
    <Button on={shelfUi.favoritesOnly} onclick={() => (shelfUi.favoritesOnly = true)} test="shelf-favorites"
      >{t("shelf.favorites")} <span class="m-num">{favorites}</span></Button
    >
    <span class="m-spacer"></span>
    <span class="m-lbl keys"
      >{#each effectsView ? textParts("shelf.favoriteKey", { favoriteKey: t("key.favorite") }) : textParts("shelf.keys", {
            sampleKey: t("key.playStop"),
            favoriteKey: t("key.favorite"),
          }) as part, i (i)}{#if part.hole !== null}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}{/each}</span
    >
  </div>
  <ScrollArea bind:viewport data-test="shelf-list">
    <div style:height="{offsets[range.from]}px"></div>
    {#each visible as e (e.id)}
      {#if e.kind === "row" && e.notYet}
        {@render notYetRow(e)}
      {:else if e.kind === "row"}
        {@render row(e)}
      {:else if e.kind === "heading"}
        <button
          class="m-plain group m-focus-in"
          class:sub={e.depth === 1}
          onclick={() => toggleHeading(e.id, library)}
          data-group={e.id}
          data-test="shelf-heading"
        >
          <span class={e.folded ? "m-lbl" : "m-hi"}>{e.folded ? "▸" : "▾"}</span>
          <span class="m-hi group-name">{e.label}</span>
          {#if e.sf2}<span class="m-num mark">{t("shelf.sf2Mark")}</span>{/if}
          <span class="m-num m-lbl">{e.count}</span>
        </button>
      {:else if e.kind === "section"}
        <div class="group section" data-test={e.id}>
          <span class="m-lbl section-name">{e.label}</span>
          {#if e.id === "section:added"}
            {#if effectsView}
              <Button onclick={() => send({ type: "rescanPlugins" })} test="shelf-rescan">{t("button.rescan")}</Button>
            {:else}
              <Button disabled={busy} onclick={() => send({ type: "addSf2" })} test="shelf-add-sf2">{t("button.addSf2")}</Button>
            {/if}
          {/if}
        </div>
      {:else if e.kind === "box"}
        <div class="measured" bind:offsetHeight={measured[e.id]}>{@render box()}</div>
      {:else if e.kind === "notice"}
        <div class="measured" bind:offsetHeight={measured[e.id]}>{@render notice(e.missing)}</div>
      {:else if e.kind === "scan"}
        <div class="measured" bind:offsetHeight={measured[e.id]}>
          {#if scan.state === "running"}<ScanStrip done={scan.done} total={scan.total} />{/if}
        </div>
      {/if}
    {/each}
    <div style:height="{offsets[entries.length]! - offsets[range.to]!}px"></div>
  </ScrollArea>
  {#each stopped as s (JSON.stringify(s.slot))}
    <div class="stopped" data-test="shelf-stopped">
      <span class="stopped-head">{t("plugin.stoppedHead", { name: s.name, track: s.where })}</span>
      <span class="say">{t("plugin.stoppedBody")}</span>
      <div class="buttons">
        <Button primary onclick={() => send({ type: "reloadPlugin", slot: s.slot })} test="shelf-reload">{t("button.reload")}</Button>
        <Button onclick={() => send({ type: "removePlugin", slot: s.slot })} test="shelf-remove-plugin">{t("button.remove")}</Button>
      </div>
    </div>
  {/each}
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
    height: var(--part-shelfrow-group);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    white-space: nowrap;
  }
  .head {
    height: var(--size-row);
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
  /* The text of the current mode (mock F4-ShelfSample: background gray.1, an accent line below). */
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
    border-bottom: var(--line-hair) solid var(--color-line-thin);
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
  /* Heading of SF2 presets (mock F5-SF2Shelf: left 28). */
  .group.sub {
    padding-left: var(--mock-shelf-indent);
  }
  .group-name,
  .section-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  button.group:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* Instrument row (spec 2: height 24; star, sample button, name, format mark). */
  .row {
    height: var(--part-shelfrow-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3) 0 var(--space-2);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    white-space: nowrap;
    cursor: pointer;
  }
  /* Rows added later and effect rows under a heading (mock F5-SF2Shelf, F5-ShelfFx: left 24). */
  .row.added {
    padding-left: var(--space-6);
  }
  /* A view-only list (effects in the instrument mode, instruments in the effect mode). */
  .row.look {
    cursor: default;
  }
  .row.look:hover {
    box-shadow: none;
  }
  .row:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* While auditioning: background gray.4, an accent line.mark on the left. */
  .row.selected {
    background: var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-accent);
    padding-left: calc(var(--space-2) - var(--line-mark));
  }
  .row.added.selected {
    padding-left: calc(var(--space-6) - var(--line-mark));
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
  /* A row not found (spec 2 "Instrument row (shelf)": text gray.8). */
  .name.gone {
    color: var(--color-gray-8);
  }
  .vendor {
    max-width: var(--part-shelfrow-vendor);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Format mark (the plain mark: border gray.7, text gray.8; the format in use: border gray.9, text gray.10). */
  .mark {
    flex-shrink: 0;
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-7);
    border-radius: var(--radius-1);
    color: var(--color-gray-8);
  }
  .mark.used {
    border-color: var(--color-gray-9);
    color: var(--color-gray-10);
  }
  .more {
    flex-shrink: 0;
  }
  .more:hover {
    color: var(--color-gray-10);
  }
  .time {
    color: var(--color-accent);
  }
  /* The hint and buttons that open under the chosen row (mock F4-ShelfAudition, F5-PluginAudition, F5-ShelfFx). */
  .box {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    border-left: var(--line-mark) solid var(--color-accent);
  }
  /* The notice of missing things (spec 2: a gray.8 line.mark on the left, padding 8 / 12). */
  .notice {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    border-left: var(--line-mark) solid var(--color-gray-8);
  }
  .accent {
    color: var(--color-accent);
  }
  .say {
    white-space: normal;
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-wrap: wrap;
  }
  /* Stopped plugin (mock A-Mixer: a rec line.hair border, margin 0 / 12 / 12, padding 8). */
  .stopped {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0 var(--space-3) var(--space-3);
    padding: var(--space-2);
    border: var(--line-hair) solid var(--color-rec);
  }
  .stopped-head {
    color: var(--color-rec);
    font-weight: var(--text-s-weight-600);
  }
</style>
