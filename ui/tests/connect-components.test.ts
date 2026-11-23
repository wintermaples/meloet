// Mounts and checks the input meter, the output chooser, sound settings and「音が出ました」.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, EndpointView } from "../src/bridge";
import InputMeter from "../src/components/InputMeter.svelte";
import OutputChooser from "../src/components/OutputChooser.svelte";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import SoundHint from "../src/components/SoundHint.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { closeContextMenu, contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import { hideTip, TIP_DELAY_MS, tipView } from "../src/lib/tip.svelte";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

const q = <T extends Element = HTMLElement>(sel: string) => target.querySelector<T & HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (sel: string) => q(sel)?.textContent?.replace(/\s+/g, " ").trim() ?? null;

const KEYBOARD = { id: "kb", name: "鍵盤 49", portName: "鍵盤 49 MIDI 1", address: "24:0" };

const endpoint = (e: Partial<EndpointView> & { id: string; name: string }): EndpointView => ({
  friendly: null,
  availability: "yes",
  link: "jack",
  nodeName: "node",
  device: "Ryzen HD Audio Controller",
  unusable: null,
  channels: null,
  ...e,
});

const OUTPUTS: EndpointView[] = [
  endpoint({ id: "hp", name: "アナログヘッドフォン", friendly: "headphones" }),
  endpoint({ id: "usb", name: "USB オーディオ", link: "plug" }),
  endpoint({ id: "none", name: "うしろの穴", friendly: "lineOut", availability: "no" }),
  endpoint({ id: "x", name: "なにか", availability: "unknown" }),
  endpoint({ id: "line", name: "ライン出力", friendly: "lineOut", availability: "no", unusable: "switchedByPc" }),
  endpoint({ id: "hdmi", name: "HDMI / DisplayPort 2", friendly: "hdmi", link: "plug", unusable: "stoppedByPc", device: "GB206 High Definition Audio Controller" }),
];

