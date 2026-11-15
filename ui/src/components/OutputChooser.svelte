<!-- 出口えらび（流れ 2。モック F-Connect2、仕様書 6 章「音の出口がまだ」）。置き場はアレンジの時間の面（position: relative）の中。 -->
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
  <!-- 窓に入りきらないときは、問いかけの行と「ここから鳴らす」の行を動かさず、間の一覧だけスクロールする（仕様書 3 章）。 -->
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
  /* 上はモックの位置。アレンジが低ければ、ダイアログと同じく上下 24 ずつのあきまで上へ寄せる。 */
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
  /* 横が足りないときは「あとから…」が次の行へ折れて、見えなくなる。 */
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
