<!-- パラメータの欄（右の欄。仕様書 1 章「パラメータ」: 見出し 24、名前と形式と居場所 48、さがす枠 32、一覧。モック F4-Mixer ・ F-ParamFew ・ F-ParamMany ・
     F-ParamNoWindow ・ F5-MixerBypass ・ R9-Params）。えらんだ子は core が持つ（AppState.params.selected）。さがす文字は画面だけが持ち（paramUi）、えらぶ子が変わると空に戻す。
     同じ枠の同じ子のあいだは、一覧の番号（table）が変わっても（取り消し ・ やりなおし）、さがす文字 ・ ひらいたグループ ・ スクロールを残す。
     名前の行: 名前、形式の印（最初から入っている物は「最初から入っている」）、インサートなら「とばす」（とばしている間は「とばしている」で、えらんでいる形）、
     窓をひらける子は「窓をひらく」か「窓を前に出す」。まだ窓をひらけない子 ・ 窓を持たない子は、一覧の上に文。
     何もえらんでいないときは 2 行（86-5）。 -->
<script lang="ts">
  import { untrack } from "svelte";
  import type { InsertRef, InsertView, SongView } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import Button from "../Button.svelte";
  import { stripName } from "../mixer/stripName";
  import { formatName } from "../sounds";
  import WindowMark from "../WindowMark.svelte";
  import BuiltinList from "./BuiltinList.svelte";
  import { allRows, rowCount } from "./paramRows";
  import { paramUi, stopTyping } from "./paramUi.svelte";
  import PluginList from "./PluginList.svelte";
  import { builtinOf, pluginAt, pluginOf, selectedInsert, windowOf } from "./selected";

  let { onfold }: { onfold: () => void } = $props();

  const app = $derived(current.state!);
  const builtin = $derived(builtinOf(app.params));
  const plugin = $derived(pluginOf(app.params));
  const insert = $derived(selectedInsert(app.params));
  const pluginView = $derived(plugin ? pluginAt(app.song, plugin.slot) : null);

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
  /** とばせるか（止まったプラグインはとばせない）。 */
  const canBypass = $derived(insert !== null && held !== null && !(held.effect.kind === "plugin" && held.effect.state === "stopped"));

  /** さがしているときの Esc は、さがす枠を空にする（モック F-ParamSearch「さがす枠を空にすると、グループの一覧に戻ります Esc」）。 */
  function searchKey(e: KeyboardEvent): void {
    if (e.key !== "Escape" || e.isComposing || paramUi.query === "") return;
    e.preventDefault();
    e.stopPropagation();
    paramUi.query = "";
  }

  /** まだ窓をひらけない子の文は、はじめの 2 文を gray.10、のこりを gray.8（モック F-ParamNoWindow）。 */
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
  /* 名前と形式と居場所（高さ 48。モック F4-Mixer: 左右 12、下に gray.5 の線）。 */
  .who {
    height: calc(2 * var(--size-row));
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
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
  /* まだ窓をひらけない子 ・ 窓を持たない子（モック F-ParamNoWindow: 左に gray.7 の line.mark）。 */
  .notice {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-left: var(--line-mark) solid var(--color-gray-7);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
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
    border-bottom: var(--line-hair) solid var(--color-gray-5);
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
</style>
