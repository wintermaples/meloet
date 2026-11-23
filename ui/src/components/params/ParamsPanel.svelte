<!-- The params panel (the right panel; spec 3 table, "Params panel": heading 24, name, format and place 48, search box 32, the list;
     mocks F4-Mixer, F-ParamFew, F-ParamMany, F-ParamNoWindow, F5-MixerBypass, R9-Params). Core keeps the chosen item
     (AppState.params.selected). Only the screen keeps the search text (paramUi); it is emptied when the chosen item changes.
     For the same item in the same slot, the search text, open groups and scroll stay even when the list number (table)
     changes (undo, redo). The name row: the name, the format mark (「最初から入っている」 for built-in ones), 「とばす」 for an insert
     (「とばしている」 in the chosen form while bypassed), and 「窓をひらく」 or 「窓を前に出す」 for a plugin with a window. For a plugin
     that cannot open its window yet, or has none, a text above the list. Two lines when nothing is chosen (86-5).
     Built-in sounds and SF2 sounds (`sound` of SelectedParams; spec 3 92-1 and 19-A, mock F7-Answers92): the name and
     「最初から入っている」 (the 「SF2」 mark for SF2), 「この音色には、動かせるパラメータがありません。」, in gray.8
     「音量とパンはミキサーで、音色を変えるには入れかえます。」, and the button 「音源を入れかえる…」. From round 12 (E3) a sound that
     comes with meloet has the four rows of its knobs in place of the two texts (SoundKnobList); the button stays under them.
     An audio clip (`clip` of SelectedParams): the chosen clip panel (ClipPanel). Two or more audio clips (`clips` of ParamsView):
     the panel that changes them together (ClipsPanel). -->
<script lang="ts">
  import { untrack } from "svelte";
  import type { InsertRef, InsertView, SongView } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import Button from "../Button.svelte";
  import { stripName } from "../mixer/stripName";
  import { formatName, soundNameText } from "../sounds";
  import { openShelf } from "../tracks.svelte";
  import WindowMark from "../WindowMark.svelte";
  import BuiltinList from "./BuiltinList.svelte";
  import ClipPanel from "./ClipPanel.svelte";
  import ClipsPanel from "./ClipsPanel.svelte";
  import { allRows, rowCount } from "./paramRows";
  import { paramUi, stopTyping } from "./paramUi.svelte";
  import PluginList from "./PluginList.svelte";
  import SoundKnobList from "./SoundKnobList.svelte";
  import { builtinOf, pluginAt, pluginOf, selectedInsert, windowOf } from "./selected";

  let { onfold }: { onfold: () => void } = $props();

  const app = $derived(current.state!);
  const builtin = $derived(builtinOf(app.params));
  const plugin = $derived(pluginOf(app.params));
  const insert = $derived(selectedInsert(app.params));
  const pluginView = $derived(plugin ? pluginAt(app.song, plugin.slot) : null);
  const sound = $derived(app.params.selected?.kind === "sound" ? app.params.selected : null);
  const clip = $derived(app.params.selected?.kind === "clip" ? app.params.selected : null);
  const busy = $derived(app.transport.state === "recording" || app.transport.state === "countIn");

  function slotView(song: SongView, at: InsertRef): InsertView | null {
    const { strip, slot } = at;
    const slots = strip.kind === "master" ? song.master.slots : strip.kind === "return" ? song.returns[strip.bus === "a" ? 0 : 1].slots : song.tracks.find((x) => x.id === strip.id)?.slots;
    return slots?.[slot] ?? null;
  }
  const held = $derived(insert ? slotView(app.song, insert) : null);

  const selKey = $derived(JSON.stringify(builtin ? [builtin.insert, builtin.effect] : plugin ? [plugin.slot, pluginView?.key ?? null] : null));
  $effect(() => {
    void selKey;
    untrack(() => {
      paramUi.query = "";
      stopTyping();
    });
  });

  const name = $derived(builtin ? t(`effect.${builtin.effect}`) : (pluginView?.name ?? ""));
  const where = $derived.by(() => {
    if (insert) return t("params.where", { strip: stripName(app.song, insert.strip, app.library), slot: insert.slot + 1 });
    if (plugin?.slot.kind === "instrument") return t("params.whereInstrument", { strip: stripName(app.song, { kind: "track", id: plugin.slot.track }, app.library) });
    return "";
  });
  const ready = $derived(pluginView?.state === "ready");
  const open = $derived(plugin ? windowOf(app.windows, plugin.slot) !== null : false);
  /** Whether it can be bypassed (a stopped plugin cannot). */
  const canBypass = $derived(insert !== null && held !== null && !(held.effect.kind === "plugin" && held.effect.state === "stopped"));

  /** Esc while searching empties the search box (mock F-ParamSearch: 「さがす枠を空にすると、グループの一覧に戻ります Esc」). */
  function searchKey(e: KeyboardEvent): void {
    if (e.key !== "Escape" || e.isComposing || paramUi.query === "") return;
    e.preventDefault();
    e.stopPropagation();
    paramUi.query = "";
  }

  /** For a plugin that cannot open its window yet, the first 2 sentences are gray.10 and the rest gray.8 (mock F-ParamNoWindow). */
  const notice = $derived.by(() => {
    if (!plugin || plugin.window === "opens") return null;
    const sentences = t(plugin.window === "notYet" ? "msg.windowNotYet" : "msg.noWindow").match(/[^。]+。/g) ?? [];
    return { strong: sentences.slice(0, 2).join(""), rest: sentences.slice(2).join("") };
  });
