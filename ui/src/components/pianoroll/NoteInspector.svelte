<!-- ピアノロールの右の「選んだノート」の中身（モック A-PianoRoll・F-WidePR の右の欄: 選んだノート、クオンタイズ、スケール、長さをそろえる、
     ベロシティをまとめて変える）。見出し（▾ 選んだノート 3）とたたんだ縦の見出しは、帯の部品（PianoRoll.svelte）が描く。これは、ひらいたときの中身だけを、親の箱いっぱいに描く。
     区切り1 で動くのは「スケール」の音階と固定だけ（setMusicalScale を送る）。選んだノートは見るだけ。クオンタイズは値を見せる「使えない」の見た目。
     「長さをそろえる」「ベロシティをまとめて変える」は、中身が仕様書にもモックにもないので、たたんだ見出しだけの「使えない」。 -->
<script lang="ts">
  import type { ScaleMode } from "../../bridge";
  import { menuOpened } from "../../lib/menu";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { term } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { gridText, scaleName } from "../format";
  import { selectedSummary } from "./notes";
  import ScaleMenu from "./ScaleMenu.svelte";
  import { shown } from "./shown.svelte";

  const app = $derived(current.state!);
  const pr = $derived(app.pianoRoll);
  const meter = $derived({ num: app.song.meterNum, den: app.song.meterDen });
  const take = $derived(shown.take);
  const summary = $derived(
    take && shown.clip && pr.take === take.id ? selectedSummary(take, shown.clip, pr.selectedNotes, meter) : null,
  );
  const count = $derived(summary?.count ?? 0);
  const quantized = $derived((take?.origin ?? 0) !== 0);
  const q = $derived(pr.quantize);

  const velocity = $derived(
    summary === null
      ? ""
      : summary.velocityLow === summary.velocityHigh
        ? String(summary.velocityLow)
        : t("inspector.range", { low: summary.velocityLow, high: summary.velocityHigh }),
  );

  function setScale(root: number, mode: ScaleMode, lock: boolean): void {
    send({ type: "setMusicalScale", root, mode, lock });
  }

  // ---- 音階の一覧 ----

  let open = $state(false);
  let scaleButton = $state<HTMLButtonElement>();
  let popup = $state<HTMLElement>();
  let place = $state<{ right: number; top: number | null; bottom: number | null; maxHeight: number }>({
    right: 0,
    top: null,
    bottom: null,
    maxHeight: 0,
  });

  /** 押した行のすぐ下か上（広いほう）にひらく。右はしは行にそろえる。 */
  function placeMenu(): void {
    const row = scaleButton?.closest(".rw");
    if (!row) return;
    const r = row.getBoundingClientRect();
    const below = window.innerHeight - r.bottom;
    const above = r.top;
    place =
      below >= above
        ? { right: window.innerWidth - r.right, top: r.bottom, bottom: null, maxHeight: below }
        : { right: window.innerWidth - r.right, top: null, bottom: window.innerHeight - r.top, maxHeight: above };
  }

  function closeScale(): void {
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    open = false;
    if (hadFocus) scaleButton?.focus();
  }

  function pickScale(root: number, mode: ScaleMode): void {
    setScale(root, mode, pr.scale.lock);
    closeScale();
  }

  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  $effect(() => {
    if (!open) return;
    placeMenu();
    window.addEventListener("resize", placeMenu);
    const forget = menuOpened(() => closeScale());
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || scaleButton?.contains(target))) return;
      closeScale();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", placeMenu);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });
</script>