describe("input meter", () => {
  it("no keyboard and none unplugged:「鍵盤なし」and the hint to connect. No reading", () => {
    show(InputMeter, app.stateWith(() => {}));
    expect(text('[data-test="keyboard-name"]')).toBe("鍵盤なし");
    expect(text('[data-test="meter-right"]')).toBe(t("msg.keyboardMissing"));
    expect(q('[data-test="held"]')).toBeNull();
    expect(q('[data-gl-slot="keyboard"]')).not.toBeNull();
  });

  it("no keyboard, one unplugged: the text for a lost connection", () => {
    show(InputMeter, app.stateWith((s) => (s.keyboards.lost = "Keystation 49 MK3")));
    expect(text('[data-test="meter-right"]')).toBe(
      "Keystation 49 MK3 とのつながりが切れました。曲はそのままです。さしなおすと、自動で戻ります。",
    );
  });

  it("input from the keyboard stopped: shown the same way as an unplugged keyboard, with the stopped text", () => {
    show(
      InputMeter,
      app.stateWith((s) => (s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: true })),
    );
    expect(text('[data-test="keyboard-name"]')).toBe("鍵盤なし");
    expect(text('[data-test="meter-right"]')).toBe("鍵盤からの入力が止まりました。曲はそのままです。いま、つなぎなおしています。");
    expect(q('[data-test="held"]')).toBeNull();
  });

  it("keyboard there, no output yet: accent「音の出口がまだです…」", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
      }),
    );
    expect(text('[data-test="keyboard-name"]')).toBe("鍵盤 49");
    const right = q('[data-test="meter-right"]')!;
    expect(right.textContent).toBe("音の出口がまだです。下でえらんでください");
    // The second sentence hides when there is not enough width (spec 1, Rules for the scale).
    expect([...right.querySelectorAll(".accent")].map((x) => x.textContent)).toEqual(["音の出口がまだです。", "下でえらんでください"]);
  });

  it("output there: the play target「1 ピアノ ・ グランドピアノ」", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
        s.outputs.chosen = "hp";
      }),
    );
    expect(text('[data-test="meter-right"]')).toBe("弾くと鳴る先 1 ピアノ ・ グランドピアノ");
    // With not enough width, only the instrument part hides (spec 1, Rules for the scale).
    expect(text('[data-test="meter-right"] [data-part="target-instrument"]')).toBe("・ グランドピアノ");
  });

  it("slim: the play target is only「1 ピアノ」, without the instrument name (mock A-PianoRoll, F-History)", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
        s.outputs.chosen = "hp";
        s.workspace.layout.meter = "slim";
      }),
    );
    expect(text('[data-test="meter-right"]')).toBe("弾くと鳴る先 1 ピアノ");
  });

  it("held notes: the meter's keys from low to high. Key on/off changes without waiting for the meter", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
      }),
    );
    expect(text('[data-test="held"]')).toBe("— — —");
    app.setTelemetry(app.telemetryWith({ keys: [67, 60, 64], lastVelocity: 87, sustain: 127, modulation: 12 }));
    flushSync();
    expect(text('[data-test="held"]')).toBe("C4 E4 G4");
    expect(text('[data-test="sustain"]')).toBe("ON 127");
    expect(text('[data-test="modulation"]')).toBe("モジュレーション 012");
    expect(text('[data-test="pitch-bend"]')).toBe("ピッチベンド +0000");

    app.emit({ type: "note", on: true, note: 72, velocity: 90, tUs: 0 });
    flushSync();
    expect(text('[data-test="held"]')).toBe("C4 E4 G4 C5");
    app.emit({ type: "note", on: false, note: 60, velocity: 0, tUs: 0 });
    flushSync();
    expect(text('[data-test="held"]')).toBe("E4 G4 C5");
  });

  it("sustain is ON from 64. Slim shows only ON, OFF", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
        s.workspace.layout.meter = "slim";
      }),
    );
    app.setTelemetry(app.telemetryWith({ sustain: 63 }));
    flushSync();
    expect(text('[data-test="sustain"]')).toBe("OFF");
    app.setTelemetry(app.telemetryWith({ sustain: 64 }));
    flushSync();
    expect(text('[data-test="sustain"]')).toBe("ON");
    expect(q('[data-gl-slot="keyboard"]')).not.toBeNull();
    expect(q('[data-gl-slot="velocity"]')).toBeNull();
  });

  it("two keyboards: the name is the last played keyboard, followed by「+1」", () => {
    const second = { ...KEYBOARD, id: "kb2", name: "microKEY2-37", address: "25:0" };
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD, second], active: "kb2", lost: null, inputStopped: false };
      }),
    );
    expect(text('[data-test="keyboard-name"]')).toBe("microKEY2-37");
    expect(q('[data-test="other-keyboards"]')!.textContent).toBe(" +1");
    vi.useFakeTimers();
    q('[data-test="other-keyboards"]')!.dispatchEvent(new Event("pointerenter"));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    vi.useRealTimers();
    expect(tipView.value?.body).toBe("鍵盤 49 ・ microKEY2-37");
    hideTip();
    app.setState(app.stateWith((s) => (s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false })));
    flushSync();
    expect(q('[data-test="other-keyboards"]')).toBeNull();
  });

  it("full: name field 108 up to 8 letters, 148 above", () => {
    const long = { ...KEYBOARD, name: "Keystation 49 MK3" };
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [long], active: "kb", lost: null, inputStopped: false };
      }),
    );
    expect(q(".meter > .who")!.style.width).toBe("var(--part-meter-name-long)");
    app.setState(
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
      }),
    );
    flushSync();
    expect(q(".meter > .who")!.style.width).toBe("var(--size-meter-name)");
  });
});

