<!-- The content of 「選んだノート」 on the right of the piano roll (the right panel of mock A-PianoRoll, F-WidePR: selected notes, quantize, scale, align length,
     change velocity together). The heading (「▾ 選んだノート 3」) and the folded vertical heading are drawn by the band part (PianoRoll.svelte). This draws only the open content, filling the parent box.
     The position, note, length and velocity of the selected notes become an 「入力の枠」 when pressed (the same as the note list; deciding sends setNotesField to all selected).
     Each change to the quantize settings sends setQuantize (all current settings). 「かける」 (「かけなおす」 after applying) sends quantize,
     「もとの位置へ戻す」 sends unquantize (the selected notes, or all if none). Scale sends setMusicalScale.
     「長さをそろえる」 and 「ベロシティをまとめて変える」, when opened, give one 「まとめて変える行」 row (spec 2). Enter or the button sends matchLength / matchVelocity.
     Numbers in fields out of range are set to the nearest end and decided (B-68).
     On a drum track, the note is the drum piece name, and the scale is 「—」 with lock 「使えない」 (spec 3, mock R7-Drum). -->
<script lang="ts">
  import { mapOf, meterAtOf } from "../../time/bars";
  import type { Command, QuantizeAlign, QuantizeSettings, ScaleMode } from "../../bridge";
  import { menuOpened } from "../../lib/menu";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { term, tip } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import ChoiceMenu from "../ChoiceMenu.svelte";
  import { gridText, scaleName } from "../format";
  import { keyLabel, rollShowsDrums, selectedKeysFull, selectedKeysText } from "./drums";
  import { newSeed, readTool, stepTool, TOOL_INPUTS, toolCommand, toolInitial, type Tool, type ToolField } from "./bulkTools";
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
  /** Positions are counted following time signature changes (lengths in beats of the time signature at that place). */
  const meter = $derived(mapOf(app.timeline));
  const take = $derived(shown.take);
  /** The shown take is the same as the take of Rust's pianoRoll (intents may be sent). */
  const live = $derived(take !== null && shown.clip !== null && pr.take === take.id);
  const drums = $derived(rollShowsDrums(app, shown.clip, shown.track));
  const summary = $derived(live ? selectedSummary(take!, shown.clip!, pr.selectedNotes, meter, 64, (k) => keyLabel(k, drums)) : null);
  const count = $derived(summary?.count ?? 0);
  /** The keys of the selected notes (the same names grouped; spec 3 (85)). */
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

  /** The text seen in the field (used for 「もとの … のまま」 when an unusable value is typed). */
  const fieldText = $derived<Record<FieldName, string>>({
    position: summary ? (count > 1 ? t("inspector.from", { position: summary.position }) : summary.position) : "",
    key: summary ? selectedKeysText(selectedKeys, drums) : "",
    length: summary ? (summary.length ?? t("inspector.mixed")) : "",
    velocity,
  });
  const keysFull = $derived(summary ? selectedKeysFull(selectedKeys, drums) : null);

  function setScale(root: number, mode: ScaleMode, lock: boolean, highlight = pr.scale.highlight): void {
    send({ type: "setMusicalScale", root, mode, lock, highlight });
  }

  // ---- Typing in the selected notes' fields ----

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

  // ---- Align length / change velocity together (the change-all row) ----

  type Bulk = "length" | "velocity";
  const bulkSpecs = $derived({ length: specOf("length"), velocity: specOf("velocity") });
  /** The value if all selected notes have the same value, otherwise empty. */
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

  // ---- 「割合」 「だんだん」 「ばらつき」 and 「次の音までのばす」 (P3) ----

  const toolUsable = $derived(live && selected.length > 0);
  const toolDrafts = $state<Record<ToolField, string>>({
    percent: toolInitial("percent"),
    rampFrom: toolInitial("rampFrom"),
    rampTo: toolInitial("rampTo"),
    spread: toolInitial("spread"),
  });
  /** The values last used, for 「もとの … のまま」 and Esc. */
  const toolKept: Record<ToolField, string> = {
    percent: toolInitial("percent"),
    rampFrom: toolInitial("rampFrom"),
    rampTo: toolInitial("rampTo"),
    spread: toolInitial("spread"),
  };
  const toolBroken = (f: ToolField) => toolDrafts[f].trim() !== "" && readTool(f, toolDrafts[f]) === null;

  function applyTool(tool: Tool): void {
    if (!toolUsable) return;
    const fields = TOOL_INPUTS[tool];
    const reads = fields.map((f) => readTool(f, toolDrafts[f]));
    const bad = fields.findIndex((_, i) => reads[i] === null);
    if (bad >= 0) {
      // Only the velocity fields have words on the status line; the others stay red.
      if (tool === "ramp") send({ type: "rejectValue", field: "velocity", kept: toolKept[fields[bad]!], range: null });
      return;
    }
    const values = reads.map((r) => r!.value);
    send(toolCommand(tool, take!.id, selected, values, newSeed()));
    const clamped = reads.find((r) => r!.clamped)?.clamped;
    if (clamped) send(clampCommand(clamped));
    fields.forEach((f, i) => (toolDrafts[f] = toolKept[f] = String(values[i])));
  }

  function toolKey(ev: KeyboardEvent, f: ToolField, tool: Tool): void {
    if (ev.isComposing || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key === "Enter" && !ev.shiftKey) {
      applyTool(tool);
    } else if (ev.key === "Escape" && !ev.shiftKey) {
      toolDrafts[f] = toolKept[f];
      (ev.currentTarget as HTMLInputElement).blur();
    } else if (ev.key === "ArrowUp" || ev.key === "ArrowDown") {
      toolDrafts[f] = stepTool(f, toolDrafts[f], (ev.key === "ArrowUp" ? 1 : -1) * (ev.shiftKey ? 10 : 1));
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }

  const legato = () => toolUsable && send({ type: "legato", take: take!.id, notes: selected });

  // ---- Quantize ----

  function setQuantize(change: Partial<QuantizeSettings>): void {
    const next = { ...q, ...change };
    send({ type: "setQuantize", ...next } satisfies Command);
  }
  /** The row whose slider is being moved (the "moving" state of spec 2 "Parameter row"). */
  const moving = $state({ strength: false, swing: false });

  // The key Q at the right end of 「かけなおす」 (B-73). Not shown if there is not enough width.
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

  // ---- Lists (scale, quantize grid, what to align) ----

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

  /** Opens just below or above the pressed row (whichever has more room). The right edge lines up with the row. */
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
        use:tip={name === "key" ? keysFull : null}
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

{#snippet toolInput(f: ToolField, label: string, test: string, tool: Tool)}
  {@const broken = toolBroken(f)}
  <input
    class="bulk-field m-num part"
    class:broken
    type="text"
    spellcheck="false"
    autocomplete="off"
    aria-label={label}
    aria-invalid={broken}
    disabled={!toolUsable}
    data-test={test}
    bind:value={toolDrafts[f]}
    onkeydown={(ev) => toolKey(ev, f, tool)}
  />
{/snippet}

{#snippet toolRow(tool: Tool, label: string, test: string)}
  <div class="bulk-row">
    <span class="m-lbl bulk-label" class:off={!toolUsable}>{label}</span>
    <span class="tool-box">
      {#if tool === "percent"}
        {@render toolInput("percent", label, "inspector-percent-field", tool)}
        <span class="m-lbl">{t("inspector.percentMark")}</span>
      {:else if tool === "ramp"}
        {@render toolInput("rampFrom", label, "inspector-ramp-from-field", tool)}
        <span class="m-lbl">{t("inspector.rampMark")}</span>
        {@render toolInput("rampTo", label, "inspector-ramp-to-field", tool)}
      {:else}
        <span class="m-lbl">{t("inspector.spreadMark")}</span>
        {@render toolInput("spread", label, "inspector-spread-field", tool)}
      {/if}
    </span>
    <button class="m-btn" aria-disabled={!toolUsable} data-test={test} onclick={() => applyTool(tool)}>{t("inspector.applyTool")}</button>
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
      <div class="rw">
        <span class="m-lbl grow">{t("inspector.highlight")}</span>
        <span class="m-lbl" data-test="inspector-highlight">{t("inspector.lockUnusable")}</span>
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
      <div class="rw">
        <span class="m-lbl grow">{t("inspector.highlight")}</span>
        <button
          class="m-plain m-num m-hi pick"
          class:on={pr.scale.highlight}
          aria-pressed={pr.scale.highlight}
          data-test="inspector-highlight"
          onclick={() => setScale(pr.scale.root, pr.scale.mode, pr.scale.lock, !pr.scale.highlight)}
          >{t(pr.scale.highlight ? "inspector.on" : "inspector.off")}</button
        >
      </div>
    {/if}

    {@render bulkHead("sameLength", t("inspector.sameLength"), "inspector-same-length")}
    {#if inspectorFolds.sameLength}
      {@render bulkRow("length", t("note.length"), null, "inspector-same-length-field", "inspector-match-length")}
      <div class="bulk-row">
        <span class="m-lbl grow" class:off={!toolUsable}>{t("inspector.legato")}</span>
        <button class="m-btn" aria-disabled={!toolUsable} data-test="inspector-legato" onclick={legato}>{t("inspector.legatoApply")}</button>
      </div>
    {/if}
    {@render bulkHead("velocityAll", t("inspector.velocityAll"), "inspector-velocity-all")}
    {#if inspectorFolds.velocityAll}
      {@render bulkRow("velocity", t("pr.velocity"), "ベロシティ", "inspector-velocity-all-field", "inspector-match-velocity")}
      {@render toolRow("percent", t("inspector.scaleVelocity"), "inspector-scale-velocity")}
      {@render toolRow("ramp", t("inspector.rampVelocity"), "inspector-ramp-velocity")}
      {@render toolRow("spread", t("inspector.randomVelocity"), "inspector-random-velocity")}
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
  /* Heading (.hd in mock A-PianoRoll). */
  .hd {
    width: 100%;
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
  }
  /* Inspector rows (.rw in mock A-PianoRoll; the left is indented). */
  .rw {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3) 0 var(--mock-inspector-indent);
    border-bottom: var(--line-hair) solid var(--color-line-row);
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
  /* The change-all row (spec 2, mock A-PianoRoll). */
  .bulk-row {
    height: var(--mock-inspector-bulk-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3) 0 var(--mock-inspector-indent);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    white-space: nowrap;
  }
  .bulk-label {
    width: var(--mock-inspector-bulk-label);
    flex-shrink: 0;
    overflow: hidden;
  }
  /* Input box (spec 2 "Input box": background gray.0, border gray.6, hover gray.8, disabled border gray.5 / text gray.7, broken rec.
     While typing, gray.10 like the other input boxes). */
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
  /* The boxes of 「割合」 「だんだん」 「ばらつき」 with their marks, as wide as one input box. */
  .tool-box {
    width: var(--mock-inspector-bulk-field);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .bulk-field.part {
    width: auto;
    flex: 1;
    min-width: 0;
  }
  /* Two boxes in the width of one: no padding, so that three digits fit. */
  .tool-box:has(.part + * + .part) .part {
    padding: 0;
    text-align: center;
  }
  .bulk-field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  /* The triplet toggle (like 「三連符」 in the toolbar, a separate toggle right of the grid): off is gray.8, on is accent. */
  .toggle {
    color: var(--color-gray-8);
  }
  /* The field being typed in: the input box fills the row height, and the width is the right half of the field. */
  .input {
    width: 50%;
    height: 100%;
  }
  .m-btn.hi:not([aria-disabled="true"]) {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }

  /* Slider rows: hover (border gray.8, text gray.10), moving (row background gray.4, a line.mark accent on the left). The border and mark are laid inside. */
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

  /* The label of slider rows (mock A-PianoRoll: 60). */
  .slider-label {
    width: var(--mock-inspector-slider-label);
    flex-shrink: 0;
  }

  .buttons {
    flex-shrink: 0;
    display: flex;
    gap: var(--space-2);
    padding: var(--mock-inspector-buttons-pad-y) var(--space-3) var(--mock-inspector-buttons-pad-y) var(--mock-inspector-indent);
    border-bottom: var(--line-hair) solid var(--color-line-row);
  }

  /* Stacking: the same as the 「拡大率」 list in the status line (above dialogs at 30, below term tips at 1000). */
  .popup {
    position: fixed;
    z-index: 40;
    display: flex;
    flex-direction: column;
  }
</style>
