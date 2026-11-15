<!-- 「いまは使えない出口」「いまは使えない入口」の見出しと、その下のえらべない行（仕様書 2 章「えらべない出口の行」、
     3 章「入口も、出口と同じに扱う…」、6 章）。たたんで始める。行は押せない。
     見出しはモック F-Connect2（流れ 2）と F-Settings（音の設定）。 -->
<script lang="ts">
  import { tipWhenCut } from "../lib/tip.svelte";
  import type { UnusableLook } from "./endpoint";

  let {
    rows,
    heading,
    note = null,
    size,
  }: {
    rows: readonly UnusableLook[];
    heading: string;
    /** 見出しの右の文。 */
    note?: string | null;
    size: "connect" | "settings";
  } = $props();

  let open = $state(false);
</script>

{#if rows.length > 0}
  <button
    type="button"
    class="head"
    class:connect={size === "connect"}
    aria-expanded={open}
    data-test="unusable-head"
    onclick={() => (open = !open)}
  >
    <span class:label={!open} class:strong={open}>{open ? "▾" : "▸"}</span>
    <span>{heading}</span>
    <span class="label mono">{rows.length}</span>
    {#if note}
      <span class="spacer"></span>
      <span class="label note">{note}</span>
    {/if}
  </button>
  {#if open}
    {#each rows as row (row.id)}
      <div class="row" class:connect={size === "connect"} data-test="unusable-row">
        <span class="choice"></span>
        <span class="name">{row.name}</span>
        <span class="device" use:tipWhenCut>{row.device ?? ""}</span>
        <span class="label">{row.reason}</span>
      </div>
    {/each}
  {/if}
{/if}

<style>
  .head {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: var(--size-row);
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-gray-9);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }
  .head.connect {
    height: var(--part-endpoint-height-connect);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .head:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }
  .label {
    color: var(--color-gray-8);
  }
  .strong {
    color: var(--color-gray-10);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .spacer {
    flex: 1;
  }
  .note {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--part-unusable-height);
    color: var(--color-gray-7);
    white-space: nowrap;
  }
  .row.connect {
    padding: 0 var(--space-3);
  }
  .choice {
    box-sizing: border-box;
    flex-shrink: 0;
    width: var(--part-choice-size);
    height: var(--part-choice-size);
    border: var(--line-hair) solid var(--color-gray-5);
  }
  .name {
    flex-shrink: 0;
    width: var(--part-endpoint-name);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .device {
    flex-shrink: 0;
    width: var(--part-endpoint-friendly);
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
