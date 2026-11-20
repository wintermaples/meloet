<!-- ピアノロールの右の「選んだノート」の中身（モック A-PianoRoll・F-WidePR の右の欄: 選んだノート、クオンタイズ、スケール、長さをそろえる、
     ベロシティをまとめて変える）。見出し（▾ 選んだノート 3）とたたんだ縦の見出しは、帯の部品（PianoRoll.svelte）が描く。これは、ひらいたときの中身だけを、親の箱いっぱいに描く。
     選んだノートの 位置 ・ 音 ・ 長さ ・ ベロシティ は、押すと「入力の枠」（ノートの一覧と同じ。決めると選んだぜんぶへ setNotesField）。
     クオンタイズの設定は、変えるたびに setQuantize（いまの設定ぜんぶ）。「かける」（かけたあとは「かけなおす」）は quantize、
     「もとの位置へ戻す」は unquantize（選んだノート、なければ全部）。スケールは setMusicalScale。
     「長さをそろえる」と「ベロシティをまとめて変える」は、ひらくと「まとめて変える行」が 1 行（仕様書 2 章）。Enter かボタンで matchLength ・ matchVelocity。
     欄の数は、範囲の外なら近いはしにして決める（B-68）。
     ドラムのトラックでは、音は楽器の名前、スケールは音階「—」・固定「使えない」（仕様書 3 章、モック R7-Drum）。 -->
