<!-- 出口えらび（流れ 2。モック F-Connect2、仕様書 6 章「音の出口がまだ」）。置き場はアレンジの時間の面（position: relative）の中。 -->
<script lang="ts">
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import Button from "./Button.svelte";
  import EndpointRow from "./EndpointRow.svelte";
  import { endpointLook } from "./endpoint";

  const app = $derived(current.state);
  const chooser = $derived(app?.workspace.chooser ?? null);
</script>

{#if app && chooser}
  <section class="chooser" data-test="output-chooser">
    <div class="head">
      <span class="title">{t("msg.noOutputArrange")}</span>
      <span class="spacer"></span>
      <span class="label">{t("connect.changeLater")}</span>
    </div>
    {#each app.outputs.list as output (output.id)}
      <EndpointRow
        look={endpointLook(output)}
        selected={chooser.highlighted === output.id}
        disabled={!output.choosable}
        size="connect"
        onpick={() => send({ type: "chooserSelect", output: output.id })}
        ontest={() => send({ type: "testTone", output: output.id })}
      />
    {/each}
    <div class="foot">
      <Button primary test="play-here" onclick={() => send({ type: "chooserConfirm" })}>
        {t("button.playHere")}<span class="key">{t("key.enter")}</span>
      </Button>
      <span class="label">{t("connect.help")}</span>
    </div>
  </section>
{/if}

<style>
  .chooser {
    position: absolute;
    left: var(--mock-arrange-note-left);
    top: var(--mock-arrange-note-top);
    z-index: 1;
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
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--part-endpoint-height-connect);
    white-space: nowrap;
  }
  .title {
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
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: calc(var(--space-2) + var(--part-endpoint-height-connect));
    padding-top: var(--space-2);
    white-space: nowrap;
  }
  .key {
    font-family: var(--font-mono);
    color: var(--color-gray-8);
  }
</style>
