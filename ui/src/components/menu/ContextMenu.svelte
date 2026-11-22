<!-- 右クリックの一覧（docs/history/design-notes.ja.md B-60）。App にひとつだけ置く。右クリックされた部品が openContextMenu を呼び、ここが描く。
     押した所に 2 章の「一覧」の形でひらく（窓からはみ出すなら内がわへずらす）。行を押すと、とじてから行う。外を押す ・ Esc でとじる。
     WebKit の一覧は、どこでも出さない。文字の枠（input ・ textarea）の右クリックは、切り取り ・ コピー ・ 貼りつけ ・ ぜんぶ選ぶ。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { platformOf } from "../../keys/keys";
  import { menuOpened } from "../../lib/menu";
  import MenuList from "./MenuList.svelte";
  import { closeContextMenu, contextMenu, listEntries, openContextMenu } from "./contextMenu.svelte";
  import { keyNames, type ListEntry, type ListRow } from "./menus";
  import { textFieldOf, textFieldRows } from "./textField";

  const open = $derived(contextMenu.open);
  const entries = $derived<ListEntry[]>(open ? listEntries(open.entries) : []);

  let popup = $state<HTMLElement>();

  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  function run(row: ListRow): void {
    closeContextMenu();
    row.run?.();
  }

  $effect(() => {
    if (!open) return;
    const forget = menuOpened(() => closeContextMenu());
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && popup?.contains(target)) return;
      closeContextMenu();
    };
    const onResize = () => closeContextMenu();
    document.addEventListener("pointerdown", onDown, true);
    window.addEventListener("resize", onResize);
    return () => {
      forget();
      document.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("resize", onResize);
    };
  });

  onMount(() => {
    const keys = keyNames(platformOf(navigator));
    const onContext = (ev: MouseEvent) => {
      ev.preventDefault();
      const field = textFieldOf(ev.target);
      if (field) openContextMenu(ev.clientX, ev.clientY, textFieldRows(field, keys.mod));
    };
    document.addEventListener("contextmenu", onContext);
    return () => document.removeEventListener("contextmenu", onContext);
  });
</script>

{#if open}
  <div class="popup" style:left="{open.x}px" style:top="{open.y}px" use:toBody bind:this={popup}>
    {#key open}
      <MenuList {entries} onrun={run} focus="list" test="context-menu" />
    {/key}
  </div>
{/if}

<style>
  .popup {
    position: fixed;
    z-index: 40;
  }
</style>
