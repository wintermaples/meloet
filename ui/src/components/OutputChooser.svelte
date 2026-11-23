<!-- Output chooser (flow 2; mock F-Connect2, spec 6 "No output yet"). It sits inside the time area of the arrange (position: relative). -->
<script lang="ts">
  import ScrollArea from "../lib/ScrollArea.svelte";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import Button from "./Button.svelte";
  import EndpointRow from "./EndpointRow.svelte";
  import UnusableEndpoints from "./UnusableEndpoints.svelte";
  import { endpointGroups } from "./endpoint";

  const app = $derived(current.state);
  const chooser = $derived(app?.workspace.chooser ?? null);
  const groups = $derived(endpointGroups(app?.outputs.list ?? []));
</script>

{#if app && chooser}
  <!-- When it does not fit the window, keep the question row and the 「ここから鳴らす」 row in place and scroll only the list between (spec 3). -->
  <div class="place" style:pointer-events="none">
    <div class="lead"></div>
    <section class="chooser" data-test="output-chooser">
      <div class="head">
        <span class="title">{t("msg.noOutputArrange")}</span>
        <span class="spacer"></span>
        <span class="label later" data-test="change-later">{t("connect.changeLater")}</span>
      </div>
      <ScrollArea data-test="chooser-list">
        {#each groups.usable as output (output.id)}
          <EndpointRow
            look={output.look}
            selected={chooser.highlighted === output.id}
            disabled={false}
            size="connect"
            onpick={() => send({ type: "chooserSelect", output: output.id })}
            ontest={() => send({ type: "testTone", output: output.id })}
          />
        {/each}
        <UnusableEndpoints
          rows={groups.unusable}
          heading={t("msg.unusableHeading")}
          size="connect"
          note={t("msg.unusableHeadingChooser", { switched: groups.switchedByPc, stopped: groups.stoppedByPc })}
        />
      </ScrollArea>
      <div class="foot">
        <Button primary test="play-here" onclick={() => send({ type: "chooserConfirm" })}>
          {t("button.playHere")}<span class="key">{t("key.enter")}</span>
        </Button>
        <span class="label help">{t("connect.help")}</span>
      </div>
    </section>
  </div>
{/if}

<style>
  /* The top is the mock position. If the arrange is low, move it up to a gap of 24 above and below, the same as dialogs. */
  .place {
    position: absolute;
    inset: 0;
    z-index: 1;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: var(--part-dialog-margin) var(--mock-arrange-note-left);
  }
  .lead {
    flex: 0 1 calc(var(--mock-arrange-note-top) - var(--part-dialog-margin));
  }
  .chooser {
    flex-shrink: 0;
    max-height: 100%;
    max-width: 100%;
    pointer-events: auto;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: var(--mock-chooser-width);
    padding: var(--space-3);
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-7);
    color: var(--color-gray-9);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
  }
  /* When there is not enough width, 「あとから…」 wraps to the next line and is hidden. */
  .head {
    flex-shrink: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    column-gap: var(--space-3);
    height: var(--part-endpoint-height-connect);
    overflow: hidden;
    white-space: nowrap;
  }
  .head > * {
    line-height: var(--part-endpoint-height-connect);
  }
  .title {
    flex-shrink: 0;
    color: var(--color-gray-10);
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
    font-weight: var(--text-m-weight-500);
  }
  .spacer {
    flex: 1;
  }
  .label {
    color: var(--color-gray-8);
  }
  .foot {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: calc(var(--space-2) + var(--part-endpoint-height-connect));
    padding-top: var(--space-2);
    white-space: nowrap;
  }
  .help {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .key {
    font-family: var(--font-mono);
    color: var(--color-gray-8);
  }
</style>
