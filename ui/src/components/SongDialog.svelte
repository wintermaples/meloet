<!-- Song dialog (spec 2 "Dialog", wording in spec 6). The heading is the song name, or the name of the file that could not be read (the owner's decision).
     Esc is Rust's back (cancel). Enter is the bright button at the right end (keys/attach.ts). -->
<script lang="ts">
  import { movableDialog } from "../lib/dialogMove";
  import ScrollArea from "../lib/ScrollArea.svelte";
  import { current, send } from "../state/app.svelte";
  import Button from "./Button.svelte";
  import { metrics } from "./metrics.svelte";
  import { songDialogLook } from "./songDialog";

  const dialog = $derived(current.state?.workspace.songDialog ?? null);
  const look = $derived(dialog ? songDialogLook(dialog) : null);
</script>

{#if dialog && look}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={dialog.title} data-test="song-dialog" style:color-scheme={metrics.colorScheme} use:movableDialog>
      <div class="head" data-dialog-head><span class="title">{dialog.title}</span></div>
      <ScrollArea data-test="song-dialog-body">
        <p class="body">{look.body}</p>
      </ScrollArea>
      <div class="foot">
        {#each look.buttons as b (b.answer)}
          <Button primary={b.primary} test="song-{b.answer}" onclick={() => send({ type: "answerSongDialog", dialog: dialog.id, answer: b.answer })}
            >{b.label}</Button
          >
        {/each}
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  /* The height follows the content, up to a gap of 24 above and below the window. Past that, only the content between scrolls. */
  .place {
    position: absolute;
    inset: 0;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: var(--part-dialog-margin) 0;
  }
  .lead {
    flex: 1 1 0;
  }
  .dialog {
    flex-shrink: 0;
    pointer-events: auto;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: var(--part-dialog-width);
    max-height: 100%;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    border-radius: var(--radius-1);
    color: var(--color-gray-9);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
  }
  .head {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    height: var(--part-dialog-head);
    padding: 0 var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
    overflow: hidden;
    cursor: move;
  }
  /* Hovering or moving the heading row: bg gray.4 (spec 2 "Dialog heading row (moving)"). */
  .head:hover {
    background: var(--color-gray-4);
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-10);
    font-weight: var(--text-s-weight-600);
  }
  .body {
    margin: 0;
    padding: var(--space-4);
  }
  .foot {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    height: var(--part-dialog-foot);
    padding: 0 var(--space-4);
    border-top: var(--line-hair) solid var(--color-gray-6);
  }
</style>
