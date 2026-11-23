<!-- The name box of 「いまのミュートを残す…」 and 「名前を変える ▸」 of the mute sets (B-97-3, B-97-5): the list closes and a frame of the
     list's look with one name field opens where it was (the look of the track and marker name fields). Enter sends one intent when it
     can be done; while it is open the status line says what Enter does or why it cannot (a name taken by another set). Esc or a press
     outside closes it without sending; so do counting in, recording, and the set being renamed going away. Only one in App. -->
<script lang="ts">
  import { tick } from "svelte";
  import { MUTE_SET_NAME_MAX } from "../../bridge";
  import { menuOpened } from "../../lib/menu";
  import { songBusy } from "../../keys/attach";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { dragReading } from "../pianoroll/view.svelte";
  import { closeMuteSetBox, muteSetBox, muteSetBoxResult, muteSetName } from "./muteSets.svelte";

  const open = $derived(muteSetBox.open);
  const app = $derived(current.state);
  let draft = $state("");
  let field = $state<HTMLInputElement>();
  let at = $state({ x: 0, y: 0 });
  let popup = $state<HTMLElement>();

  const result = $derived(open && app ? muteSetBoxResult(app, open, draft) : null);
  const broken = $derived(result !== null && result.command === null && result.reading !== null);

  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  let was: typeof open = null;
  $effect(() => {
    if (open === was) return;
    was = open;
    if (!open) return;
    draft = open.kind === "rename" ? open.from : "";
    if (open.at) at = open.at;
    else {
      const r = document.querySelector('[data-test="menubar"] [data-menu="5"]')?.getBoundingClientRect();
      at = r ? { x: r.left, y: r.bottom } : { x: 0, y: 0 };
    }
    void tick().then(() => {
      // Inside the window, as the list is.
      const width = popup?.offsetWidth ?? 0;
      if (at.x + width > window.innerWidth) at = { ...at, x: Math.max(0, window.innerWidth - width) };
      field?.focus();
      field?.select();
    });
  });

  $effect(() => {
    if (!open || !app) return;
    if (songBusy(app) || (open.kind === "rename" && !app.song.muteSets.some((s) => s.name === open.from))) closeMuteSetBox();
  });

  $effect(() => {
    if (!open) return;
    dragReading.text = result?.reading ?? null;
    return () => {
      dragReading.text = null;
    };
  });

  $effect(() => {
    if (!open) return;
    return menuOpened(() => closeMuteSetBox());
  });

  function key(e: KeyboardEvent): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      // A name that cannot be used keeps the box open; a rename to the name it has closes it.
      const command = result?.command;
      if (command) {
        closeMuteSetBox();
        send(command);
      } else if (open?.kind === "rename" && muteSetName(draft) === open.from) {
        closeMuteSetBox();
      }
    } else if (e.key === "Escape") {
      closeMuteSetBox();
    } else {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#if open}
  <div class="popup" style:left="{at.x}px" style:top="{at.y}px" use:toBody bind:this={popup}>
    <div class="frame" role="presentation">
      <input
        class="name-field"
        class:broken
        type="text"
        maxlength={MUTE_SET_NAME_MAX}
        spellcheck="false"
        autocomplete="off"
        aria-label={t("mixer.muteSetName")}
        aria-invalid={broken}
        data-test="mute-set-name"
        bind:this={field}
        bind:value={draft}
        onkeydown={key}
        onblur={closeMuteSetBox}
      />
    </div>
  </div>
{/if}

<style>
  .popup {
    position: fixed;
    z-index: 40;
  }
  /* The look of the list (spec 2 "List") with one row. */
  .frame {
    width: var(--part-list-width);
    display: flex;
    align-items: center;
    height: calc(var(--part-list-row) + 2 * var(--space-1));
    padding: 0 var(--space-3);
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .name-field {
    flex: 1;
    min-width: 0;
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    font-weight: var(--text-s-weight-600);
    outline: none;
  }
  .name-field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
</style>
