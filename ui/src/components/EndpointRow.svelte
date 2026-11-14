<!-- 出口 ・ 入口の行（仕様書 2 章「出口 ・ 入口の行」）。流れ 2 では高さ 32 とテスト音、音の設定では高さ 24。 -->
<script lang="ts">
  import { tipWhenCut } from "../lib/tip.svelte";
  import { t } from "../text/ja";
  import type { EndpointLook } from "./endpoint";

  let {
    look,
    selected,
    disabled,
    size,
    still = false,
    onpick,
    ontest,
  }: {
    look: EndpointLook;
    selected: boolean;
    disabled: boolean;
    size: "connect" | "settings";
    /** 見せるだけで押せない（見た目は「使えない」にしない）。 */
    still?: boolean;
    onpick?: () => void;
    /** あるときだけ、右はしにテスト音のボタンを出す。 */
    ontest?: () => void;
  } = $props();

  const connect = $derived(size === "connect");
  let row = $state<HTMLElement>();

  // ↑ ↓ でえらんだ行を、スクロールする一覧の中で見える所へ。
  $effect(() => {
    if (connect && selected) row?.scrollIntoView?.({ block: "nearest" });
  });
</script>

<div
  class="row"
  class:connect
  class:highlighted={connect && selected}
  class:disabled
  data-test="endpoint-row"
  data-selected={selected}
  data-disabled={disabled}
  bind:this={row}
>
  <button type="button" class="pick" disabled={disabled || still} onclick={() => onpick?.()} aria-pressed={selected}>
    <span class="choice" class:selected><span class="dot"></span></span>
    <span class="ep-name" class:bold={connect} class:dim={look.link === "nothing"}>{look.name}</span>
    {#if look.sideIsDevice}
      <span class="friendly" use:tipWhenCut>{look.side}</span>
    {:else}
      <span class="friendly">{look.side ?? ""}</span>
    {/if}
    {#if look.link !== null}
      <span class="link" class:yes={look.link !== "nothing"}>
        <span class="link-mark"></span>
        {look.link === "plugged"
          ? t("endpoint.plugged")
          : look.link === "connected"
            ? t("endpoint.connected")
            : t("endpoint.nothingPlugged")}
      </span>
    {/if}
  </button>
  {#if ontest && !disabled}
    <button type="button" class="test" aria-label={t("endpoint.testTone")} data-test="test-tone" onclick={ontest}>
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l8 5-8 5z" fill="currentColor" /></svg>
    </button>
  {/if}
</div>

<style>
  .row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--part-endpoint-height-settings);
    white-space: nowrap;
  }
  .row.connect {
    height: var(--part-endpoint-height-connect);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .row.highlighted {
    background: var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-accent);
    padding-left: calc(var(--space-3) - var(--line-mark));
  }
  .pick {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .pick:disabled {
    cursor: default;
  }
  .choice {
    box-sizing: border-box;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--part-choice-size);
    height: var(--part-choice-size);
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .choice.selected {
    border-color: var(--color-accent);
  }
  .dot {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .choice.selected .dot {
    background: var(--color-accent);
  }
  .ep-name {
    flex-shrink: 0;
    width: var(--part-endpoint-name);
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-10);
  }
  .ep-name.bold {
    font-weight: var(--text-s-weight-600);
  }
  .ep-name.dim {
    color: var(--color-gray-8);
  }
  .friendly {
    flex-shrink: 0;
    width: var(--part-endpoint-friendly);
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-8);
  }
  .link {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-gray-8);
  }
  .link.yes {
    color: var(--color-gray-10);
  }
  .link-mark {
    box-sizing: border-box;
    flex-shrink: 0;
    width: var(--part-endpoint-link);
    height: var(--part-endpoint-link);
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .link.yes .link-mark {
    border: 0;
    background: var(--color-gray-10);
  }
  .test {
    box-sizing: border-box;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--part-toggle-size);
    height: var(--part-toggle-size);
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .test:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .test svg {
    width: var(--part-button-icon);
    height: var(--part-button-icon);
  }
  .pick:focus-visible,
  .test:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }

  /* 使えない（仕様書 2 章）: 枠 gray.5 ・ 文字 gray.7 ・ 押せない */
  .row.disabled .ep-name,
  .row.disabled .friendly,
  .row.disabled .link {
    color: var(--color-gray-7);
  }
  .row.disabled .choice {
    border-color: var(--color-gray-5);
  }
  .row.disabled .choice .dot {
    background: none;
  }
  .row.disabled .link-mark {
    border-color: var(--color-gray-7);
  }
  .row.disabled .link.yes .link-mark {
    background: var(--color-gray-7);
  }
</style>
