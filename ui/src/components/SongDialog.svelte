<!-- 曲のダイアログ（仕様書 2 章「ダイアログ」、6 章の文）。見出しは曲の名前か、読めなかったファイルの名前（ヤチヨさんの決め）。
     Esc は Rust の back（やめる）。Enter は右はしの明るいボタン（keys/attach.ts）。 -->
<script lang="ts">
  import { movableDialog } from "../lib/dialogMove";
  import ScrollArea from "../lib/ScrollArea.svelte";
  import { current, send } from "../state/app.svelte";
  import Button from "./Button.svelte";
  import { songDialogLook } from "./songDialog";

  const dialog = $derived(current.state?.workspace.songDialog ?? null);
  const look = $derived(dialog ? songDialogLook(dialog) : null);
</script>

{#if dialog && look}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={dialog.title} data-test="song-dialog" use:movableDialog>
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
  /* 高さは中身に合わせ、窓の上下 24 ずつのあきまで。それをこえたら、間の中身だけスクロールする。 */
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