{#snippet slider(label: string, termName: string | null, value: number)}
  <div class="rw off">
    {#if termName}
      <span class="slider-label" use:term={termName}>{label}</span>
    {:else}
      <span class="slider-label">{label}</span>
    {/if}
    <div class="slider">
      <div class="track"></div>
      <div class="fill" style:width="{value}%"></div>
      <div class="knob" style:left="{value}%"></div>
    </div>
    <span class="m-num slider-value">{t("inspector.percent", { value })}</span>
  </div>
{/snippet}

<div class="inspector" data-testid="pr-inspector">
  <ScrollArea>
    <div class="rw">
      <span class="m-lbl grow">{t("note.position")}</span>
      <span class="m-num m-hi" data-test="inspector-position">
        {#if summary}{count > 1 ? t("inspector.from", { position: summary.position }) : summary.position}{/if}
      </span>
    </div>
    <div class="rw">
      <span class="m-lbl grow">{t("note.key")}</span>
      <span class="m-num m-hi value" data-test="inspector-keys">{summary?.keys ?? ""}</span>
    </div>
    <div class="rw">
      <span class="m-lbl grow">{t("note.length")}</span>
      <span class="m-num m-hi" data-test="inspector-length">{summary ? (summary.length ?? t("inspector.mixed")) : ""}</span>
    </div>
    <div class="rw">
      <span class="m-lbl grow" use:term={"ベロシティ"}>{t("pr.velocity")}</span>
      <span class="m-num m-hi" data-test="inspector-velocity">{velocity}</span>
    </div>

    <div class="hd">
      <span class="m-hi">▾</span>
      <span class="m-hi grow" use:term={"クオンタイズ"}>{t("inspector.quantize")}</span>
      {#if quantized}<span class="m-lbl" data-test="inspector-quantized">{t("inspector.quantized")}</span>{/if}
    </div>
    <div class="rw off">
      <span class="grow" use:term={"グリッド"}>{t("pr.grid")}</span>
      <span class="m-num">{gridText(q.division, q.triplet)}</span>
    </div>
    {@render slider(t("inspector.strength"), null, q.strength)}
    {@render slider(t("inspector.swing"), "スウィング", q.swing)}
    <div class="rw off">
      <span class="grow">{t("inspector.align")}</span>
      <span>{t(q.align === "start" ? "inspector.alignStart" : "inspector.alignStartAndEnd")}</span>
    </div>
    <div class="buttons">
      <button class="m-btn" aria-disabled="true" tabindex="-1">{t("inspector.requantize")}</button>
      <button class="m-btn" aria-disabled="true" tabindex="-1">{t("inspector.unquantize")}</button>
    </div>

    <div class="hd">
      <span class="m-hi">▾</span>
      <span class="m-hi grow">{t("inspector.scale")}</span>
    </div>
    <div class="rw">
      <span class="m-lbl grow">{t("inspector.musicalScale")}</span>
      <button
        class="m-plain m-num m-hi pick"
        aria-haspopup="menu"
        aria-expanded={open}
        data-test="inspector-scale"
        bind:this={scaleButton}
        onclick={() => (open = !open)}>{scaleName(pr.scale.root, pr.scale.mode)}</button
      >
    </div>
    <div class="rw">
      <span class="m-lbl grow" use:term={"スケール固定"}>{t("inspector.lock")}</span>
      <button
        class="m-plain m-num m-hi pick"
        class:on={pr.scale.lock}
        aria-pressed={pr.scale.lock}
        data-test="inspector-lock"
        onclick={() => setScale(pr.scale.root, pr.scale.mode, !pr.scale.lock)}>{t(pr.scale.lock ? "inspector.on" : "inspector.off")}</button
      >
    </div>

    <button class="m-plain hd folded" aria-disabled="true" tabindex="-1">
      <span class="arrow">▸</span>
      <span class="grow">{t("inspector.sameLength")}</span>
    </button>
    <button class="m-plain hd folded" aria-disabled="true" tabindex="-1">
      <span class="arrow">▸</span>
      <span class="grow">{t("inspector.velocityAll")}</span>
    </button>
  </ScrollArea>
</div>

{#if open}
  <div
    class="popup"
    use:toBody
    bind:this={popup}
    style:right="{place.right}px"
    style:top={place.top === null ? null : `${place.top}px`}
    style:bottom={place.bottom === null ? null : `${place.bottom}px`}
    style:max-height="{place.maxHeight}px"
  >
    <ScaleMenu scale={pr.scale} onpick={pickScale} />
  </div>
{/if}

<style>
  .inspector {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    overflow: hidden;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  /* 見出し（モック A-PianoRoll の .hd）。 */
  .hd {
    width: 100%;
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
  }
  /* たたんだ「使えない」見出し: 文字 gray.7、押せない。 */
  .hd.folded {
    color: var(--color-gray-7);
    cursor: default;
  }
  /* インスペクタの行（モック A-PianoRoll の .rw。左は字下げ）。 */
  .rw {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3) 0 var(--mock-inspector-indent);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    white-space: nowrap;
  }
  .value {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* 使えない（仕様書 2 章）: 文字 gray.7。区切り2 で動かす。 */
  .rw.off {
    color: var(--color-gray-7);
  }
  .pick:hover {
    text-decoration: underline;
  }
  .pick.on {
    color: var(--color-accent);
  }

  /* つまみの棒（モック A-PianoRoll: 線 line.hair、ぬり 3、つまみ 6×12）。使えないあいだは線 gray.5、ぬりとつまみ gray.7。 */
  .slider-label {
    width: var(--mock-inspector-slider-label);
    flex-shrink: 0;
  }
  .slider {
    flex: 1;
    min-width: 0;
    position: relative;
    height: var(--part-param-knob-h);
  }
  .track {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(var(--part-param-knob-h) / 2);
    height: var(--line-hair);
    background: var(--color-gray-5);
  }
  .fill {
    position: absolute;
    left: 0;
    top: calc((var(--part-param-knob-h) - var(--mock-inspector-slider-fill)) / 2);
    height: var(--mock-inspector-slider-fill);
    background: var(--color-gray-7);
  }
  .knob {
    position: absolute;
    top: 0;
    width: var(--part-param-knob-w);
    height: var(--part-param-knob-h);
    margin-left: calc(var(--part-param-knob-w) / -2);
    background: var(--color-gray-7);
  }
  .slider-value {
    width: var(--mock-inspector-slider-value);
    flex-shrink: 0;
    text-align: right;
  }

  .buttons {
    flex-shrink: 0;
    display: flex;
    gap: var(--space-2);
    padding: var(--mock-inspector-buttons-pad-y) var(--space-3) var(--mock-inspector-buttons-pad-y) var(--mock-inspector-indent);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
  }

  /* 重なり: 状態行の「拡大率」の一覧と同じ（ダイアログ 30 より上、用語の説明 1000 より下）。 */
  .popup {
    position: fixed;
    z-index: 40;
    display: flex;
    flex-direction: column;
  }
</style>