describe("output chooser", () => {
  const withChooser = () =>
    app.stateWith((s) => {
      s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
      s.outputs.list = OUTPUTS;
      s.workspace.chooser = { highlighted: "hp" };
    });

  it("not shown without chooser", () => {
    show(OutputChooser, app.stateWith(() => {}));
    expect(q('[data-test="output-chooser"]')).toBeNull();
  });

  it("heading and the content of each row", () => {
    show(OutputChooser, withChooser());
    expect(target.textContent).toContain("音をどこから鳴らしますか");
    expect(target.textContent).toContain("あとから「音の設定」で変えられます");
    const rows = qa('[data-test="endpoint-row"]');
    expect(rows).toHaveLength(4);
    expect(rows[0]!.textContent).toContain("アナログヘッドフォン");
    expect(rows[0]!.textContent).toContain("ヘッドホンの穴");
    expect(rows[0]!.textContent).toContain("ささっています");
    expect(rows[0]!.dataset.selected).toBe("true");
    expect(rows[1]!.textContent).toContain("つながっています");
    expect(rows[2]!.textContent).toContain("何もささっていません");
    expect(rows[3]!.querySelector(".link")).toBeNull();
    expect(rows.every((r) => r.dataset.disabled === "false")).toBe(true);
    expect(text(".help")).toBe("↑ ↓ でえらぶ ・ ▶ でテスト音。何かささっている出口を、上にならべています");
  });

  it("outputs that cannot be chosen start folded under「いまは使えない出口」. Opening shows the reason. Cannot be pressed", () => {
    show(OutputChooser, withChooser());
    const head = q<HTMLButtonElement>('[data-test="unusable-head"]')!;
    expect(head.textContent!.replace(/\s+/g, " ").trim()).toBe(
      "▸ いまは使えない出口 2 PC が切りかえる物 1 ・ PC の設定で止まっている物 1",
    );
    expect(head.getAttribute("aria-expanded")).toBe("false");
    expect(qa('[data-test="unusable-row"]')).toHaveLength(0);
    head.click();
    flushSync();
    const rows = qa('[data-test="unusable-row"]');
    expect(rows.map((r) => r.textContent!.replace(/\s+/g, " ").trim())).toEqual([
      "ライン出力 Ryzen HD Audio Controller さすと、PC がこちらに切りかえます",
      "HDMI / DisplayPort 2 GB206 High Definition Audio Controller PC の設定で、いまは使えません",
    ]);
    expect(rows.every((r) => r.querySelector("button") === null)).toBe(true);
    expect(app.sent).toEqual([]);
    expect(q('[data-test="chooser-list"]')!.contains(head)).toBe(true);
  });

  it("the question row and the「ここから鳴らす」row are outside the scroll", () => {
    show(OutputChooser, withChooser());
    const list = q('[data-test="chooser-list"]')!;
    expect(list.contains(q('[data-test="play-here"]'))).toBe(false);
    expect(list.contains(q('[data-test="change-later"]'))).toBe(false);
    expect(qa('[data-test="endpoint-row"]').every((r) => list.contains(r))).toBe(true);
  });

  it("when outputs with the same name line up, add the device name in place of the friendly name", () => {
    show(
      OutputChooser,
      app.stateWith((s) => {
        s.outputs.list = [
          endpoint({ id: "a", name: "HDMI / DisplayPort", friendly: "hdmi", device: "GB206 High Definition Audio Controller" }),
          endpoint({ id: "b", name: "HDMI / DisplayPort", friendly: "hdmi", device: "Radeon High Definition Audio Controller" }),
          endpoint({ id: "c", name: "アナログヘッドフォン", friendly: "headphones" }),
        ];
        s.workspace.chooser = { highlighted: "a" };
      }),
    );
    const side = qa('[data-test="endpoint-row"] .friendly').map((e) => e.textContent);
    expect(side).toEqual(["GB206 High Definition Audio Controller", "Radeon High Definition Audio Controller", "ヘッドホンの穴"]);
  });

  it("row → chooserSelect, ▶ → testTone, 「ここから鳴らす」 → chooserConfirm", () => {
    show(OutputChooser, withChooser());
    const rows = qa('[data-test="endpoint-row"]');
    rows[3]!.querySelector<HTMLButtonElement>(".pick")!.click();
    rows[0]!.querySelector<HTMLButtonElement>('[data-test="test-tone"]')!.click();
    q<HTMLButtonElement>('[data-test="play-here"]')!.click();
    expect(app.sent).toEqual([
      { type: "chooserSelect", output: "x" },
      { type: "testTone", output: "hp" },
      { type: "chooserConfirm" },
    ]);
  });
});

