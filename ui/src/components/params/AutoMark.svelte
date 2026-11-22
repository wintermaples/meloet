<!-- パラメータの行の右の「オートメーションに書く」印（仕様書 2 章「パラメータの行」、3 章 Y16、20×20）。
     押すと、その動きを書く帯をトラックの下に足す（たたんだ帯なら、ひらく）。帯がある行は accent（オートメーションに書いている）。
     戻り A ・ B とマスターのインサートは、帯を作らないので「使えない」（docs/history/design-notes.ja.md A-19 の 7）。 -->
<script lang="ts">
  import { t } from "../../text/ja";

  let { on, usable, add }: { on: boolean; usable: boolean; add: () => void } = $props();
</script>

<button
  class="automation"
  class:on
  class:usable
  aria-disabled={!usable || undefined}
  aria-pressed={on}
  aria-label={t("params.automation")}
  data-test="param-automation"
  onpointerdown={(e) => e.stopPropagation()}
  onclick={() => usable && add()}><svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 8L3 3l3 4 4-6" /></svg></button
>

<style>
  /* 使えない: 枠 gray.5、文字 gray.7、押せない。使える: 枠 gray.6、文字 gray.8。書いている行は accent。 */
  .automation {
    box-sizing: border-box;
    width: var(--part-param-automation);
    height: var(--part-param-automation);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-5);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-7);
    cursor: default;
  }
  .automation.usable {
    border-color: var(--color-gray-6);
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .automation.usable:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .automation.on,
  .automation.on:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .automation svg {
    width: var(--part-button-icon);
    height: var(--part-button-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }
</style>