<script lang="ts">
  import { mapOf, meterAtOf } from "../../time/bars";
  import type { Command, QuantizeAlign, QuantizeSettings, ScaleMode } from "../../bridge";
  import { menuOpened } from "../../lib/menu";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { term } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import ChoiceMenu from "../ChoiceMenu.svelte";
  import { gridText, scaleName } from "../format";
  import { keyLabel, rollShowsDrums, selectedKeysText } from "./drums";
  import { GRID_DIVISIONS, indexesOf } from "./geom";
  import NoteFieldInput from "./NoteFieldInput.svelte";
  import {
    clampCommand,
    commitCommand,
    nextField,
    noteFieldSpec,
    readField,
    rejectCommand,
    stepField,
    type FieldName,
    type FieldRead,
  } from "./noteFields";
  import { selectedSummary } from "./notes";
  import PercentSlider from "./PercentSlider.svelte";
  import ScaleMenu from "./ScaleMenu.svelte";
  import { shown } from "./shown.svelte";
  import { inspectorFolds } from "./view.svelte";

  const app = $derived(current.state!);
  const pr = $derived(app.pianoRoll);
  /** 位置は拍子の変更をたどって数える（長さは、その所の拍子の拍で）。 */
  const meter = $derived(mapOf(app.timeline));
  const take = $derived(shown.take);
  /** 見せているテイクが Rust の pianoRoll のテイクと同じ（意図を送ってよい）。 */
  const live = $derived(take !== null && shown.clip !== null && pr.take === take.id);
  const drums = $derived(rollShowsDrums(app, shown.clip, shown.track));
  const summary = $derived(live ? selectedSummary(take!, shown.clip!, pr.selectedNotes, meter, 64, (k) => keyLabel(k, drums)) : null);
  const count = $derived(summary?.count ?? 0);
  /** 選んだノートの鍵（同じ名前をまとめて出す。仕様書 3 章 85）。 */
  const selectedKeys = $derived(live ? indexesOf(pr.selectedNotes).filter((i) => i < take!.keys.length).map((i) => take!.keys[i]!) : []);
  const selected = $derived(live ? indexesOf(pr.selectedNotes).filter((i) => i < take!.starts.length) : []);
  const quantized = $derived((take?.origin ?? 0) !== 0);
  const q = $derived(pr.quantize);

  const velocity = $derived(
    summary === null
      ? ""
      : summary.velocityLow === summary.velocityHigh
        ? String(summary.velocityLow)
        : t("inspector.range", { low: summary.velocityLow, high: summary.velocityHigh }),
  );

  /** 欄に見えている文字（使えない値を打ったときの「もとの … のまま」に使う）。 */
  const fieldText = $derived<Record<FieldName, string>>({
    position: summary ? (count > 1 ? t("inspector.from", { position: summary.position }) : summary.position) : "",
    key: summary ? selectedKeysText(selectedKeys, drums) : "",
    length: summary ? (summary.length ?? t("inspector.mixed")) : "",
    velocity,
  });

  function setScale(root: number, mode: ScaleMode, lock: boolean): void {
    send({ type: "setMusicalScale", root, mode, lock });
  }

  // ---- 選んだノートの欄の打ちこみ ----

  const LABELS: Record<FieldName, string> = {
    position: t("note.position"),
    key: t("note.key"),
    length: t("note.length"),
    velocity: t("pr.velocity"),
  };
  const TESTS: Record<FieldName, string> = {
    position: "inspector-position",
    key: "inspector-keys",
    length: "inspector-length",
    velocity: "inspector-velocity",
  };

  let editing = $state<{ name: FieldName; draft: string; token: number } | null>(null);
  let tokens = 0;
  const fieldButtons: Partial<Record<FieldName, HTMLButtonElement>> = $state({});
  const specOf = (name: FieldName) => (live && selected.length > 0 ? noteFieldSpec(name, take!, shown.clip!, selected, meter, drums) : null);
  const editSpec = $derived(editing ? specOf(editing.name) : null);

  function openField(name: FieldName): void {
    const spec = specOf(name);
    if (spec) editing = { name, draft: spec.initial, token: ++tokens };
  }

  function closeField(refocus: boolean): void {
    const name = editing?.name;
    editing = null;
    if (refocus && name) queueMicrotask(() => fieldButtons[name]?.focus());
  }

  function finishField(value: number, tab: -1 | 0 | 1, clamped: FieldRead["clamped"]): void {
    const e = editing;
    const spec = editSpec;
    if (!e || !spec) return;
    const command = commitCommand(spec, value, take!.id, selected);
    if (command) send(command);
    if (clamped) send(clampCommand(clamped));
    const next = tab === 0 ? null : nextField(e.name, tab < 0);
    if (next && next.row === 0) openField(next.name);
    else closeField(true);
  }

  function rejectField(): void {
    if (editing && editSpec) send(rejectCommand(editSpec, fieldText[editing.name]));
  }

  // ---- 長さをそろえる ・ ベロシティをまとめて変える（まとめて変える行） ----

  type Bulk = "length" | "velocity";
  const bulkSpecs = $derived({ length: specOf("length"), velocity: specOf("velocity") });
  /** 選んだノートがみな同じ値ならその値、ちがえば空。 */
  const bulkInitial = (name: Bulk) => {
    const spec = bulkSpecs[name];
    return spec?.current != null ? spec.format(spec.current) : "";
  };
  const bulkDrafts = $state<Record<Bulk, string>>({ length: "", velocity: "" });
  $effect(() => {
    bulkDrafts.length = bulkInitial("length");
  });
  $effect(() => {
    bulkDrafts.velocity = bulkInitial("velocity");
  });
  const bulkBroken = (name: Bulk) => {
    const spec = bulkSpecs[name];
    return spec !== null && bulkDrafts[name].trim() !== "" && readField(spec, bulkDrafts[name]) === null;
  };

  function matchBulk(name: Bulk): void {
    const spec = bulkSpecs[name];
    if (!spec || !live) return;
    const r = readField(spec, bulkDrafts[name]);
    if (r === null) {
      send(rejectCommand(spec, fieldText[name]));
      return;
    }
    if (name === "length") send({ type: "matchLength", take: take!.id, notes: selected, value: r.value });
    else send({ type: "matchVelocity", take: take!.id, notes: selected, value: r.value });
    if (r.clamped) send(clampCommand(r.clamped));
    bulkDrafts[name] = spec.format(r.value);
  }

  function bulkKey(ev: KeyboardEvent, name: Bulk): void {
    if (ev.isComposing || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    const spec = bulkSpecs[name];
    if (ev.key === "Enter" && !ev.shiftKey) {
      matchBulk(name);
    } else if (ev.key === "Escape" && !ev.shiftKey) {
      bulkDrafts[name] = bulkInitial(name);
      (ev.currentTarget as HTMLInputElement).blur();
    } else if ((ev.key === "ArrowUp" || ev.key === "ArrowDown") && spec) {
      const from = readField(spec, bulkDrafts[name])?.value ?? spec.current ?? readField(spec, spec.initial)?.value ?? spec.low;
      bulkDrafts[name] = spec.format(stepField(spec, from, (ev.key === "ArrowUp" ? 1 : -1) * (ev.shiftKey ? spec.shift : 1)));
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }

  // ---- クオンタイズ ----

  function setQuantize(change: Partial<QuantizeSettings>): void {
    const next = { ...q, ...change };
    send({ type: "setQuantize", ...next } satisfies Command);
  }
  /** つまみの棒を動かしている行（仕様書 2 章「パラメータの行」の「動かしている」）。 */
  const moving = $state({ strength: false, swing: false });

  // 「かけなおす」の右はしのキー Q（B-73）。横が足りなければ出さない。
  let buttonsRow = $state<HTMLElement>();
  let quantizeKey = $state(true);
  let keyWidth = 0;
  function fitKey(): void {
    const row = buttonsRow;
    if (!row) return;
    if (quantizeKey) {
      const key = row.querySelector<HTMLElement>(".key");
      keyWidth = key ? key.offsetWidth + parseFloat(getComputedStyle(key.parentElement!).columnGap || "0") : 0;
      if (row.scrollWidth > row.clientWidth + 1) quantizeKey = false;
    } else if (row.scrollWidth + keyWidth <= row.clientWidth) {
      quantizeKey = true;
    }
  }
  $effect(() => {
    const row = buttonsRow;
    if (!row) return;
    const observer = new ResizeObserver(fitKey);
    observer.observe(row);
    fitKey();
    return () => observer.disconnect();
  });

  const canQuantize = $derived(live && take!.starts.length > 0);
  const canUnquantize = $derived(live && quantized);
  const quantize = () => canQuantize && send({ type: "quantize", take: take!.id, notes: selected });
  const unquantize = () => canUnquantize && send({ type: "unquantize", take: take!.id, notes: selected });

  const gridRows = GRID_DIVISIONS.map((d) => ({ value: d as number, label: gridText(d, false) }));
  const alignRows: { value: QuantizeAlign; label: string }[] = [
    { value: "start", label: t("inspector.alignStart") },
    { value: "startAndEnd", label: t("inspector.alignStartAndEnd") },
  ];

  // ---- 一覧（音階 ・ クオンタイズのグリッド ・ そろえる所） ----

  type Menu = "scale" | "grid" | "align";
  let open = $state<Menu | null>(null);
  const menuButtons: Partial<Record<Menu, HTMLButtonElement>> = $state({});
  let popup = $state<HTMLElement>();
  let place = $state<{ right: number; top: number | null; bottom: number | null; maxHeight: number }>({
    right: 0,
    top: null,
    bottom: null,
    maxHeight: 0,
  });

  /** 押した行のすぐ下か上（広いほう）にひらく。右はしは行にそろえる。 */
  function placeMenu(): void {
    const row = open ? menuButtons[open]?.closest(".rw") : null;
    if (!row) return;
    const r = row.getBoundingClientRect();
    const below = window.innerHeight - r.bottom;
    const above = r.top;
    place =
      below >= above
        ? { right: window.innerWidth - r.right, top: r.bottom, bottom: null, maxHeight: below }
        : { right: window.innerWidth - r.right, top: null, bottom: window.innerHeight - r.top, maxHeight: above };
  }

  function closeMenu(): void {
    const button = open ? menuButtons[open] : undefined;
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    open = null;
    if (hadFocus) button?.focus();
  }

  const toggleMenu = (menu: Menu) => (open = open === menu ? null : menu);

  function pickScale(root: number, mode: ScaleMode): void {
    setScale(root, mode, pr.scale.lock);
    closeMenu();
  }

  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  $effect(() => {
    if (!open) return;
    const button = menuButtons[open];
    placeMenu();
    window.addEventListener("resize", placeMenu);
    const forget = menuOpened(() => closeMenu());
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || button?.contains(target))) return;
      closeMenu();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", placeMenu);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });
</script>

{#snippet field(name: FieldName, termName: string | null)}
  <div class="rw">
    {#if termName}
      <span class="m-lbl grow" use:term={termName}>{LABELS[name]}</span>
    {:else}
      <span class="m-lbl grow">{LABELS[name]}</span>
    {/if}
    {#if editing && editSpec && editing.name === name}
      {@const e = editing}
      <span class="input">
        <NoteFieldInput
          spec={editSpec}
          bind:draft={e.draft}
          label={LABELS[name]}
          test={TESTS[name]}
          right={name !== "key"}
          onfinish={finishField}
          oncancel={() => editing?.token === e.token && closeField(true)}
          onreject={rejectField}
        />
      </span>
    {:else}
      <button
        class="m-plain m-hi value"
        class:m-num={!(drums && name === "key")}
        class:pick={summary !== null}
        disabled={summary === null}
        aria-label={LABELS[name]}
        data-test={TESTS[name]}
        bind:this={fieldButtons[name]}
        onclick={() => openField(name)}>{fieldText[name]}</button
      >
    {/if}
  </div>
{/snippet}

{#snippet slider(label: string, termName: string | null, value: number, test: string, key: "strength" | "swing")}
  <div class="rw slider-row" class:moving={moving[key]}>
    {#if termName}
      <span class="m-lbl slider-label" use:term={termName}>{label}</span>
    {:else}
      <span class="m-lbl slider-label">{label}</span>
    {/if}
    <PercentSlider {value} {label} {test} onchange={(v) => setQuantize({ [key]: v })} onmoving={(on) => (moving[key] = on)} />
  </div>
{/snippet}

{#snippet bulkHead(fold: "sameLength" | "velocityAll", label: string, test: string)}
  <button class="m-plain hd m-focus-in" aria-expanded={inspectorFolds[fold]} data-test={test} onclick={() => (inspectorFolds[fold] = !inspectorFolds[fold])}>
    <span class={inspectorFolds[fold] ? "m-hi" : "m-lbl"}>{inspectorFolds[fold] ? "▾" : "▸"}</span>
    <span class="m-hi grow">{label}</span>
  </button>
{/snippet}

{#snippet bulkRow(name: Bulk, label: string, termName: string | null, fieldTest: string, buttonTest: string)}
  {@const usable = bulkSpecs[name] !== null}
  {@const broken = bulkBroken(name)}
  <div class="bulk-row">
    {#if termName}
      <span class="m-lbl bulk-label" class:off={!usable} use:term={termName}>{label}</span>
    {:else}
      <span class="m-lbl bulk-label" class:off={!usable}>{label}</span>
    {/if}
    <input
      class="bulk-field m-num"
      class:broken
      type="text"
      spellcheck="false"
      autocomplete="off"
      aria-label={label}
      aria-invalid={broken}
      disabled={!usable}
      data-test={fieldTest}
      bind:value={bulkDrafts[name]}
      onkeydown={(ev) => bulkKey(ev, name)}
    />
    <button class="m-btn" aria-disabled={!usable} data-test={buttonTest} onclick={() => matchBulk(name)}>{t("inspector.matchVelocity")}</button>
  </div>
{/snippet}

<div class="inspector" data-testid="pr-inspector">
  <ScrollArea>
    {@render field("position", null)}
    {@render field("key", null)}
    {@render field("length", null)}
    {@render field("velocity", "ベロシティ")}

    <div class="hd">
      <span class="m-hi">▾</span>
      <span class="m-hi grow" use:term={"クオンタイズ"}>{t("inspector.quantize")}</span>
      {#if quantized}<span class="m-lbl" data-test="inspector-quantized">{t("inspector.quantized")}</span>{/if}
    </div>
    <div class="rw">
      <span class="m-lbl grow" use:term={"グリッド"}>{t("pr.grid")}</span>
      <button
        class="m-plain m-num m-hi pick"
        aria-haspopup="menu"
        aria-expanded={open === "grid"}
        data-test="quantize-grid"
        bind:this={menuButtons.grid}
        onclick={() => toggleMenu("grid")}>{gridText(q.division, false)}</button
      >
      <button
        class="m-plain pick toggle"
        class:on={q.triplet}
        aria-pressed={q.triplet}
        use:term={"三連符"}
        data-test="quantize-triplet"
        onclick={() => setQuantize({ triplet: !q.triplet })}>{t("grid.triplet")}</button
      >
    </div>
    {@render slider(t("inspector.strength"), null, q.strength, "quantize-strength", "strength")}
    {@render slider(t("inspector.swing"), "スウィング", q.swing, "quantize-swing", "swing")}
    <div class="rw">
      <span class="m-lbl grow">{t("inspector.align")}</span>
      <button
        class="m-plain m-hi pick"
        aria-haspopup="menu"
        aria-expanded={open === "align"}
        data-test="quantize-align"
        bind:this={menuButtons.align}
        onclick={() => toggleMenu("align")}>{t(q.align === "start" ? "inspector.alignStart" : "inspector.alignStartAndEnd")}</button
      >
    </div>
    <div class="buttons" bind:this={buttonsRow}>
      <button class="m-btn" class:hi={canQuantize} aria-disabled={!canQuantize} data-test="quantize-apply" onclick={quantize}
        >{t(quantized ? "inspector.requantize" : "inspector.quantizeApply")}{#if quantizeKey}<span class="m-num m-lbl key" data-test="quantize-key">{t("key.quantize")}</span>{/if}</button
      >
      <button class="m-btn" aria-disabled={!canUnquantize} data-test="quantize-restore" onclick={unquantize}>{t("inspector.unquantize")}</button>
    </div>

    <div class="hd">
      <span class="m-hi">▾</span>
      <span class="m-hi grow">{t("inspector.scale")}</span>
    </div>
    {#if drums}
      <div class="rw">
        <span class="m-lbl grow">{t("inspector.musicalScale")}</span>
        <span class="m-num m-lbl" data-test="inspector-scale">{t("inspector.scaleNone")}</span>
      </div>
      <div class="rw">
        <span class="m-lbl grow" use:term={"スケール固定"}>{t("inspector.lock")}</span>
        <span class="m-lbl" data-test="inspector-lock">{t("inspector.lockUnusable")}</span>
      </div>
    {:else}
      <div class="rw">
        <span class="m-lbl grow">{t("inspector.musicalScale")}</span>
        <button
          class="m-plain m-num m-hi pick"
          aria-haspopup="menu"
          aria-expanded={open === "scale"}
          data-test="inspector-scale"
          bind:this={menuButtons.scale}
          onclick={() => toggleMenu("scale")}>{scaleName(pr.scale.root, pr.scale.mode)}</button
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
    {/if}

    {@render bulkHead("sameLength", t("inspector.sameLength"), "inspector-same-length")}
    {#if inspectorFolds.sameLength}
      {@render bulkRow("length", t("note.length"), null, "inspector-same-length-field", "inspector-match-length")}
    {/if}
    {@render bulkHead("velocityAll", t("inspector.velocityAll"), "inspector-velocity-all")}
    {#if inspectorFolds.velocityAll}
      {@render bulkRow("velocity", t("pr.velocity"), "ベロシティ", "inspector-velocity-all-field", "inspector-match-velocity")}
    {/if}
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
    {#if open === "scale"}
      <ScaleMenu scale={pr.scale} onpick={pickScale} />
    {:else if open === "grid"}
      <ChoiceMenu
        heading={t("pr.grid")}
        rows={gridRows}
        selected={q.division}
        onpick={(division) => {
          setQuantize({ division });
          closeMenu();
        }}
        test="quantize-grid-menu"
      />
    {:else}
      <ChoiceMenu
        heading={t("inspector.align")}
        rows={alignRows}
        selected={q.align}
        onpick={(align) => {
          setQuantize({ align });
          closeMenu();
        }}
        test="quantize-align-menu"
      />
    {/if}
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
  .pick:hover {
    text-decoration: underline;
  }
  .pick.on {
    color: var(--color-accent);
  }
  .pick:disabled {
    cursor: default;
  }
  .off {
    color: var(--color-gray-7);
  }
  /* まとめて変える行（仕様書 2 章、モック A-PianoRoll）。 */
  .bulk-row {
    height: var(--mock-inspector-bulk-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3) 0 var(--mock-inspector-indent);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    white-space: nowrap;
  }
  .bulk-label {
    width: var(--mock-inspector-bulk-label);
    flex-shrink: 0;
    overflow: hidden;
  }
  /* 入力の枠（仕様書 2 章「入力の枠」: 地 gray.0、枠 gray.6、乗った gray.8、使えない 枠 gray.5 ・ 文字 gray.7、こわれた rec。
     打っている間は、ほかの入力の枠と同じ gray.10）。 */
  .bulk-field {
    box-sizing: border-box;
    width: var(--mock-inspector-bulk-field);
    height: var(--mock-inspector-bulk-field-h);
    flex-shrink: 0;
    padding: 0 var(--mock-inspector-bulk-field-pad);
    margin: 0;
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-0);
    color: var(--color-gray-10);
    font-size: inherit;
    line-height: inherit;
    text-align: right;
    outline: none;
  }
  .bulk-field:hover {
    border-color: var(--color-gray-8);
  }
  .bulk-field:focus {
    border-color: var(--color-gray-10);
  }
  .bulk-field:disabled {
    border-color: var(--color-gray-5);
    color: var(--color-gray-7);
  }
  .bulk-field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  /* 三連符の切りかえ（道具の列の「三連符」と同じく、グリッドの右に別の切りかえ）: オフは gray.8、オンは accent。 */
  .toggle {
    color: var(--color-gray-8);
  }
  /* 打っている欄: 入力の枠が行の高さいっぱい、幅は欄の右半分。 */
  .input {
    width: 50%;
    height: 100%;
  }
  .m-btn.hi:not([aria-disabled="true"]) {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }

  /* つまみの棒の行: 乗った（枠 gray.8 ・ 文字 gray.10）、動かしている（行の地 gray.4、左 line.mark の accent）。枠と印は内がわに重ねる。 */
  .slider-row:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .slider-row:hover .slider-label {
    color: var(--color-gray-10);
  }
  .slider-row.moving,
  .slider-row.moving:hover {
    background: var(--color-gray-4);
    box-shadow: inset var(--line-mark) 0 0 0 var(--color-accent);
  }
  .key {
    color: var(--color-gray-8);
  }

  /* つまみの棒の行のラベル（モック A-PianoRoll: 60）。 */
  .slider-label {
    width: var(--mock-inspector-slider-label);
    flex-shrink: 0;
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