</script>

<aside class="panel" data-band="params" data-test="params">
  <button class="m-plain head m-focus-in" onclick={onfold} data-test="rail-fold">
    <span class="m-hi">▾</span>
    <span class="m-name">{t("band.params")}</span>
    <span class="m-spacer"></span>
    <span class="m-num m-lbl">{t("key.params")}</span>
  </button>
  {#if builtin || plugin}
    <div class="who">
      <div class="line">
        <span class="m-name name" data-test="params-name">{name}</span>
        {#if builtin}
          <span class="m-lbl">{t("params.builtin")}</span>
        {:else if pluginView?.format}
          <span class="m-num mark" data-test="params-format">{formatName(pluginView.format)}</span>
        {/if}
        <span class="m-spacer"></span>
        {#if insert}
          <Button on={held?.bypassed === true} disabled={!canBypass} onclick={() => send({ type: "toggleBypass", insert: insert! })} test="params-bypass"
            >{t(held?.bypassed ? "params.bypassing" : "params.bypass")} <span class="m-num m-lbl">{t("key.bypass")}</span></Button
          >
        {/if}
        {#if plugin?.window === "opens"}
          <Button disabled={!ready} onclick={() => send({ type: "openPluginWindow", slot: plugin!.slot })} test="params-window"
            >{#if open}<WindowMark />{t("button.raiseWindow")}{:else}{t("button.openWindow")}{/if}</Button
          >
        {/if}
      </div>
      <span class="m-lbl where" data-test="params-where">{where}</span>
    </div>
    {#if notice}
      <div class="notice" data-test="params-notice">
        <span>{notice.strong}</span>
        {#if notice.rest}<span class="m-lbl">{notice.rest}</span>{/if}
      </div>
    {/if}
    {#if builtin || plugin?.table !== null}
      <label class="bar">
        <span class="m-lbl">{t("params.search")}</span>
        <input class="search" type="text" placeholder={t("params.searchLabel")} bind:value={paramUi.query} onkeydown={searchKey} data-test="params-search" />
        <span class="m-num m-lbl" data-test="params-count"
          >{t("params.count", { shown: paramUi.matches, total: builtin ? rowCount(allRows(builtin.effect, builtin.values)) : plugin!.count })}</span
        >
      </label>
    {/if}
    {#if builtin}
      <BuiltinList sel={builtin} />
    {:else if plugin && plugin.table !== null}
      {#key selKey}
        <PluginList sel={plugin} plugin={name} usable={pluginView?.state !== "stopped"} />
      {/key}
    {/if}
  {:else if app.params.clips}
    <ClipsPanel sel={app.params.clips} />
  {:else if clip}
    <ClipPanel sel={clip} />
  {:else if sound}
    <div class="who">
      <div class="line">
        <span class="m-name name" data-test="params-name">{sound.sound.kind === "sf2" ? sound.sound.name : soundNameText(sound.sound)}</span>
        {#if sound.sound.kind === "sf2"}
          <span class="m-num mark" data-test="params-format">{t("shelf.sf2Mark")}</span>
        {:else}
          <span class="m-lbl">{t("params.builtin")}</span>
        {/if}
      </div>
      <span class="m-lbl where" data-test="params-where"
        >{t("params.whereInstrument", { strip: stripName(app.song, { kind: "track", id: sound.track }, app.library) })}</span
      >
    </div>
    {#if sound.knobs}
      <SoundKnobList sel={sound} knobs={sound.knobs} />
    {/if}
    <div class="nothing" data-test={sound.knobs ? "params-sound-replace" : "params-no-sound-params"}>
      {#if !sound.knobs}
        <span>{t("params.noSoundParams")}</span>
        <span class="m-lbl">{t("params.noSoundParamsHint")}</span>
      {/if}
      <span
        ><Button test="params-replace-sound" disabled={busy} onclick={() => send(openShelf({ kind: "replace", track: sound.track }))}
          >{t("button.replaceSound")}</Button
        ></span
      >
    </div>
  {:else}
    <div class="nothing" data-test="params-nothing">
      <span>{t("params.nothingSelected")}</span>
      <span class="m-lbl">{t("params.nothingInstrument")}</span>
    </div>
  {/if}
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
  .head {
    width: 100%;
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    white-space: nowrap;
  }
  .head:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .head:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* Name, format and place (height 48; mock F4-Mixer: 12 left and right, a line.thin line below). */
  .who {
    height: calc(2 * var(--size-row));
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
    overflow: hidden;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mark {
    flex-shrink: 0;
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-7);
    border-radius: var(--radius-1);
    color: var(--color-gray-8);
  }
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* A plugin that cannot open its window yet, or has none (mock F-ParamNoWindow: a gray.7 line.mark on the left). */
  .notice {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-left: var(--line-mark) solid var(--color-gray-7);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: normal;
  }
  .notice .m-lbl {
    white-space: normal;
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
  .search {
    flex: 1;
    min-width: 0;
    height: var(--part-input-height);
    padding: 0 var(--part-input-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    outline: none;
  }
  .search::placeholder {
    color: var(--color-gray-7);
  }
  .search:focus {
    border-color: var(--color-gray-8);
  }
  .nothing {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    white-space: normal;
  }
  .nothing .m-lbl {
    white-space: normal;
  }
</style>
