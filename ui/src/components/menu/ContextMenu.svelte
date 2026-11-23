<!-- Context menu (docs/history/design-notes.ja.md B-60). Only one in App. The right-clicked part calls openContextMenu, and this draws it.
     Opens where pressed in the form of the 「一覧」 (List) of spec 2 (shifted inside if it goes outside the window). A row press closes, then acts. A press outside or Esc closes.
     The WebKit menu is never shown. Right click on a text box (input, textarea) gives cut, copy, paste, select all. -->
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
