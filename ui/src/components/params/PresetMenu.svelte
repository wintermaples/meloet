<!-- The open list of 「プリセット ▾」 at the head of the parameter list of a built-in effect (round 12 M1; the mock in the
     round 12 spec): 「はじめから入っているもの」, 「自分で残したもの」 and 「いまの設定を残す…」, in a gray.5 frame on gray.2, the row of the
     preset the values are on gray.3. A row uses the preset (not while counting in or recording, as Rust). 「いまの設定を残す…」 turns
     into a name box in its place (at most PRESET_NAME_MAX characters): Enter keeps, Esc puts the row back. Right click on a row of
     「自分で残したもの」: 「消す」. Esc closes the list. Core keeps the presets; the screen keeps only whether the list is open. -->
<script lang="ts">
  import { tick } from "svelte";
  import type { PresetRef, SelectedInsert } from "../../bridge";
  import { FACTORY_PRESETS, PRESET_NAME_MAX } from "../../bridge/types";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { presetName } from "./presetName";

  let { sel, onclose }: { sel: SelectedInsert; onclose: () => void } = $props();

  const app = $derived(current.state!);
  const busy = $derived(app.transport.state === "recording" || app.transport.state === "countIn");
  const factory = $derived(FACTORY_PRESETS[sel.effect]);
  const mine = $derived(app.presets.user.find((u) => u.effect === sel.effect)?.names ?? []);

  let naming = $state(false);
  let name = $state("");
  let box = $state<HTMLInputElement>();

  const factoryRef = (id: string): PresetRef => ({ kind: "factory", id });
  const userRef = (name: string): PresetRef => ({ kind: "user", name });
  const isNow = (preset: PresetRef) => JSON.stringify(preset) === JSON.stringify(sel.preset);

  function use(preset: PresetRef): void {
    if (busy) return;
    send({ type: "applyPreset", insert: sel.insert, preset });
    onclose();
  }

  async function startNaming(): Promise<void> {
    naming = true;
    name = "";
    await tick();
    box?.focus();
  }

  function keep(): void {
    if (name.trim() === "") return;
    send({ type: "savePreset", insert: sel.insert, name });
    naming = false;
    onclose();
  }

  function remove(ev: MouseEvent, preset: string): void {
    ev.preventDefault();
    openContextMenu(ev.clientX, ev.clientY, [
      { kind: "row", label: "preset.delete", usable: true, run: () => send({ type: "deletePreset", effect: sel.effect, name: preset }) },
    ]);
  }

  // Keys stay here: the list around has its own ↑ ↓ and Enter.
  function onkeydown(ev: KeyboardEvent): void {
    if (ev.isComposing) return;
    if (ev.key === "Escape") {
      ev.preventDefault();
      if (naming) naming = false;
      else onclose();
    } else if (ev.key === "Enter" && naming) {
      ev.preventDefault();
      keep();
    }
    ev.stopPropagation();
  }
</script>

<div class="menu" role="presentation" {onkeydown} data-test="preset-menu">
  <div class="head m-lbl">{t("preset.factory")}</div>
  {#each factory as p (p.id)}
    {@const preset = factoryRef(p.id)}
    <button class="m-plain row m-focus-in" class:now={isNow(preset)} disabled={busy} onclick={() => use(preset)} data-test="preset-factory" data-id={p.id}
      >{presetName(sel.effect, preset)}</button
    >
  {/each}
  <div class="head m-lbl">{t("preset.user")}</div>
  {#each mine as n (n)}
    {@const preset = userRef(n)}
    <button
      class="m-plain row m-focus-in"
      class:now={isNow(preset)}
      disabled={busy}
      onclick={() => use(preset)}
      oncontextmenu={(ev) => remove(ev, n)}
      data-test="preset-user"
      data-name={n}>{n}</button
    >
  {/each}
  {#if naming}
    <div class="row">
      <input class="name" type="text" maxlength={PRESET_NAME_MAX} bind:value={name} bind:this={box} data-test="preset-name" />
    </div>
  {:else}
    <button class="m-plain row save m-focus-in" onclick={startNaming} data-test="preset-save">{t("preset.save")}</button>
  {/if}
</div>

<style>
  .menu {
    display: flex;
    flex-direction: column;
    margin: 0 var(--space-2) var(--space-1);
    background: var(--color-gray-2);
    border: var(--line-hair) solid var(--color-gray-5);
  }
  .head,
  .row {
    width: 100%;
    min-height: var(--size-row);
    display: flex;
    align-items: center;
    padding: 0 var(--space-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .head {
    color: var(--color-gray-7);
  }
  .row {
    display: block;
    line-height: var(--size-row);
  }
  .row:hover:not(:disabled) {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .row:disabled {
    color: var(--color-gray-7);
    cursor: default;
  }
  .now {
    background: var(--color-gray-3);
    color: var(--color-gray-10);
    font-weight: var(--text-s-weight-600);
  }
  .save {
    color: var(--color-accent);
  }
  .name {
    width: 100%;
    height: var(--part-input-height);
    padding: 0 var(--part-input-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-8);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    outline: none;
    vertical-align: middle;
  }
</style>
