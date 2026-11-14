// 入力計、出口えらび、音の設定、「音が出ました」を mount して確かめる。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, EndpointView } from "../src/bridge";
import InputMeter from "../src/components/InputMeter.svelte";
import OutputChooser from "../src/components/OutputChooser.svelte";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import SoundHint from "../src/components/SoundHint.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
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
  choosable: true,
  ...e,
});

const OUTPUTS: EndpointView[] = [
  endpoint({ id: "hp", name: "アナログヘッドフォン", friendly: "headphones" }),
  endpoint({ id: "hdmi", name: "HDMI / DisplayPort 2", friendly: "hdmi", link: "plug", choosable: false }),
  endpoint({ id: "line", name: "ライン出力", friendly: "lineOut", availability: "no", choosable: false }),
  endpoint({ id: "x", name: "なにか", availability: "unknown" }),
];

describe("入力計", () => {
  it("鍵盤がなく、抜けた鍵盤もない: 「鍵盤なし」と、つなぐ案内。読み値は出さない", () => {
    show(InputMeter, app.stateWith(() => {}));
    expect(text('[data-test="keyboard-name"]')).toBe("鍵盤なし");
    expect(text('[data-test="meter-right"]')).toBe(t("msg.keyboardMissing"));
    expect(q('[data-test="held"]')).toBeNull();
    expect(q('[data-gl-slot="keyboard"]')).not.toBeNull();
  });

  it("鍵盤がなく、抜けた鍵盤がある: つながりが切れた文", () => {
    show(InputMeter, app.stateWith((s) => (s.keyboards.lost = "Keystation 49 MK3")));
    expect(text('[data-test="meter-right"]')).toBe(
      "Keystation 49 MK3 とのつながりが切れました。曲はそのままです。さしなおすと、自動で戻ります。",
    );
  });

  it("鍵盤があり、出口がまだ: accent の「音の出口がまだです…」", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
      }),
    );
    expect(text('[data-test="keyboard-name"]')).toBe("鍵盤 49");
    const right = q('[data-test="meter-right"]')!;
    expect(right.textContent).toBe("音の出口がまだです。下でえらんでください");
    expect(right.classList.contains("accent")).toBe(true);
  });

  it("出口がある: 弾くと鳴る先「1 ピアノ ・ 仮の音」", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
        s.outputs.chosen = "hp";
      }),
    );
    expect(text('[data-test="meter-right"]')).toBe("弾くと鳴る先 1 ピアノ ・ 仮の音");
  });

  it("細いとき: 弾くと鳴る先は「1 ピアノ」だけで、音源の名前は出さない（モック A-PianoRoll・F-History）", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
        s.outputs.chosen = "hp";
        s.workspace.layout.meter = "slim";
      }),
    );
    expect(text('[data-test="meter-right"]')).toBe("弾くと鳴る先 1 ピアノ");
  });

  it("押している音: 計器の鍵を低い順に。鍵の on/off は計器を待たずに変わる", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
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

  it("サステインは 64 から ON。細いときは ON ・ OFF だけ", () => {
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
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

  it("太いとき: 名前が 8 文字までは名前の欄 108、こえたら 148", () => {
    const long = { ...KEYBOARD, name: "Keystation 49 MK3" };
    show(
      InputMeter,
      app.stateWith((s) => {
        s.keyboards = { list: [long], active: "kb", lost: null };
      }),
    );
    expect(q(".meter > .who")!.style.width).toBe("var(--part-meter-name-long)");
    app.setState(
      app.stateWith((s) => {
        s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
      }),
    );
    flushSync();
    expect(q(".meter > .who")!.style.width).toBe("var(--size-meter-name)");
  });
});

describe("出口えらび", () => {
  const withChooser = () =>
    app.stateWith((s) => {
      s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
      s.outputs.list = OUTPUTS;
      s.workspace.chooser = { highlighted: "hp" };
    });

  it("chooser がなければ出さない", () => {
    show(OutputChooser, app.stateWith(() => {}));
    expect(q('[data-test="output-chooser"]')).toBeNull();
  });

  it("見出しと、行ごとの中身", () => {
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
  });

  it("choosable でない行は押せず、テスト音もない", () => {
    show(OutputChooser, withChooser());
    const rows = qa('[data-test="endpoint-row"]');
    for (const i of [1, 2]) {
      expect(rows[i]!.dataset.disabled).toBe("true");
      const pick = rows[i]!.querySelector<HTMLButtonElement>(".pick")!;
      expect(pick.disabled).toBe(true);
      pick.click();
      expect(rows[i]!.querySelector('[data-test="test-tone"]')).toBeNull();
    }
    expect(app.sent).toEqual([]);
  });

  it("行 → chooserSelect、▶ → testTone、ここから鳴らす → chooserConfirm", () => {
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

describe("音の設定", () => {
  const withSettings = (advancedOpen: boolean) =>
    app.stateWith((s) => {
      s.keyboards = { list: [{ ...KEYBOARD, name: "Keystation 49 MK3" }], active: "kb", lost: null };
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

  it("settings がなければ出さない", () => {
    show(SettingsDialog, app.stateWith(() => {}));
    expect(q('[data-test="settings-dialog"]')).toBeNull();
  });

  it("ボタンが送るコマンド", () => {
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

  it("おもて: 出口は choosable でない行が使えない。歌の入口は全部使えない。遅れは約 53 ms", () => {
    show(SettingsDialog, withSettings(false));
    const rows = qa('[data-test="endpoint-row"]');
    // 出口 4、歌の入口は「使わない」だけ、鍵盤 1
    expect(rows).toHaveLength(6);
    expect(rows.map((r) => r.dataset.disabled)).toEqual(["false", "true", "true", "false", "true", "false"]);
    expect(rows[4]!.textContent).toContain("使わない");
    expect(rows[5]!.textContent).toContain("この鍵盤");
    expect(rows[5]!.dataset.selected).toBe("true");
    expect(text('[data-test="latency"]')).toBe("約 53 ms");
    expect(target.textContent).toContain("PipeWire ・ 48000 Hz ・ バッファ 1024");
    expect(q('[data-gl-slot="meter"]')).not.toBeNull();
    expect(q('[data-gl-slot="keyboard"]')).not.toBeNull();
  });

  it("奥: 上の 3 つはえらんだ物だけ。固定の囲みと、頼んだ数とちがう知らせ", () => {
    show(SettingsDialog, withSettings(true));
    const rows = qa('[data-test="endpoint-row"]');
    expect(rows.map((r) => r.textContent?.includes("アナログヘッドフォン") || r.textContent?.includes("使わない") || r.textContent?.includes("Keystation"))).toEqual([true, true, true]);
    expect(text('[data-test="buffer-actual"]')).toBe("1024");
    expect(target.textContent).toContain("サンプル ・ 頼んだ数とちがいます");
    const fixed = text('[data-test="buffer-fixed"]')!;
    expect(fixed).toContain("バッファが PC 全体の設定で 1024 に固定されています");
    expect(fixed).toContain("meloet は 256 を頼んでいますが、1024 で動いています。鍵を押してから音が出るまで 約 53 ms。256 で動けば 約 13.3 ms です。");
    expect(fixed).toContain("変える場所は PipeWire の設定（quantum の固定）です。");
  });

  it("奥: オンの部品は押しても何も送らず、ほかは使えない", () => {
    show(SettingsDialog, withSettings(true));
    const buttons = qa("button.b");
    const on = buttons.filter((b) => b.classList.contains("on"));
    expect(on.map((b) => b.textContent?.trim())).toEqual(["PipeWire", "48000", "256"]);
    for (const b of on) b.click();
    expect(app.sent).toEqual([]);
    const off = buttons.filter((b) => ["JACK", "ALSA", "44100", "512", "自動", "測りなおす"].includes(b.textContent!.trim()));
    expect(off.every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });
});

describe("音が出ました", () => {
  it("hint が soundCameOut のときだけ、仕様書の文だけを出す", () => {
    show(SoundHint, app.stateWith((s) => (s.workspace.hint = "emptyArrange")));
    expect(q('[data-test="sound-hint"]')).toBeNull();
    app.setState(app.stateWith((s) => (s.workspace.hint = "soundCameOut")));
    flushSync();
    expect(text('[data-test="sound-hint"]')).toBe(t("msg.soundCameOut"));
  });
});

describe("状態行", () => {
  it("鍵盤がない: 「入口: まだ」。音が動いていない（actualFrames が null）ので、バッファは出さない", () => {
    show(StatusLine, app.stateWith(() => {}));
    const s = text("footer.status")!;
    expect(s).toContain("入口: まだ");
    expect(s).toContain("出口: まだ");
    expect(s).not.toContain("バッファ");
  });

  it("鍵盤があり、音が動いている: 入口は鍵盤の名前、バッファは動いている数", () => {
    show(
      StatusLine,
      app.stateWith((s) => {
        s.keyboards = { list: [KEYBOARD], active: "kb", lost: null };
        s.audio.actualFrames = 256;
      }),
    );
    const s = text("footer.status")!;
    expect(s).toContain("入口: 鍵盤 49");
    expect(s).not.toContain("入口: まだ");
    expect(s).toContain("バッファ 256");
  });
});