describe("sound settings", () => {
  const withSettings = (advancedOpen: boolean) =>
    app.stateWith((s) => {
      s.keyboards = { ...s.keyboards, list: [{ ...KEYBOARD, name: "Keystation 49 MK3" }], active: "kb", lost: null, inputStopped: false };
      s.outputs.list = OUTPUTS;
      s.outputs.chosen = "hp";
      s.workspace.settings = { draftOutput: "hp", advancedOpen };
      s.audio = {
        ...s.audio,
        running: true,
        sampleRate: 48000,
        requestedFrames: 256,
        actualFrames: 1024,
        forcedQuantum: 1024,
        latencyMs: 53.33,
        latencyMsAtRequested: 13.33,
        outputDevice: "alsa_output.pci-0000_00_1f.3.analog-stereo",
        midiInput: "Keystation 49 MK3:Keystation 49 MK3 MIDI 1 24:0",
      };
      s.status.message = { id: "bufferFixed", actual: 1024, requested: 256, latencyMs: 53.33, latencyMsAtRequested: 13.33 };
    });

  it("not shown without settings", () => {
    show(SettingsDialog, app.stateWith(() => {}));
    expect(q('[data-test="settings-dialog"]')).toBeNull();
  });

  it("commands the buttons send", () => {
    show(SettingsDialog, withSettings(false));
    q<HTMLButtonElement>('[data-test="settings-cancel"]')!.click();
    q<HTMLButtonElement>('[data-test="settings-apply"]')!.click();
    q<HTMLButtonElement>('[data-test="settings-close"]')!.click();
    q<HTMLButtonElement>('[data-test="settings-advanced"]')!.click();
    q<HTMLButtonElement>('[data-test="settings-test-tone"]')!.click();
    qa('[data-test="endpoint-row"]')[3]!.querySelector<HTMLButtonElement>(".pick")!.click();
    expect(app.sent).toEqual([
      { type: "settingsCancel" },
      { type: "settingsApply" },
      { type: "settingsCancel" },
      { type: "settingsToggleAdvanced" },
      { type: "testTone", output: "hp" },
      { type: "settingsSelectOutput", output: "x" },
    ]);
  });

  it("front: outputs that cannot be chosen under the heading (right is「meloet は…」). All vocal inputs disabled. Latency about 53 ms", () => {
    show(SettingsDialog, withSettings(false));
    const rows = qa('[data-test="endpoint-row"]');
    // 4 outputs, vocal input only「使わない」, 1 keyboard
    expect(rows).toHaveLength(6);
    expect(rows.map((r) => r.dataset.disabled)).toEqual(["false", "false", "false", "false", "true", "false"]);
    expect(text('[data-test="unusable-head"]')).toBe("▸ いまは使えない出口 2 meloet は、PC 全体の設定を変えません");
    expect(rows[4]!.textContent).toContain("使わない");
    expect(rows[5]!.textContent).toContain("この鍵盤");
    expect(rows[5]!.dataset.selected).toBe("true");
    expect(text('[data-test="latency"]')).toBe("約 53 ms");
    expect(target.textContent).toContain("PipeWire ・ 48000 Hz ・ バッファ 1024");
    expect(q('[data-gl-slot="meter"]')).not.toBeNull();
    expect(q('[data-gl-slot="keyboard"]')).not.toBeNull();
  });

  it("inputs that cannot be chosen start folded under「いまは使えない入口」. No text on the right. Opening shows the reason and the device name", () => {
    const state = withSettings(false);
    state.inputs.list = [
      endpoint({ id: "mic", name: "リアマイクロフォン", friendly: "rearMic" }),
      endpoint({ id: "front", name: "フロントマイクロフォン", availability: "no", unusable: "switchedByPc" }),
      endpoint({ id: "line-in", name: "ライン入力", availability: "no", unusable: "stoppedByPc", device: "USB Audio Device" }),
    ];
    show(SettingsDialog, state);
    const rows = qa('[data-test="endpoint-row"]');
    // 4 outputs, vocal inputs 「リアマイクロフォン」 and「使わない」, 1 keyboard
    expect(rows).toHaveLength(7);
    expect(rows[4]!.textContent).toContain("うしろのマイクの穴");
    expect(rows[4]!.dataset.disabled).toBe("true");
    const heads = qa('[data-test="unusable-head"]');
    expect(heads).toHaveLength(2);
    expect(heads[1]!.textContent!.replace(/\s+/g, " ").trim()).toBe("▸ いまは使えない入口 2");
    expect(qa('[data-test="unusable-row"]')).toHaveLength(0);
    heads[1]!.click();
    flushSync();
    const unusable = qa('[data-test="unusable-row"]').map((r) => r.textContent!.replace(/\s+/g, " ").trim());
    expect(unusable).toEqual([
      "フロントマイクロフォン Ryzen HD Audio Controller さすと、PC がこちらに切りかえます",
      "ライン入力 USB Audio Device PC の設定で、いまは使えません",
    ]);
  });

  it("when the PC has not chosen a plugged-in input (other inputs are plugged in too), show the text to choose it in PC settings instead of「さすと…」", () => {
    const state = withSettings(false);
    state.inputs.list = [
      endpoint({ id: "front", name: "フロントマイクロフォン" }),
      endpoint({ id: "rear", name: "リアマイクロフォン", friendly: "rearMic", availability: "yes", unusable: "switchedByPc" }),
    ];
    show(SettingsDialog, state);
    qa('[data-test="unusable-head"]')[1]!.click();
    flushSync();
    expect(qa('[data-test="unusable-row"]').map((r) => r.textContent!.replace(/\s+/g, " ").trim())).toEqual([
      "リアマイクロフォン Ryzen HD Audio Controller ささっています ・ PC の設定でほかの入口をえらんでいます（設定 ▸ サウンド ▸ 入力 で変えられます）",
    ]);
  });

  it("advanced settings (F7-SettingsAdv): the heading is「音の設定」, the second page is「◂ 音の設定 くわしい設定」. No output or keyboard rows and no「音のしくみ」", () => {
    show(SettingsDialog, withSettings(true));
    expect(text(".head .title")).toBe("音の設定");
    expect(text('[data-test="settings-advanced"]')).toBe("◂ 音の設定");
    expect(qa('[data-test="endpoint-row"]')).toHaveLength(0);
    expect(target.textContent).not.toContain("音のしくみ");
    expect(target.textContent).not.toContain("PipeWire");
    expect(qa(".adv-body .row .name").map((n) => n.textContent!.trim())).toEqual(["入口", "バッファ", "サンプルレート", "録る形", "遅れの合わせ", "音の素材", "プラグインの場所", "AI とつなぐ", "新しい版をたしかめる"]);
  });

  it("advanced settings: the running number to the right of the asked number. If different,「頼んだ 256 ではなく、1024 で動いています…」below. The fixed box is not shown", () => {
    show(SettingsDialog, withSettings(true));
    expect(text('[data-test="settings-buffer"]')).toBe("256▾");
    expect(text('[data-test="buffer-actual"]')).toBe("動いている 1024 ・ 21 ms");
    expect(text('[data-test="buffer-differs"]')).toBe("頼んだ 256 ではなく、1024 で動いています（PC の設定で決まっています）。meloet は PC 全体の設定は変えません。");
    expect(text('[data-test="settings-rate"]')).toBe("48000 Hz▾");
    expect(text('[data-test="rate-actual"]')).toBe("動いている 48000");
    expect(q('[data-test="rate-differs"]')).toBeNull();
    expect(text('[data-test="settings-format"]')).toBe("24 ビット▾");
    expect(q('[data-test="buffer-fixed"]')).toBeNull();
  });

  it("advanced settings: ▾ opens the list, rows send intents.「遅れを測る…」, ◂, 「閉じる」", () => {
    show(SettingsDialog, withSettings(true));
    q<HTMLButtonElement>('[data-test="settings-buffer"]')!.click();
    const rows = (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
    expect(rows.map((r) => [r.text, r.checked])).toEqual([
      ["64", false],
      ["128", false],
      ["256", true],
      ["512", false],
      ["1024", false],
    ]);
    rows[1]!.run();
    q<HTMLButtonElement>('[data-test="settings-format"]')!.click();
    const formats = (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
    expect(formats.map((r) => r.text)).toEqual(["16 ビット", "24 ビット", "32 ビット"]);
    formats[2]!.run();
    q<HTMLButtonElement>('[data-test="settings-rate"]')!.click();
    (contextMenu.open!.entries[0] as MenuRow).run();
    q<HTMLButtonElement>('[data-test="settings-measure"]')!.click();
    q<HTMLButtonElement>('[data-test="settings-advanced"]')!.click();
    q<HTMLButtonElement>('[data-test="settings-done"]')!.click();
    closeContextMenu();
    expect(app.sent).toEqual([
      { type: "settingsSetBuffer", frames: 128 },
      { type: "settingsSetRecordFormat", format: "bits32" },
      { type: "settingsSetSampleRate", hz: 44100 },
      { type: "openLatencyMeasure" },
      { type: "settingsToggleAdvanced" },
      { type: "settingsCancel" },
    ]);
  });

  describe("「手で決める…」(spec 3 94-19)", () => {
    const typeLatency = (value: string, key: string) => {
      q<HTMLButtonElement>('[data-test="settings-manual"]')!.click();
      flushSync();
      const input = q<HTMLInputElement>('[data-test="param-input"]')!;
      expect(input).not.toBeNull();
      expect(q('[data-test="latency-used"]')).toBeNull();
      input.value = value;
      input.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
      flushSync();
    };

    it("pressing it turns the row's number into「8.0」and「ms」boxes in place. Enter decides", () => {
      show(SettingsDialog, withSettings(true));
      expect(q<HTMLButtonElement>('[data-test="settings-manual"]')!.disabled).toBe(false);
      q<HTMLButtonElement>('[data-test="settings-manual"]')!.click();
      flushSync();
      const input = q<HTMLInputElement>('[data-test="param-input"]')!;
      const used = app.current.state!.audio.latency.usedMs;
      expect(input.value).toBe(used === null ? "" : used.toFixed(1));
      expect(input.parentElement!.textContent).toContain("ms");
      input.blur();
      flushSync();
      typeLatency("12.5", "Enter");
      expect(app.sent).toEqual([{ type: "settingsSetManualLatency", ms: 12.5 }]);
      expect(q('[data-test="param-input"]')).toBeNull();
      expect(q('[data-test="latency-used"]')).not.toBeNull();
    });

    it("Esc cancels (sends nothing). Text that cannot be read is not sent. Empty drops the number set by hand", () => {
      show(SettingsDialog, withSettings(true));
      typeLatency("20", "Escape");
      typeLatency("abc", "Enter");
      typeLatency("１５ ms", "Enter");
      typeLatency("", "Enter");
      expect(app.sent).toEqual([
        { type: "settingsSetManualLatency", ms: 15 },
        { type: "settingsSetManualLatency", ms: null },
      ]);
    });
  });
});

describe("sound settings: with long content (9 output rows)", () => {
  const many = (advancedOpen: boolean) =>
    app.stateWith((s) => {
      s.keyboards = { ...s.keyboards, list: [{ ...KEYBOARD, name: "Keystation 49 MK3" }], active: "kb", lost: null, inputStopped: false };
      s.outputs.list = [
        ...OUTPUTS,
        ...[1, 2, 3, 4, 5, 6].map((n) => endpoint({ id: `hdmi-${n}`, name: `HDMI / DisplayPort ${n}`, link: "plug", unusable: "stoppedByPc" })),
      ];
      s.workspace.settings = { draftOutput: "hp", advancedOpen };
    });

  it.each([false, true])("the heading and the bottom row are outside the scroll. The content and the WebGL slot are inside the scroll (deep part: %s)", (advanced) => {
    show(SettingsDialog, many(advanced));
    const dialog = q('[data-test="settings-dialog"]')!;
    const body = q('[data-test="settings-body"]')!;
    expect(body.hasAttribute("data-gl-scroll")).toBe(true);
    expect([...dialog.children].map((c) => c.className.split(" ")[0])).toEqual(advanced ? ["head", "crumb", "area", "foot"] : ["head", "area", "foot"]);
    for (const b of advanced ? ["settings-done", "settings-close"] : ["settings-cancel", "settings-apply", "settings-close"]) {
      const button = q(`[data-test="${b}"]`)!;
      expect(body.contains(button)).toBe(false);
      expect((button as HTMLButtonElement).disabled).toBe(false);
    }
    const rows = qa('[data-test="endpoint-row"]');
    expect(rows.every((r) => body.contains(r))).toBe(true);
    expect(rows).toHaveLength(advanced ? 0 : 4 + 1 + 1);
    if (!advanced) {
      q<HTMLButtonElement>('[data-test="unusable-head"]')!.click();
      flushSync();
      expect(qa('[data-test="unusable-row"]').every((r) => body.contains(r))).toBe(true);
      expect(qa('[data-test="unusable-row"]')).toHaveLength(8);
    }
    for (const slot of qa("[data-gl-slot]")) expect(body.contains(slot)).toBe(true);
    expect(qa("[data-gl-slot]")).toHaveLength(advanced ? 0 : 2);
  });
});

describe("「音が出ました」", () => {
  it("shown only when hint is soundCameOut. With the record button disabled for now, only the first sentence", () => {
    show(SoundHint, app.stateWith((s) => (s.workspace.hint = "emptyArrange")));
    expect(q('[data-test="sound-hint"]')).toBeNull();
    app.setState(app.stateWith((s) => (s.workspace.hint = "soundCameOut")));
    flushSync();
    expect(text('[data-test="sound-hint"]')).toBe("音が出ました。");
  });
});

describe("status line", () => {
  it("no keyboard:「入口: まだ」. Sound is not running (actualFrames is null), so no buffer", () => {
    show(StatusLine, app.stateWith(() => {}));
    const s = text("footer.status")!;
    expect(s).toContain("入口: まだ");
    expect(s).toContain("出口: まだ");
    expect(s).not.toContain("バッファ");
  });

  it.each([
    [{ id: "audioSystemStopped" }, "PC の音のしくみ（PipeWire）が止まりました。曲はそのままです。動きだしたら、自動でつなぎなおします。", false],
    [
      { id: "recordingStoppedAudioSystem", take: 2 },
      "PC の音のしくみ（PipeWire）が止まったので、録音を止めました。そこまでの演奏は テイク 2 に残っています。",
      false,
    ],
    [
      { id: "recordingStoppedOutputMissing", output: "ヘッドホン", take: 2 },
      "ヘッドホンが見つからなくなったので、録音を止めました。そこまでの演奏は テイク 2 に残っています。",
      true,
    ],
    [{ id: "keyboardInputStopped" }, "鍵盤からの入力が止まりました。曲はそのままです。いま、つなぎなおしています。", false],
  ] as const)("知らせ %o", (message, sentence, button) => {
    show(
      StatusLine,
      app.stateWith((s) => {
        s.status.message = message;
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: message.id === "keyboardInputStopped" };
      }),
    );
    expect(text("footer.status .message")).toBe(sentence);
    const link = q<HTMLButtonElement>("footer.status button.link");
    if (button) {
      expect(link?.textContent).toBe("音の設定をひらく");
      link!.click();
      expect(app.sent).toEqual([{ type: "openSettings", advanced: false }]);
    } else {
      expect(link).toBeNull();
    }
    if (message.id === "keyboardInputStopped") expect(text("footer.status")).toContain("入口: まだ");
  });

  it("keyboard there, sound running: the input is the keyboard name, the buffer is the running number", () => {
    show(
      StatusLine,
      app.stateWith((s) => {
        s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
        s.audio.actualFrames = 256;
      }),
    );
    const s = text("footer.status")!;
    expect(s).toContain("入口: 鍵盤 49");
    expect(s).not.toContain("入口: まだ");
    expect(s).toContain("バッファ 256");
  });
});
