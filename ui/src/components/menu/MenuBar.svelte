<!-- 上のメニューの行（仕様書 2 章「上のメニューの行」、モック R5-Menus）。再生まわりの上、高さ size.menubar。左から「ファイル 編集 表示 再生 ノート トラック」。
     押すとすぐ下に一覧（MenuList）。ひらいている間は、ほかの名前にマウスを乗せると切りかわる。F10 は keys/attach.ts。
     Esc は Esc の順の「用語の説明やメニュー」の番（keys/attach.ts）。外を押すと、とじる。 -->
<script lang="ts">
  import { platformOf } from "../../keys/keys";
  import { menuOpened } from "../../lib/menu";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import MenuList from "./MenuList.svelte";
  import { closeMenuBar, menuBar, openMenuBar } from "./menuBar.svelte";
  import { TOP_MENUS, topMenus, type ListRow } from "./menus";

  const platform = platformOf(navigator);
  const deps = { state: () => current.state, send };

  // 中身は、ひらいている間だけ組み立てる。
  const menus = $derived(menuBar.open !== null && current.state ? topMenus(current.state, deps, platform) : null);
  const open = $derived(menuBar.open);

  let bar = $state<HTMLElement>();
  const names: (HTMLButtonElement | undefined)[] = $state([]);
  let popup = $state<HTMLElement>();
  let anchor = $state({ left: 0, top: 0 });

  function place(): void {
    if (open === null || !bar) return;
    const name = names[open];
    anchor = { left: name?.getBoundingClientRect().left ?? 0, top: bar.getBoundingClientRect().bottom };
  }
  // 一覧はダイアログより手前に出すので、.app の重なりの外（body）に置く。
  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  function toggle(i: number): void {
    if (open === i) closeMenuBar();
    else openMenuBar(i, false);
  }
  function step(by: number): void {
    if (open === null) return;
    openMenuBar((open + by + TOP_MENUS.length) % TOP_MENUS.length, true);
  }
  function run(row: ListRow): void {
    closeMenuBar();
    row.run?.();
  }

  // ひらく前にフォーカスのあった所。とじてフォーカスが行き場をなくしたら、そこへ戻す。
  let before: Element | null = null;
  let was: number | null = null;
  $effect.pre(() => {
    const now = open;
    if (was === null && now !== null) before = document.activeElement;
    if (was !== null && now === null) {
      const back = before;
      before = null;
      queueMicrotask(() => {
        const active = document.activeElement;
        if (active && active !== document.body) return;
        if (back instanceof HTMLElement && back.isConnected) back.focus();
      });
    }
    was = now;
  });
  $effect(() => {
    if (open === null) return;
    place();
    const forget = menuOpened(() => closeMenuBar());
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || bar?.contains(target))) return;
      closeMenuBar();
    };
    document.addEventListener("pointerdown", onDown, true);
    window.addEventListener("resize", place);
    return () => {
      forget();
      document.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("resize", place);
    };
  });
</script>

<div class="menubar" role="menubar" tabindex="-1" bind:this={bar} data-test="menubar">
  {#each TOP_MENUS as label, i (label)}
    <button
      class="m-plain name"
      class:open={open === i}
      role="menuitem"
      aria-haspopup="menu"
      aria-expanded={open === i}
      data-menu={i}
      bind:this={names[i]}
      onclick={() => toggle(i)}
      onpointerenter={() => {
        if (open !== null && open !== i) openMenuBar(i, false);
      }}>{t(label)}</button
    >
  {/each}
</div>
{#if open !== null && menus}
  <div class="popup" style:left="{anchor.left}px" style:top="{anchor.top}px" use:toBody bind:this={popup}>
    {#key open}
      <MenuList
        entries={menus[open]!.entries}
        onrun={run}
        onleft={() => step(-1)}
        onright={() => step(1)}
        focus={menuBar.byKeys ? "first" : "list"}
        test="menubar-list"
      />
    {/key}
  </div>
{/if}

<style>
  .menubar {
    height: var(--size-menubar);
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    padding: 0 var(--space-1);
    background: var(--color-gray-1);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .name {
    display: inline-flex;
    align-items: center;
    padding: 0 var(--space-2);
    color: var(--color-gray-9);
  }
  .name:hover {
    color: var(--color-gray-10);
  }
  .name.open {
    background: var(--color-gray-4);
    color: var(--color-gray-10);
  }
  .name:focus-visible {
    outline-offset: calc(-2 * var(--line-hair));
  }
  .popup {
    position: fixed;
    z-index: 40;
  }
</style>
